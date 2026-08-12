//! Strong loaded-module identity proof using GNU build-id whenever available.
//!
//! The loaded raw GNU build-id note is compared with the identically encoded
//! ELF note from the RPM-owned candidate. `srcversion` is only a strict fallback
//! when neither side exposes a build-id. Candidate bytes never pass through a
//! named temporary file: regular modules are read from an already fingerprinted
//! descriptor and compressed modules stream into bounded memory.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::process::{Command, Stdio};

const TIMEOUT_COMMAND: &str = "/usr/bin/timeout";
const TIMEOUT_DURATION: &str = "8s";
const TIMEOUT_KILL_AFTER: &str = "2s";
const XZ_COMMAND: &str = "/usr/bin/xz";
const GZIP_COMMAND: &str = "/usr/bin/gzip";
const ZSTD_COMMAND: &str = "/usr/bin/zstd";
const XZ_MEMORY_LIMIT_ARGUMENT: &str = "--memlimit-decompress=128MiB";
const XZ_MT_MEMORY_LIMIT_ARGUMENT: &str = "--memlimit-mt-decompress=128MiB";
const ZSTD_MEMORY_LIMIT_ARGUMENT: &str = "--memory=128MB";
const GNU_BUILD_ID_SECTION: &str = ".note.gnu.build-id";
const MAX_PACKAGED_MODULE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_UNCOMPRESSED_MODULE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_BUILD_ID_NOTE_SIZE: u64 = 128;
const MAX_ELF_SECTIONS: usize = 4096;
const MAX_ELF_SECTION_NAME_TABLE_SIZE: u64 = 1024 * 1024;
const ELF64_HEADER_SIZE: usize = 64;
const ELF64_SECTION_HEADER_SIZE: usize = 64;
const ELF_SHT_NULL: u32 = 0;
const ELF_SHT_STRTAB: u32 = 3;
const ELF_SHT_NOTE: u32 = 7;
const ELF_SHT_NOBITS: u32 = 8;
const ELF_SHN_XINDEX: u16 = 0xffff;
// Linux O_NOFOLLOW. Galaxy Book Setup is packaged only for Fedora/Linux.
const LINUX_O_NOFOLLOW: i32 = 0o400000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    size: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    links: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: i64,
    ctime_seconds: i64,
    ctime_nanoseconds: i64,
}

impl FileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.size(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            mtime_seconds: metadata.mtime(),
            mtime_nanoseconds: metadata.mtime_nsec(),
            ctime_seconds: metadata.ctime(),
            ctime_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModuleCompression {
    None,
    Xz,
    Gzip,
    Zstd,
}

#[derive(Debug, PartialEq, Eq)]
enum BuildIdEvidence {
    Present(Vec<u8>),
    Absent,
    Invalid,
}

impl ModuleCompression {
    fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if name.ends_with(".ko.xz") {
            Some(Self::Xz)
        } else if name.ends_with(".ko.gz") {
            Some(Self::Gzip)
        } else if name.ends_with(".ko.zst") {
            Some(Self::Zstd)
        } else if name.ends_with(".ko") {
            Some(Self::None)
        } else {
            None
        }
    }

    fn command(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Self::None => None,
            Self::Xz => Some((
                XZ_COMMAND,
                &[
                    "--decompress",
                    "--stdout",
                    "--threads=1",
                    XZ_MEMORY_LIMIT_ARGUMENT,
                    XZ_MT_MEMORY_LIMIT_ARGUMENT,
                ],
            )),
            Self::Gzip => Some((GZIP_COMMAND, &["--decompress", "--stdout"])),
            Self::Zstd => Some((
                ZSTD_COMMAND,
                &[
                    "--decompress",
                    "--stdout",
                    "--quiet",
                    ZSTD_MEMORY_LIMIT_ARGUMENT,
                    "-T1",
                ],
            )),
        }
    }
}

fn canonical_srcversion(value: Option<&str>) -> Option<&str> {
    let value = value?;
    // sysfs and modinfo terminate their single-line value with LF. Accept one
    // terminal Linux/CRLF line ending and horizontal ASCII padding only; do
    // not let generic Unicode trimming turn multiline or control-laden input
    // into apparently valid evidence.
    let value = value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix('\n'))
        .unwrap_or(value)
        .trim_matches([' ', '\t']);
    (value.len() == 23
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte)))
    .then_some(value)
}

fn binary_identity_matches_with<F>(
    loaded_srcversion: Option<&str>,
    candidate_srcversion: Option<&str>,
    build_ids: F,
) -> bool
where
    F: FnOnce() -> (BuildIdEvidence, BuildIdEvidence),
{
    match build_ids() {
        (BuildIdEvidence::Present(loaded), BuildIdEvidence::Present(candidate)) => {
            loaded == candidate
        }
        (BuildIdEvidence::Absent, BuildIdEvidence::Absent) => match (
            canonical_srcversion(loaded_srcversion),
            canonical_srcversion(candidate_srcversion),
        ) {
            (Some(loaded), Some(candidate)) => loaded == candidate,
            (None, None) | (Some(_), None) | (None, Some(_)) => false,
        },
        _ => false,
    }
}

pub(crate) fn loaded_module_binary_matches_candidate(
    proc_name: &str,
    candidate_path: &str,
    loaded_srcversion: Option<&str>,
    candidate_srcversion: Option<&str>,
) -> bool {
    binary_identity_matches_with(loaded_srcversion, candidate_srcversion, || {
        (
            loaded_module_build_id_evidence(proc_name),
            packaged_module_build_id_evidence(Path::new(candidate_path)),
        )
    })
}

fn safe_proc_module_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn trusted_directory(path: &Path, expected_mode: u32, require_root_owned: bool) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    metadata.is_dir()
        && metadata.mode() & 0o7777 == expected_mode
        && (!require_root_owned || metadata.uid() == 0 && metadata.gid() == 0)
}

fn build_id_note_permissions_are_trusted(
    uid: u32,
    gid: u32,
    mode: u32,
    links: u64,
    expected_mode: u32,
    require_root_owned: bool,
) -> bool {
    mode & 0o7777 == expected_mode && links == 1 && (!require_root_owned || uid == 0 && gid == 0)
}

#[cfg(test)]
fn trusted_loaded_build_id_path(
    sys_root: &Path,
    proc_name: &str,
    require_root_owned: bool,
) -> Option<PathBuf> {
    if !safe_proc_module_name(proc_name) || !trusted_directory(sys_root, 0o555, require_root_owned)
    {
        return None;
    }
    let module_root = sys_root.join("module");
    let module_directory = module_root.join(proc_name);
    let notes_directory = module_directory.join("notes");
    if !trusted_directory(&module_root, 0o755, require_root_owned)
        || !trusted_directory(&module_directory, 0o755, require_root_owned)
        || !trusted_directory(&notes_directory, 0o755, require_root_owned)
    {
        return None;
    }
    Some(notes_directory.join(GNU_BUILD_ID_SECTION))
}

fn trusted_note_file_identity(
    path: &Path,
    expected_mode: u32,
    require_root_owned: bool,
) -> Option<FileIdentity> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file()
        || metadata.size() == 0
        || metadata.size() > MAX_BUILD_ID_NOTE_SIZE
        || !build_id_note_permissions_are_trusted(
            metadata.uid(),
            metadata.gid(),
            metadata.mode(),
            metadata.nlink(),
            expected_mode,
            require_root_owned,
        )
    {
        return None;
    }
    Some(FileIdentity::from_metadata(&metadata))
}

fn valid_gnu_build_id_note(note: &[u8]) -> bool {
    if note.len() < 16 || note.len() > MAX_BUILD_ID_NOTE_SIZE as usize {
        return false;
    }
    let read_u32 = |offset: usize| {
        note.get(offset..offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
    };
    let Some(name_size) = read_u32(0).map(|value| value as usize) else {
        return false;
    };
    let Some(description_size) = read_u32(4).map(|value| value as usize) else {
        return false;
    };
    let Some(note_type) = read_u32(8) else {
        return false;
    };
    let align = |size: usize| size.checked_add(3).map(|value| value & !3);
    let Some(aligned_name_size) = align(name_size) else {
        return false;
    };
    let Some(aligned_description_size) = align(description_size) else {
        return false;
    };
    let Some(expected_size) = 12usize
        .checked_add(aligned_name_size)
        .and_then(|value| value.checked_add(aligned_description_size))
    else {
        return false;
    };
    name_size == 4
        && (16..=64).contains(&description_size)
        && note_type == 3
        && note.get(12..16) == Some(b"GNU\0")
        && expected_size == note.len()
}

fn read_build_id_note_with_hook<F>(
    path: &Path,
    expected_mode: u32,
    require_root_owned: bool,
    after_read: F,
) -> Option<Vec<u8>>
where
    F: FnOnce(),
{
    let before = trusted_note_file_identity(path, expected_mode, require_root_owned)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(LINUX_O_NOFOLLOW)
        .open(path)
        .ok()?;
    if FileIdentity::from_metadata(&file.metadata().ok()?) != before {
        return None;
    }
    let mut note = Vec::with_capacity(before.size as usize);
    Read::by_ref(&mut file)
        .take(MAX_BUILD_ID_NOTE_SIZE + 1)
        .read_to_end(&mut note)
        .ok()?;
    after_read();
    let descriptor_after = FileIdentity::from_metadata(&file.metadata().ok()?);
    let path_after = trusted_note_file_identity(path, expected_mode, require_root_owned)?;
    if before != descriptor_after
        || before != path_after
        || note.len() as u64 != before.size
        || !valid_gnu_build_id_note(&note)
    {
        return None;
    }
    Some(note)
}

fn read_build_id_note(
    path: &Path,
    expected_mode: u32,
    require_root_owned: bool,
) -> Option<Vec<u8>> {
    read_build_id_note_with_hook(path, expected_mode, require_root_owned, || {})
}

fn loaded_module_build_id_evidence(proc_name: &str) -> BuildIdEvidence {
    let Ok(root) = fs::symlink_metadata("/") else {
        return BuildIdEvidence::Invalid;
    };
    if !root.is_dir() || root.uid() != 0 || root.gid() != 0 || root.mode() & 0o022 != 0 {
        return BuildIdEvidence::Invalid;
    }
    loaded_module_build_id_evidence_in(Path::new("/sys"), proc_name, true)
}

fn loaded_module_build_id_evidence_in(
    sys_root: &Path,
    proc_name: &str,
    require_root_owned: bool,
) -> BuildIdEvidence {
    if !safe_proc_module_name(proc_name) || !trusted_directory(sys_root, 0o555, require_root_owned)
    {
        return BuildIdEvidence::Invalid;
    }
    let module_root = sys_root.join("module");
    let module_directory = module_root.join(proc_name);
    if !trusted_directory(&module_root, 0o755, require_root_owned)
        || !trusted_directory(&module_directory, 0o755, require_root_owned)
    {
        return BuildIdEvidence::Invalid;
    }
    let notes_directory = module_directory.join("notes");
    match fs::symlink_metadata(&notes_directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return BuildIdEvidence::Absent,
        Err(_) => return BuildIdEvidence::Invalid,
        Ok(_) if !trusted_directory(&notes_directory, 0o755, require_root_owned) => {
            return BuildIdEvidence::Invalid;
        }
        Ok(_) => {}
    }
    let path = notes_directory.join(GNU_BUILD_ID_SECTION);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => BuildIdEvidence::Absent,
        Err(_) => BuildIdEvidence::Invalid,
        Ok(_) => read_build_id_note(&path, 0o444, require_root_owned)
            .map(BuildIdEvidence::Present)
            .unwrap_or(BuildIdEvidence::Invalid),
    }
}

fn packaged_module_permissions_are_trusted(
    metadata: &fs::Metadata,
    require_root_owned: bool,
) -> bool {
    metadata.is_file()
        && metadata.size() > 0
        && metadata.size() <= MAX_PACKAGED_MODULE_SIZE
        && metadata.nlink() == 1
        && metadata.mode() & 0o7777 == 0o644
        && (!require_root_owned || metadata.uid() == 0 && metadata.gid() == 0)
}

fn trusted_packaged_module(path: &Path, require_root_owned: bool) -> Option<(File, FileIdentity)> {
    ModuleCompression::from_path(path)?;
    let path_metadata = fs::symlink_metadata(path).ok()?;
    if !packaged_module_permissions_are_trusted(&path_metadata, require_root_owned) {
        return None;
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(LINUX_O_NOFOLLOW)
        .open(path)
        .ok()?;
    let identity = FileIdentity::from_metadata(&file.metadata().ok()?);
    (identity == FileIdentity::from_metadata(&path_metadata)).then_some((file, identity))
}

fn bounded_command(command: &str, args: &[&OsStr]) -> Command {
    let mut child = Command::new(TIMEOUT_COMMAND);
    child
        .arg(format!("--kill-after={TIMEOUT_KILL_AFTER}"))
        .arg(TIMEOUT_DURATION)
        .arg(command)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C");
    child
}

fn read_bounded_module(mut input: impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    Read::by_ref(&mut input)
        .take(MAX_UNCOMPRESSED_MODULE_SIZE + 1)
        .read_to_end(&mut bytes)?;
    Ok((!bytes.is_empty() && bytes.len() as u64 <= MAX_UNCOMPRESSED_MODULE_SIZE).then_some(bytes))
}

fn module_bytes_with_hook<F>(
    compression: ModuleCompression,
    input: File,
    reader_ready: F,
) -> io::Result<Option<Vec<u8>>>
where
    F: FnOnce(),
{
    if compression == ModuleCompression::None {
        reader_ready();
        return read_bounded_module(input);
    }

    let (command, arguments) = compression.command().expect("compressed module command");
    let arguments = arguments
        .iter()
        .map(|argument| OsStr::new(argument))
        .collect::<Vec<_>>();
    // Rust-created and cloned descriptors are CLOEXEC. Only this clone is
    // deliberately installed as the bounded decompressor's standard input;
    // the original fingerprinted descriptor remains private to this process.
    let mut child = bounded_command(command, &arguments)
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    reader_ready();
    let read_result = child
        .stdout
        .take()
        .map(read_bounded_module)
        .unwrap_or(Ok(None));
    let status_result = child.wait();
    let bytes = read_result?;
    let status = status_result?;
    Ok(if status.success() { bytes } else { None })
}

fn elf_slice(bytes: &[u8], offset: u64, size: usize) -> Option<&[u8]> {
    let offset = usize::try_from(offset).ok()?;
    let end = offset.checked_add(size)?;
    bytes.get(offset..end)
}

fn elf_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn elf_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn elf_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

#[derive(Clone, Copy)]
struct ElfSectionHeader {
    name: u32,
    section_type: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
    alignment: u64,
}

impl ElfSectionHeader {
    fn is_valid_section_zero(self) -> bool {
        self.section_type == ELF_SHT_NULL && self.address == 0 && self.size == 0
    }

    fn file_range_is_valid(self, file_size: u64) -> bool {
        matches!(self.section_type, ELF_SHT_NULL | ELF_SHT_NOBITS)
            || (self.offset <= file_size && self.size <= file_size - self.offset)
    }
}

fn elf_section_header(bytes: &[u8]) -> Option<ElfSectionHeader> {
    (bytes.len() == ELF64_SECTION_HEADER_SIZE).then_some(())?;
    Some(ElfSectionHeader {
        name: elf_u32(bytes, 0)?,
        section_type: elf_u32(bytes, 4)?,
        flags: elf_u64(bytes, 8)?,
        address: elf_u64(bytes, 16)?,
        offset: elf_u64(bytes, 24)?,
        size: elf_u64(bytes, 32)?,
        alignment: elf_u64(bytes, 48)?,
    })
}

fn read_elf_section_header(
    bytes: &[u8],
    section_table_offset: u64,
    index: usize,
) -> Option<ElfSectionHeader> {
    let offset = section_table_offset
        .checked_add((index as u64).checked_mul(ELF64_SECTION_HEADER_SIZE as u64)?)?;
    elf_section_header(elf_slice(bytes, offset, ELF64_SECTION_HEADER_SIZE)?)
}

fn section_name(name_table: &[u8], offset: u32) -> Option<&[u8]> {
    let name = name_table.get(offset as usize..)?;
    let end = name.iter().position(|byte| *byte == 0)?;
    Some(&name[..end])
}

fn elf_gnu_build_id_evidence(bytes: &[u8]) -> BuildIdEvidence {
    let parsed = (|| -> Option<Option<Vec<u8>>> {
        let file_size = bytes.len() as u64;
        if bytes.len() < ELF64_HEADER_SIZE || file_size > MAX_UNCOMPRESSED_MODULE_SIZE {
            return None;
        }
        let header = bytes.get(..ELF64_HEADER_SIZE)?;
        if header.get(0..7) != Some(b"\x7fELF\x02\x01\x01")
            || elf_u16(&header, 16)? != 1
            || elf_u16(&header, 18)? != 62
            || elf_u32(&header, 20)? != 1
            || elf_u16(&header, 52)? as usize != ELF64_HEADER_SIZE
            || elf_u16(&header, 58)? as usize != ELF64_SECTION_HEADER_SIZE
        {
            return None;
        }

        let section_table_offset = elf_u64(&header, 40)?;
        if section_table_offset < ELF64_HEADER_SIZE as u64 {
            return None;
        }
        let section_count = elf_u16(&header, 60)? as usize;
        if !(2..=MAX_ELF_SECTIONS).contains(&section_count) {
            return None;
        }
        let section_table_size =
            (section_count as u64).checked_mul(ELF64_SECTION_HEADER_SIZE as u64)?;
        if section_table_offset.checked_add(section_table_size)? > file_size {
            return None;
        }
        let section_zero = read_elf_section_header(bytes, section_table_offset, 0)?;
        if !section_zero.is_valid_section_zero() {
            return None;
        }
        for index in 1..section_count {
            if !read_elf_section_header(bytes, section_table_offset, index)?
                .file_range_is_valid(file_size)
            {
                return None;
            }
        }

        let name_table_index_raw = elf_u16(&header, 62)?;
        if name_table_index_raw == ELF_SHN_XINDEX {
            return None;
        }
        let name_table_index = name_table_index_raw as usize;
        if name_table_index == 0 || name_table_index >= section_count {
            return None;
        }
        let name_table_header =
            read_elf_section_header(bytes, section_table_offset, name_table_index)?;
        if name_table_header.section_type != ELF_SHT_STRTAB
            || name_table_header.size == 0
            || name_table_header.size > MAX_ELF_SECTION_NAME_TABLE_SIZE
        {
            return None;
        }
        let name_table_size = usize::try_from(name_table_header.size).ok()?;
        let name_table = elf_slice(bytes, name_table_header.offset, name_table_size)?;
        if name_table.first() != Some(&0) || name_table.last() != Some(&0) {
            return None;
        }

        let mut build_id = None;
        for index in 1..section_count {
            let section = read_elf_section_header(bytes, section_table_offset, index)?;
            if section.section_type == ELF_SHT_NULL {
                continue;
            }
            let name = section_name(&name_table, section.name)?;
            if name != GNU_BUILD_ID_SECTION.as_bytes() {
                continue;
            }
            if build_id.is_some()
                || section.section_type != ELF_SHT_NOTE
                || section.flags != 2
                || section.size == 0
                || section.size > MAX_BUILD_ID_NOTE_SIZE
                || section.alignment != 4
            {
                return None;
            }
            let section_size = usize::try_from(section.size).ok()?;
            let note = elf_slice(bytes, section.offset, section_size)?;
            if !valid_gnu_build_id_note(&note) {
                return None;
            }
            build_id = Some(note.to_vec());
        }
        Some(build_id)
    })();

    match parsed {
        Some(Some(build_id)) => BuildIdEvidence::Present(build_id),
        Some(None) => BuildIdEvidence::Absent,
        None => BuildIdEvidence::Invalid,
    }
}

#[cfg(test)]
fn elf_gnu_build_id(bytes: &[u8]) -> Option<Vec<u8>> {
    match elf_gnu_build_id_evidence(bytes) {
        BuildIdEvidence::Present(build_id) => Some(build_id),
        BuildIdEvidence::Absent | BuildIdEvidence::Invalid => None,
    }
}

fn packaged_module_build_id_evidence_in_with_hooks<F, G>(
    path: &Path,
    require_root_owned: bool,
    after_open: F,
    reader_ready: G,
) -> BuildIdEvidence
where
    F: FnOnce(&Path),
    G: FnOnce(&Path),
{
    let Some(compression) = ModuleCompression::from_path(path) else {
        return BuildIdEvidence::Invalid;
    };
    let Some((input, input_identity)) = trusted_packaged_module(path, require_root_owned) else {
        return BuildIdEvidence::Invalid;
    };
    after_open(path);
    let Ok(extraction_input) = input.try_clone() else {
        return BuildIdEvidence::Invalid;
    };
    let module = match module_bytes_with_hook(compression, extraction_input, || reader_ready(path))
    {
        Ok(Some(module)) => module,
        Ok(None) | Err(_) => return BuildIdEvidence::Invalid,
    };
    let build_id = elf_gnu_build_id_evidence(&module);
    let Ok(descriptor_metadata) = input.metadata() else {
        return BuildIdEvidence::Invalid;
    };
    let descriptor_after = FileIdentity::from_metadata(&descriptor_metadata);
    let Ok(path_after) = fs::symlink_metadata(path) else {
        return BuildIdEvidence::Invalid;
    };
    if descriptor_after != input_identity
        || FileIdentity::from_metadata(&path_after) != input_identity
        || !packaged_module_permissions_are_trusted(&path_after, require_root_owned)
    {
        return BuildIdEvidence::Invalid;
    }
    build_id
}

#[cfg(test)]
fn packaged_module_build_id_in_with_hooks<F, G>(
    path: &Path,
    require_root_owned: bool,
    after_open: F,
    reader_ready: G,
) -> Option<Vec<u8>>
where
    F: FnOnce(&Path),
    G: FnOnce(&Path),
{
    match packaged_module_build_id_evidence_in_with_hooks(
        path,
        require_root_owned,
        after_open,
        reader_ready,
    ) {
        BuildIdEvidence::Present(build_id) => Some(build_id),
        BuildIdEvidence::Absent | BuildIdEvidence::Invalid => None,
    }
}

#[cfg(test)]
fn packaged_module_build_id_in(path: &Path, require_root_owned: bool) -> Option<Vec<u8>> {
    packaged_module_build_id_in_with_hooks(path, require_root_owned, |_| {}, |_| {})
}

fn packaged_module_build_id_evidence(path: &Path) -> BuildIdEvidence {
    packaged_module_build_id_evidence_in_with_hooks(path, true, |_| {}, |_| {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::env;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn build_id_note_with_description_size(byte: u8, description_size: usize) -> Vec<u8> {
        let mut note = Vec::new();
        note.extend_from_slice(&4u32.to_le_bytes());
        note.extend_from_slice(&(description_size as u32).to_le_bytes());
        note.extend_from_slice(&3u32.to_le_bytes());
        note.extend_from_slice(b"GNU\0");
        note.extend(std::iter::repeat_n(byte, description_size));
        note.resize(12 + 4 + align_to(description_size, 4), 0);
        note
    }

    fn build_id_note(byte: u8) -> Vec<u8> {
        build_id_note_with_description_size(byte, 20)
    }

    fn test_directory() -> TestDirectory {
        for _ in 0..128 {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "galaxybook-setup-build-id-test-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return TestDirectory(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create test directory: {error}"),
            }
        }
        panic!("could not allocate a unique test directory")
    }

    fn set_mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .expect("could not set test mode");
    }

    fn write_file(path: &Path, content: &[u8], mode: u32) {
        if path.exists() {
            set_mode(path, 0o600);
        }
        fs::write(path, content).expect("could not write test file");
        set_mode(path, mode);
    }

    #[test]
    fn build_id_is_primary_and_srcversion_is_the_both_absent_fallback() {
        const SRCVERSION_A: &str = "0123456789ABCDEF0123456";
        const SRCVERSION_B: &str = "0123456789ABCDEF0123457";
        let called = Cell::new(false);
        assert!(!binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(SRCVERSION_A),
            || {
                called.set(true);
                (
                    BuildIdEvidence::Present(build_id_note(1)),
                    BuildIdEvidence::Present(build_id_note(2)),
                )
            },
        ));
        assert!(called.get(), "matching srcversion bypassed build-id proof");
        assert!(binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(SRCVERSION_B),
            || (
                BuildIdEvidence::Present(build_id_note(1)),
                BuildIdEvidence::Present(build_id_note(1)),
            ),
        ));
        assert!(!binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(SRCVERSION_B),
            || (BuildIdEvidence::Absent, BuildIdEvidence::Absent),
        ));
        assert!(binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(" \t0123456789ABCDEF0123456\t\n"),
            || (BuildIdEvidence::Absent, BuildIdEvidence::Absent),
        ));
        assert!(!binary_identity_matches_with(
            Some(SRCVERSION_A),
            None,
            || (BuildIdEvidence::Absent, BuildIdEvidence::Absent),
        ));
        assert!(!binary_identity_matches_with(None, None, || (
            BuildIdEvidence::Absent,
            BuildIdEvidence::Absent,
        )));
        assert!(!binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(SRCVERSION_A),
            || (
                BuildIdEvidence::Present(build_id_note(1)),
                BuildIdEvidence::Absent,
            ),
        ));
        assert!(!binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(SRCVERSION_A),
            || (
                BuildIdEvidence::Absent,
                BuildIdEvidence::Present(build_id_note(1)),
            ),
        ));
        assert!(!binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(SRCVERSION_A),
            || (BuildIdEvidence::Invalid, BuildIdEvidence::Absent),
        ));
        assert!(!binary_identity_matches_with(
            Some(SRCVERSION_A),
            Some(SRCVERSION_A),
            || (BuildIdEvidence::Absent, BuildIdEvidence::Invalid),
        ));
    }

    #[test]
    fn srcversion_fallback_requires_the_canonical_linux_format() {
        const VALID: &str = "0123456789ABCDEF0123456";
        assert_eq!(canonical_srcversion(Some(VALID)), Some(VALID));
        assert_eq!(
            canonical_srcversion(Some(" \t0123456789ABCDEF0123456\t\r\n")),
            Some(VALID),
        );

        for invalid in [
            "",
            "ABC123",
            "0123456789ABCDEF012345",
            "0123456789ABCDEF01234567",
            "0123456789abcdef0123456",
            "0123456789ABCDEF012345G",
            "0123456789ABCDEF0123456\n0123456789ABCDEF0123456",
            "\n0123456789ABCDEF0123456",
            "0123456789ABCDEF0123456\n\n",
        ] {
            assert_eq!(
                canonical_srcversion(Some(invalid)),
                None,
                "accepted invalid srcversion {invalid:?}"
            );
            assert!(!binary_identity_matches_with(
                Some(invalid),
                Some(invalid),
                || (BuildIdEvidence::Absent, BuildIdEvidence::Absent),
            ));
        }
    }

    #[test]
    fn build_id_note_requires_at_least_128_bits_of_identity() {
        for description_size in [4, 15] {
            assert!(
                !valid_gnu_build_id_note(&build_id_note_with_description_size(1, description_size)),
                "accepted a {description_size}-byte build-id"
            );
        }
        for description_size in [16, 20, 64] {
            assert!(
                valid_gnu_build_id_note(&build_id_note_with_description_size(1, description_size)),
                "rejected a {description_size}-byte build-id"
            );
        }
        assert!(!valid_gnu_build_id_note(
            &build_id_note_with_description_size(1, 65)
        ));
    }

    #[test]
    fn sysfs_build_id_requires_trusted_ancestry_and_leaf_metadata() {
        assert!(build_id_note_permissions_are_trusted(
            0, 0, 0o100444, 1, 0o444, true
        ));
        assert!(!build_id_note_permissions_are_trusted(
            1000, 0, 0o100444, 1, 0o444, true
        ));
        assert!(!build_id_note_permissions_are_trusted(
            0, 0, 0o100644, 1, 0o444, true
        ));
        assert!(!build_id_note_permissions_are_trusted(
            0, 0, 0o100444, 2, 0o444, true
        ));

        let temporary = test_directory();
        let sys_root = temporary.path().join("sys");
        let module_root = sys_root.join("module");
        let module = module_root.join("ov02c10");
        let notes = module.join("notes");
        fs::create_dir_all(&notes).expect("could not create fake sysfs");
        set_mode(&sys_root, 0o555);
        set_mode(&module_root, 0o755);
        set_mode(&module, 0o755);
        set_mode(&notes, 0o755);
        let note = notes.join(GNU_BUILD_ID_SECTION);
        write_file(&note, &build_id_note(7), 0o444);

        let trusted = trusted_loaded_build_id_path(&sys_root, "ov02c10", false)
            .expect("valid fake sysfs rejected");
        assert_eq!(
            read_build_id_note(&trusted, 0o444, false),
            Some(build_id_note(7))
        );
        assert_eq!(
            loaded_module_build_id_evidence_in(&sys_root, "ov02c10", false),
            BuildIdEvidence::Present(build_id_note(7)),
        );
        assert!(trusted_loaded_build_id_path(&sys_root, "../ov02c10", false).is_none());

        let module_without_notes = module_root.join("without_notes");
        fs::create_dir(&module_without_notes).expect("could not create module without notes");
        set_mode(&module_without_notes, 0o755);
        assert_eq!(
            loaded_module_build_id_evidence_in(&sys_root, "without_notes", false),
            BuildIdEvidence::Absent,
        );

        let empty_notes = module_root.join("empty_notes").join("notes");
        fs::create_dir_all(&empty_notes).expect("could not create empty notes directory");
        set_mode(empty_notes.parent().expect("empty module missing"), 0o755);
        set_mode(&empty_notes, 0o755);
        assert_eq!(
            loaded_module_build_id_evidence_in(&sys_root, "empty_notes", false),
            BuildIdEvidence::Absent,
        );

        let untrusted_notes = module_root.join("untrusted_notes").join("notes");
        fs::create_dir_all(&untrusted_notes).expect("could not create untrusted notes directory");
        set_mode(
            untrusted_notes.parent().expect("untrusted module missing"),
            0o755,
        );
        set_mode(&untrusted_notes, 0o777);
        assert_eq!(
            loaded_module_build_id_evidence_in(&sys_root, "untrusted_notes", false),
            BuildIdEvidence::Invalid,
        );

        let replacement = notes.join("replacement");
        write_file(&replacement, &build_id_note(8), 0o444);
        let displaced = notes.join("displaced");
        assert!(
            read_build_id_note_with_hook(&note, 0o444, false, || {
                fs::rename(&note, &displaced).expect("could not displace build-id note");
                fs::rename(&replacement, &note).expect("could not swap build-id note");
            })
            .is_none(),
            "a rename-swapped sysfs note was accepted"
        );
        fs::rename(&displaced, &note).expect("could not restore build-id note");

        let hardlink = notes.join("hardlink");
        fs::hard_link(&note, &hardlink).expect("could not create hardlink");
        assert!(read_build_id_note(&note, 0o444, false).is_none());
        fs::remove_file(&hardlink).expect("could not remove hardlink");

        let target = notes.join("target");
        write_file(&target, &build_id_note(7), 0o444);
        let symlink = notes.join("symlink");
        std::os::unix::fs::symlink(&target, &symlink).expect("could not create symlink");
        assert!(read_build_id_note(&symlink, 0o444, false).is_none());

        write_file(
            &note,
            &vec![0u8; MAX_BUILD_ID_NOTE_SIZE as usize + 1],
            0o444,
        );
        assert!(read_build_id_note(&note, 0o444, false).is_none());
        assert!(read_build_id_note(&notes.join("missing"), 0o444, false).is_none());

        set_mode(&sys_root, 0o755);
        assert!(trusted_loaded_build_id_path(&sys_root, "ov02c10", false).is_none());
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn align_to(value: usize, alignment: usize) -> usize {
        value.div_ceil(alignment) * alignment
    }

    fn elf_fixture(note: &[u8], include_build_id: bool) -> Vec<u8> {
        let build_id_name = if include_build_id {
            b".note.gnu.build-id".as_slice()
        } else {
            b".note.not-build-id".as_slice()
        };
        let mut names = vec![0];
        let build_id_name_offset = names.len();
        names.extend_from_slice(build_id_name);
        names.push(0);
        let string_table_name_offset = names.len();
        names.extend_from_slice(b".shstrtab\0");

        let note_offset = ELF64_HEADER_SIZE;
        let names_offset = align_to(note_offset + note.len(), 8);
        let section_table_offset = align_to(names_offset + names.len(), 8);
        let mut elf = vec![0u8; section_table_offset + 3 * ELF64_SECTION_HEADER_SIZE];
        elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        write_u16(&mut elf, 16, 1);
        write_u16(&mut elf, 18, 62);
        write_u32(&mut elf, 20, 1);
        write_u64(&mut elf, 40, section_table_offset as u64);
        write_u16(&mut elf, 52, ELF64_HEADER_SIZE as u16);
        write_u16(&mut elf, 58, ELF64_SECTION_HEADER_SIZE as u16);
        write_u16(&mut elf, 60, 3);
        write_u16(&mut elf, 62, 2);
        elf[note_offset..note_offset + note.len()].copy_from_slice(note);
        elf[names_offset..names_offset + names.len()].copy_from_slice(&names);

        let note_header = section_table_offset + ELF64_SECTION_HEADER_SIZE;
        write_u32(&mut elf, note_header, build_id_name_offset as u32);
        write_u32(&mut elf, note_header + 4, 7);
        write_u64(&mut elf, note_header + 8, 2);
        write_u64(&mut elf, note_header + 24, note_offset as u64);
        write_u64(&mut elf, note_header + 32, note.len() as u64);
        write_u64(&mut elf, note_header + 48, 4);

        let names_header = section_table_offset + 2 * ELF64_SECTION_HEADER_SIZE;
        write_u32(&mut elf, names_header, string_table_name_offset as u32);
        write_u32(&mut elf, names_header + 4, 3);
        write_u64(&mut elf, names_header + 24, names_offset as u64);
        write_u64(&mut elf, names_header + 32, names.len() as u64);
        write_u64(&mut elf, names_header + 48, 1);
        elf
    }

    fn copy_elf_fixture(path: &Path) {
        fs::write(path, elf_fixture(&build_id_note(9), true)).expect("could not write ELF fixture");
        set_mode(path, 0o644);
    }

    fn elf_section_table_offset(bytes: &[u8]) -> usize {
        usize::try_from(elf_u64(bytes, 40).expect("fixture section table offset missing"))
            .expect("fixture section table offset did not fit usize")
    }

    fn malformed_elf_variants() -> Vec<(&'static str, Vec<u8>)> {
        let fixture = elf_fixture(&build_id_note(9), true);
        let section_table_offset = elf_section_table_offset(&fixture);
        let note_header = section_table_offset + ELF64_SECTION_HEADER_SIZE;
        let names_header = section_table_offset + 2 * ELF64_SECTION_HEADER_SIZE;
        let note_offset = ELF64_HEADER_SIZE;
        let mut variants = Vec::new();

        let mut invalid_section_zero_type = fixture.clone();
        write_u32(&mut invalid_section_zero_type, section_table_offset + 4, 1);
        variants.push(("non-NULL section zero", invalid_section_zero_type));

        let mut invalid_section_zero_size = fixture.clone();
        write_u64(&mut invalid_section_zero_size, section_table_offset + 32, 1);
        variants.push(("nonzero section zero size", invalid_section_zero_size));

        let mut invalid_section_zero_address = fixture.clone();
        write_u64(
            &mut invalid_section_zero_address,
            section_table_offset + 16,
            1,
        );
        variants.push(("nonzero section zero address", invalid_section_zero_address));

        variants.push((
            "truncated ELF header",
            fixture[..ELF64_HEADER_SIZE - 1].to_vec(),
        ));

        let mut truncated_table = fixture.clone();
        truncated_table.truncate(truncated_table.len() - 1);
        variants.push(("truncated section table", truncated_table));

        let mut overflowing_table_offset = fixture.clone();
        write_u64(&mut overflowing_table_offset, 40, u64::MAX - 31);
        variants.push(("overflowing section table offset", overflowing_table_offset));

        let mut overflowing_section_range = fixture.clone();
        write_u64(
            &mut overflowing_section_range,
            note_header + 24,
            u64::MAX - 7,
        );
        write_u64(&mut overflowing_section_range, note_header + 32, 16);
        variants.push(("overflowing note section range", overflowing_section_range));

        let mut extended_section_count = fixture.clone();
        write_u16(&mut extended_section_count, 60, 0);
        variants.push(("extended section count", extended_section_count));

        let mut extended_name_index_out_of_bounds = fixture.clone();
        write_u16(&mut extended_name_index_out_of_bounds, 62, 0xffff);
        write_u32(
            &mut extended_name_index_out_of_bounds,
            section_table_offset + 40,
            3,
        );
        variants.push((
            "extended section-name index",
            extended_name_index_out_of_bounds,
        ));

        let mut overflowing_non_target = fixture.clone();
        overflowing_non_target
            .extend_from_within(names_header..names_header + ELF64_SECTION_HEADER_SIZE);
        let non_target_header = section_table_offset + 3 * ELF64_SECTION_HEADER_SIZE;
        write_u32(&mut overflowing_non_target, non_target_header, 0);
        write_u32(&mut overflowing_non_target, non_target_header + 4, 1);
        write_u64(
            &mut overflowing_non_target,
            non_target_header + 24,
            u64::MAX,
        );
        write_u64(&mut overflowing_non_target, non_target_header + 32, 1);
        write_u16(&mut overflowing_non_target, 60, 4);
        variants.push((
            "overflowing non-target PROGBITS section",
            overflowing_non_target,
        ));

        let mut invalid_non_target_name = fixture.clone();
        invalid_non_target_name
            .extend_from_within(names_header..names_header + ELF64_SECTION_HEADER_SIZE);
        let name_table_size = elf_u64(&fixture, names_header + 32).expect("name table size");
        write_u32(
            &mut invalid_non_target_name,
            non_target_header,
            name_table_size as u32,
        );
        write_u32(&mut invalid_non_target_name, non_target_header + 4, 1);
        write_u64(&mut invalid_non_target_name, non_target_header + 24, 0);
        write_u64(&mut invalid_non_target_name, non_target_header + 32, 0);
        write_u16(&mut invalid_non_target_name, 60, 4);
        variants.push((
            "out-of-bounds non-target section name",
            invalid_non_target_name,
        ));

        let mut duplicate_build_id = fixture.clone();
        duplicate_build_id.extend_from_within(note_header..note_header + ELF64_SECTION_HEADER_SIZE);
        write_u16(&mut duplicate_build_id, 60, 4);
        variants.push(("duplicate build-id section", duplicate_build_id));

        let mut missing_build_id = fixture.clone();
        write_u32(&mut missing_build_id, note_header, u32::MAX);
        variants.push(("missing build-id section", missing_build_id));

        let mut wrong_section_type = fixture.clone();
        write_u32(&mut wrong_section_type, note_header + 4, 1);
        variants.push(("wrong build-id section type", wrong_section_type));

        let mut wrong_section_flags = fixture.clone();
        write_u64(&mut wrong_section_flags, note_header + 8, 0);
        variants.push(("wrong build-id section flags", wrong_section_flags));

        let mut wrong_section_alignment = fixture.clone();
        write_u64(&mut wrong_section_alignment, note_header + 48, 8);
        variants.push(("wrong build-id section alignment", wrong_section_alignment));

        let mut wrong_name_table_type = fixture.clone();
        write_u32(&mut wrong_name_table_type, names_header + 4, 1);
        variants.push(("wrong section-name table type", wrong_name_table_type));

        let mut overflowing_name_table = fixture.clone();
        write_u64(&mut overflowing_name_table, names_header + 24, u64::MAX);
        variants.push(("out-of-bounds section-name table", overflowing_name_table));

        let mut invalid_note_name_size = fixture.clone();
        write_u32(&mut invalid_note_name_size, note_offset, 5);
        variants.push(("invalid build-id note name size", invalid_note_name_size));

        let mut invalid_note_description_size = fixture.clone();
        write_u32(&mut invalid_note_description_size, note_offset + 4, 3);
        variants.push((
            "invalid build-id note description size",
            invalid_note_description_size,
        ));

        let mut overflowing_note_description_size = fixture.clone();
        write_u32(
            &mut overflowing_note_description_size,
            note_offset + 4,
            u32::MAX,
        );
        variants.push((
            "overflowing build-id note description size",
            overflowing_note_description_size,
        ));

        let mut invalid_note_type = fixture.clone();
        write_u32(&mut invalid_note_type, note_offset + 8, 4);
        variants.push(("invalid build-id note type", invalid_note_type));

        let mut invalid_note_name = fixture.clone();
        invalid_note_name[note_offset + 12..note_offset + 16].copy_from_slice(b"BAD\0");
        variants.push(("invalid build-id note name", invalid_note_name));

        let mut trailing_note_bytes = fixture;
        write_u64(
            &mut trailing_note_bytes,
            note_header + 32,
            build_id_note(9).len() as u64 + 4,
        );
        variants.push(("trailing build-id note bytes", trailing_note_bytes));

        variants
    }

    fn compress_fixture(source: &Path, output: &Path, command: &str, arguments: &[&str]) {
        let input = File::open(source).expect("could not open ELF fixture");
        let output_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(output)
            .expect("could not create compressed fixture");
        let args = arguments.iter().map(OsStr::new).collect::<Vec<_>>();
        let status = bounded_command(command, &args)
            .stdin(Stdio::from(input))
            .stdout(Stdio::from(output_file))
            .stderr(Stdio::null())
            .status()
            .expect("could not compress ELF fixture");
        assert!(status.success(), "fixture compressor failed");
    }

    #[test]
    fn packaged_build_id_supports_all_kernel_module_compressions() {
        let temporary = test_directory();
        let fixture = temporary.path().join("fixture.ko");
        copy_elf_fixture(&fixture);
        let expected =
            packaged_module_build_id_in(&fixture, false).expect("uncompressed build-id missing");

        let (_, xz_arguments) = ModuleCompression::Xz.command().expect("xz command missing");
        assert!(xz_arguments.contains(&"--threads=1"));
        assert!(xz_arguments.contains(&XZ_MEMORY_LIMIT_ARGUMENT));
        assert!(xz_arguments.contains(&XZ_MT_MEMORY_LIMIT_ARGUMENT));
        assert!(!xz_arguments.contains(&"--single-stream"));
        let (_, zstd_arguments) = ModuleCompression::Zstd
            .command()
            .expect("zstd command missing");
        assert!(zstd_arguments.contains(&ZSTD_MEMORY_LIMIT_ARGUMENT));
        assert!(zstd_arguments.contains(&"-T1"));

        let variants = [
            (
                "fixture-xz.ko.xz",
                XZ_COMMAND,
                &["--compress", "--stdout"][..],
            ),
            ("fixture-gzip.ko.gz", GZIP_COMMAND, &["--stdout"][..]),
            (
                "fixture-zstd.ko.zst",
                ZSTD_COMMAND,
                &["--compress", "--stdout", "--quiet"][..],
            ),
        ];
        for (name, command, arguments) in variants {
            let compressed = temporary.path().join(name);
            compress_fixture(&fixture, &compressed, command, arguments);
            assert_eq!(
                packaged_module_build_id_in(&compressed, false),
                Some(expected.clone()),
                "build-id differed for {name}"
            );
        }

        let trailing_xz = temporary.path().join("trailing-garbage.ko.xz");
        compress_fixture(
            &fixture,
            &trailing_xz,
            XZ_COMMAND,
            &["--compress", "--stdout"],
        );
        let mut corrupted_xz = fs::read(&trailing_xz).expect("could not read xz fixture");
        corrupted_xz.extend_from_slice(b"trailing garbage");
        write_file(&trailing_xz, &corrupted_xz, 0o644);
        assert!(
            packaged_module_build_id_in(&trailing_xz, false).is_none(),
            "xz trailing garbage was accepted"
        );

        let missing = temporary.path().join("missing-build-id.ko");
        fs::write(&missing, elf_fixture(&build_id_note(9), false))
            .expect("could not write missing-build-id fixture");
        set_mode(&missing, 0o644);
        assert!(packaged_module_build_id_in(&missing, false).is_none());
        assert_eq!(
            packaged_module_build_id_evidence_in_with_hooks(&missing, false, |_| {}, |_| {},),
            BuildIdEvidence::Absent,
        );
    }

    #[test]
    fn elf_parser_rejects_malformed_or_unsupported_note_containers() {
        for (description, malformed) in malformed_elf_variants() {
            assert_eq!(elf_gnu_build_id(&malformed), None, "accepted {description}");
        }

        let expected = build_id_note(9);
        let mut nobits = elf_fixture(&expected, true);
        let section_table_offset = elf_section_table_offset(&nobits);
        let names_header = section_table_offset + 2 * ELF64_SECTION_HEADER_SIZE;
        nobits.extend_from_within(names_header..names_header + ELF64_SECTION_HEADER_SIZE);
        let nobits_header = section_table_offset + 3 * ELF64_SECTION_HEADER_SIZE;
        write_u32(&mut nobits, nobits_header, 0);
        write_u32(&mut nobits, nobits_header + 4, ELF_SHT_NOBITS);
        write_u64(&mut nobits, nobits_header + 24, u64::MAX);
        write_u64(&mut nobits, nobits_header + 32, u64::MAX);
        write_u16(&mut nobits, 60, 4);
        assert_eq!(elf_gnu_build_id(&nobits), Some(expected));
    }

    #[test]
    fn decompression_is_bounded_and_nonzero_exit_is_rejected_without_temp_files() {
        let temporary = test_directory();
        let oversized_source = temporary.path().join("oversized.raw");
        let source_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&oversized_source)
            .expect("could not create sparse oversized source");
        source_file
            .set_len(MAX_UNCOMPRESSED_MODULE_SIZE + 1)
            .expect("could not size sparse oversized source");
        let oversized_compressed = temporary.path().join("oversized.ko.gz");
        compress_fixture(
            &oversized_source,
            &oversized_compressed,
            GZIP_COMMAND,
            &["--stdout"],
        );

        let invalid_stream = temporary.path().join("invalid.ko.zst");
        write_file(&invalid_stream, b"not a zstd stream", 0o644);
        let before = fs::read_dir(temporary.path())
            .expect("could not list test directory")
            .count();
        assert!(packaged_module_build_id_in(&oversized_compressed, false).is_none());
        assert!(packaged_module_build_id_in(&invalid_stream, false).is_none());
        assert_eq!(
            packaged_module_build_id_evidence_in_with_hooks(&invalid_stream, false, |_| {}, |_| {},),
            BuildIdEvidence::Invalid,
        );
        let after = fs::read_dir(temporary.path())
            .expect("could not list test directory after decompression failures")
            .count();
        assert_eq!(before, after, "module proof created a named temporary file");
    }

    #[test]
    fn elf_parser_rejects_malformed_overflowing_and_duplicate_sections() {
        let note = build_id_note(9);
        let valid = elf_fixture(&note, true);
        assert_eq!(elf_gnu_build_id(&valid), Some(note.clone()));
        assert!(elf_gnu_build_id(&valid[..ELF64_HEADER_SIZE - 1]).is_none());

        let mut wrong_class = valid.clone();
        wrong_class[4] = 1;
        assert!(elf_gnu_build_id(&wrong_class).is_none());

        let mut wrong_machine = valid.clone();
        write_u16(&mut wrong_machine, 18, 183);
        assert!(elf_gnu_build_id(&wrong_machine).is_none());

        let mut overflowing_table = valid.clone();
        write_u64(&mut overflowing_table, 40, u64::MAX - 32);
        assert!(elf_gnu_build_id(&overflowing_table).is_none());

        let section_table_offset = elf_u64(&valid, 40).expect("fixture section table") as usize;
        let mut overflowing_note = valid.clone();
        write_u64(
            &mut overflowing_note,
            section_table_offset + ELF64_SECTION_HEADER_SIZE + 24,
            u64::MAX - 8,
        );
        assert!(elf_gnu_build_id(&overflowing_note).is_none());

        let mut too_many_sections = valid.clone();
        write_u16(&mut too_many_sections, 60, (MAX_ELF_SECTIONS + 1) as u16);
        assert!(elf_gnu_build_id(&too_many_sections).is_none());

        let mut duplicate = valid.clone();
        let note_header = duplicate[section_table_offset + ELF64_SECTION_HEADER_SIZE
            ..section_table_offset + 2 * ELF64_SECTION_HEADER_SIZE]
            .to_vec();
        duplicate.extend_from_slice(&note_header);
        write_u16(&mut duplicate, 60, 4);
        assert!(elf_gnu_build_id(&duplicate).is_none());
    }

    #[test]
    fn elf_parser_rejects_extended_section_indexes() {
        let note = build_id_note(9);
        let mut extended = elf_fixture(&note, true);
        let section_table_offset = elf_u64(&extended, 40).expect("fixture section table") as usize;
        write_u16(&mut extended, 60, 0);
        assert!(elf_gnu_build_id(&extended).is_none());

        let mut extended = elf_fixture(&note, true);
        write_u16(&mut extended, 62, 0xffff);
        write_u32(&mut extended, section_table_offset + 40, 2);
        assert!(elf_gnu_build_id(&extended).is_none());
    }

    #[test]
    fn extraction_rejects_symlinks_hardlinks_and_cleans_up_after_failure() {
        let temporary = test_directory();
        let fixture = temporary.path().join("fixture.ko");
        copy_elf_fixture(&fixture);
        let symlink = temporary.path().join("symlink.ko");
        std::os::unix::fs::symlink(&fixture, &symlink).expect("could not create fixture symlink");
        assert!(packaged_module_build_id_in(&symlink, false).is_none());

        let hardlink = temporary.path().join("hardlink.ko");
        fs::hard_link(&fixture, &hardlink).expect("could not create fixture hardlink");
        assert!(packaged_module_build_id_in(&fixture, false).is_none());
        fs::remove_file(&hardlink).expect("could not remove fixture hardlink");

        let replacement = temporary.path().join("replacement.ko");
        copy_elf_fixture(&replacement);
        let displaced = temporary.path().join("displaced.ko");
        assert!(
            packaged_module_build_id_in_with_hooks(
                &fixture,
                false,
                |_| {
                    fs::rename(&fixture, &displaced).expect("could not displace module fixture");
                    fs::rename(&replacement, &fixture).expect("could not swap module fixture");
                },
                |_| {},
            )
            .is_none(),
            "a rename-swapped packaged module was accepted"
        );
        fs::rename(&displaced, &fixture).expect("could not restore module fixture");

        let invalid = temporary.path().join("invalid.ko.zst");
        write_file(&invalid, b"not a zstd stream", 0o644);
        let before = fs::read_dir(temporary.path())
            .expect("could not list test directory")
            .count();
        assert!(packaged_module_build_id_in(&invalid, false).is_none());
        let after = fs::read_dir(temporary.path())
            .expect("could not list test directory after failure")
            .count();
        assert_eq!(before, after, "private extraction directory leaked");

        let oversized = temporary.path().join("oversized.ko");
        let oversized_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&oversized)
            .expect("could not create oversized module");
        oversized_file
            .set_len(MAX_PACKAGED_MODULE_SIZE + 1)
            .expect("could not size oversized module");
        assert!(packaged_module_build_id_in(&oversized, false).is_none());
    }

    #[test]
    fn compressed_input_mutation_after_spawn_is_rejected_without_temp_files() {
        let temporary = test_directory();
        let fixture = temporary.path().join("fixture.ko");
        copy_elf_fixture(&fixture);
        let compressed = temporary.path().join("fixture.ko.xz");
        compress_fixture(
            &fixture,
            &compressed,
            XZ_COMMAND,
            &["--compress", "--stdout"],
        );
        let before = fs::read_dir(temporary.path())
            .expect("could not list test directory")
            .count();

        assert!(
            packaged_module_build_id_in_with_hooks(
                &compressed,
                false,
                |_| {},
                |path| {
                    fs::write(
                        path,
                        b"mutated while the decompressor owns a cloned descriptor",
                    )
                    .expect("could not mutate compressed input");
                },
            )
            .is_none(),
            "a compressed module mutated during decompression was accepted"
        );
        let after = fs::read_dir(temporary.path())
            .expect("could not list test directory after mutation")
            .count();
        assert_eq!(before, after, "module proof created a named temporary file");
    }

    #[test]
    fn hostile_build_id_note_paths_cannot_influence_the_in_memory_parser() {
        let temporary = test_directory();
        let fixture = temporary.path().join("fixture.ko");
        copy_elf_fixture(&fixture);
        let hostile = temporary.path().join("build-id.note");
        let displaced = temporary.path().join("displaced-build-id.note");
        let replacement = temporary.path().join("replacement-build-id.note");
        write_file(&hostile, &build_id_note(1), 0o600);
        write_file(&replacement, &build_id_note(2), 0o600);
        let before = fs::read_dir(temporary.path())
            .expect("could not list test directory")
            .count();

        let actual = packaged_module_build_id_in_with_hooks(
            &fixture,
            false,
            |_| {},
            |_| {
                fs::rename(&hostile, &displaced).expect("could not displace hostile build-id note");
                fs::rename(&replacement, &hostile).expect("could not swap hostile build-id note");
            },
        );
        assert_eq!(actual, Some(build_id_note(9)));
        let after = fs::read_dir(temporary.path())
            .expect("could not list test directory after hostile swaps")
            .count();
        assert_eq!(before, after, "module proof created a named temporary file");
    }

    #[test]
    fn live_loaded_modules_match_their_resolved_payloads_when_available() {
        let mut checked = 0usize;
        for (modinfo_name, proc_name) in [
            ("ov02c10", "ov02c10"),
            ("snd-hda-scodec-max98390", "snd_hda_scodec_max98390"),
            ("snd-hda-scodec-max98390-i2c", "snd_hda_scodec_max98390_i2c"),
        ] {
            if !Path::new(&format!("/sys/module/{proc_name}")).is_dir() {
                continue;
            }
            let Ok(path_output) = Command::new("/usr/sbin/modinfo")
                .args(["-n", modinfo_name])
                .output()
            else {
                continue;
            };
            if !path_output.status.success() {
                continue;
            }
            let path = String::from_utf8(path_output.stdout)
                .expect("module path was not UTF-8")
                .trim()
                .to_string();
            let loaded_srcversion =
                fs::read_to_string(format!("/sys/module/{proc_name}/srcversion")).ok();
            let candidate_output = Command::new("/usr/sbin/modinfo")
                .args(["-F", "srcversion", &path])
                .output()
                .ok();
            let candidate_srcversion = candidate_output.as_ref().and_then(|output| {
                output
                    .status
                    .success()
                    .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
            });
            assert!(
                loaded_module_binary_matches_candidate(
                    proc_name,
                    &path,
                    loaded_srcversion.as_deref(),
                    candidate_srcversion.as_deref(),
                ),
                "loaded module did not match its resolved payload: {modinfo_name}"
            );
            checked += 1;
        }

        if let Ok(expected) = env::var("GALAXYBOOK_EXPECT_LIVE_MODULES") {
            assert_eq!(
                checked,
                expected
                    .parse::<usize>()
                    .expect("invalid live module count"),
                "unexpected number of available live module proofs"
            );
        }
    }
}
