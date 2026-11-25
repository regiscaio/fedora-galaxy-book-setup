use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn resolve_git_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
    let dot_git = manifest_dir.join(".git");

    if dot_git.is_dir() {
        return Some(dot_git);
    }

    let pointer = fs::read_to_string(&dot_git).ok()?;
    let gitdir = pointer.trim().strip_prefix("gitdir: ")?;
    let gitdir_path = Path::new(gitdir);

    if gitdir_path.is_absolute() {
        Some(gitdir_path.to_path_buf())
    } else {
        Some(manifest_dir.join(gitdir_path))
    }
}

fn configure_git_rerun() {
    let Some(git_dir) = resolve_git_dir() else {
        return;
    };

    let head_path = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());

    if let Ok(head) = fs::read_to_string(&head_path) {
        if let Some(reference) = head.trim().strip_prefix("ref: ") {
            let ref_path = git_dir.join(reference);
            println!("cargo:rerun-if-changed={}", ref_path.display());
        }
    }
}

fn git_version() -> Option<String> {
    resolve_git_dir()?;
    let output = Command::new("./scripts/package-version.sh").output().ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-env-changed=APP_VERSION_OVERRIDE");
    println!("cargo:rerun-if-env-changed=VERSION_COMMIT_COUNT");
    println!("cargo:rerun-if-env-changed=VERSION_MAJOR");
    println!("cargo:rerun-if-changed=scripts/package-version.sh");
    configure_git_rerun();

    let version = env::var("APP_VERSION_OVERRIDE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(git_version)
        .unwrap_or_else(|| env::var("CARGO_PKG_VERSION").expect("missing CARGO_PKG_VERSION"));

    println!("cargo:rustc-env=APP_VERSION={version}");
}
