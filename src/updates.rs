use std::ffi::OsString;
use std::process::{Command, Output};

use crate::{tr, trf};

const DNF_COMMAND: &str = "/usr/bin/dnf";
const PKEXEC_COMMAND: &str = "/usr/bin/pkexec";
const RPM_COMMAND: &str = "/usr/bin/rpm";
const TIMEOUT_COMMAND: &str = "/usr/bin/timeout";
const TIMEOUT_KILL_AFTER: &str = "5s";
const DNF_QUERY_TIMEOUT: &str = "120s";
const RPM_QUERY_TIMEOUT: &str = "8s";

fn command_output_text(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(stdout));
    text.push_str(&String::from_utf8_lossy(stderr));
    text
}

fn fallback_command_error(output: String, fallback: &str) -> String {
    if output.trim().is_empty() {
        fallback.to_string()
    } else {
        output
    }
}

fn bounded_probe_arguments(command: &str, duration: &str, args: &[&str]) -> Vec<OsString> {
    let mut bounded_args = Vec::with_capacity(args.len() + 3);
    bounded_args.push(OsString::from(format!("--kill-after={TIMEOUT_KILL_AFTER}")));
    bounded_args.push(OsString::from(duration));
    bounded_args.push(OsString::from(command));
    bounded_args.extend(args.iter().map(OsString::from));
    bounded_args
}

fn bounded_probe_output(
    command: &str,
    duration: &str,
    args: &[&str],
) -> std::io::Result<std::process::Output> {
    Command::new(TIMEOUT_COMMAND)
        .env("LC_ALL", "C")
        .args(bounded_probe_arguments(command, duration, args))
        .output()
}

fn timeout_exit_code(code: Option<i32>) -> bool {
    matches!(code, Some(124 | 137))
}

fn command_error_with_context(output: String, context: &str) -> String {
    if output.trim().is_empty() {
        context.to_string()
    } else {
        format!("{context}\n{}", output.trim())
    }
}

fn parse_installed_package_names(output: &str, packages: &[&str]) -> Vec<String> {
    packages
        .iter()
        .filter(|package| output.lines().any(|line| line.trim() == **package))
        .map(|package| (*package).to_string())
        .collect()
}

fn rpm_inventory_is_complete(output: &str) -> bool {
    !output.is_empty()
        && output.ends_with('\n')
        && output.lines().all(|name| {
            !name.is_empty()
                && name.len() <= 255
                && name
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && name.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.' | b'_')
                })
        })
}

fn installed_package_names_from_output(
    output: &Output,
    packages: &[&str],
) -> Result<Vec<String>, String> {
    let exit_code = output.status.code();

    if output.status.success() && output.stderr.is_empty() {
        let stdout = std::str::from_utf8(&output.stdout).map_err(|_| {
            tr("O RPM falhou ao consultar os pacotes instalados. Verifique o banco de dados RPM e tente novamente.")
        })?;
        if !rpm_inventory_is_complete(stdout) {
            return Err(tr(
                "O RPM retornou um inventário vazio, truncado ou malformado. A consulta foi tratada como inconclusiva.",
            ));
        }
        return Ok(parse_installed_package_names(stdout, packages));
    }
    if timeout_exit_code(exit_code) {
        return Err(tr(
            "Tempo esgotado ao consultar os pacotes instalados via RPM. Verifique o gerenciador de pacotes e tente novamente.",
        ));
    }

    Err(command_error_with_context(
        command_output_text(&output.stdout, &output.stderr),
        &tr(
            "O RPM falhou ao consultar os pacotes instalados. Verifique o banco de dados RPM e tente novamente.",
        ),
    ))
}

fn package_update_names_from_output(
    output: &Output,
    installed_packages: &[&str],
) -> Result<Vec<String>, String> {
    let failure_context = tr(
        "O DNF falhou ao consultar atualizações. Verifique a conexão e os repositórios configurados e tente novamente.",
    );
    let exit_code = output.status.code();

    if timeout_exit_code(exit_code) {
        return Err(tr(
            "Tempo esgotado ao consultar atualizações via DNF. Verifique a conexão e os repositórios configurados e tente novamente.",
        ));
    }
    if !output.stderr.is_empty() {
        return Err(command_error_with_context(
            command_output_text(&output.stdout, &output.stderr),
            &failure_context,
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout).map_err(|_| failure_context.clone())?;

    match exit_code {
        Some(0) if stdout.trim().is_empty() => Ok(Vec::new()),
        Some(100) => {
            let names = parse_dnf_check_update_package_names(stdout, installed_packages);
            if names.is_empty() {
                Err(command_error_with_context(
                    stdout.to_string(),
                    &failure_context,
                ))
            } else {
                Ok(names)
            }
        }
        _ => Err(command_error_with_context(
            stdout.to_string(),
            &failure_context,
        )),
    }
}

fn dnf_package_token_matches(token: &str, package: &str) -> bool {
    if token == package {
        return true;
    }
    let Some(architecture) = token
        .strip_prefix(package)
        .and_then(|suffix| suffix.strip_prefix('.'))
    else {
        return false;
    };
    matches!(
        architecture,
        "x86_64" | "noarch" | "i686" | "aarch64" | "ppc64le" | "s390x" | "src"
    )
}

fn installed_package_names(packages: &[&str]) -> Result<Vec<String>, String> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }

    // Query the RPM database once. Unlike one `rpm -q` per package, this does
    // not conflate an absent package with a query failure that also exits 1.
    let output = bounded_probe_output(
        RPM_COMMAND,
        RPM_QUERY_TIMEOUT,
        &["-qa", "--qf", "%{NAME}\n"],
    )
    .map_err(|error| {
        trf(
            "Falha ao consultar os pacotes instalados via RPM: {error}",
            &[("error", error.to_string())],
        )
    })?;
    installed_package_names_from_output(&output, packages)
}

pub fn parse_dnf_check_update_package_names(output: &str, packages: &[&str]) -> Vec<String> {
    let mut found = Vec::new();

    for line in output.lines() {
        let first_column = line.split_whitespace().next().unwrap_or_default();
        if first_column.is_empty() {
            continue;
        }

        for package in packages {
            if dnf_package_token_matches(first_column, package)
                && !found.iter().any(|name| name == *package)
            {
                found.push((*package).to_string());
            }
        }
    }

    found
}

pub fn package_update_names(packages: &[&str]) -> Result<Vec<String>, String> {
    let installed_packages = installed_package_names(packages)?;
    if installed_packages.is_empty() {
        return Ok(Vec::new());
    }
    let installed_package_refs = installed_packages
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    let mut check_update_args = vec!["-q", "--refresh", "check-update"];
    check_update_args.extend(installed_package_refs.iter().copied());
    let output = bounded_probe_output(DNF_COMMAND, DNF_QUERY_TIMEOUT, &check_update_args).map_err(
        |error| {
            trf(
                "Falha ao consultar atualizações via DNF: {error}",
                &[("error", error.to_string())],
            )
        },
    )?;
    package_update_names_from_output(&output, &installed_package_refs)
}

pub fn install_package_updates(packages: &[&str]) -> Result<String, String> {
    let installed_packages = installed_package_names(packages)?;
    if installed_packages.is_empty() {
        return Ok(String::new());
    }
    let installed_package_refs = installed_packages
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    let output = Command::new(PKEXEC_COMMAND)
        .arg(DNF_COMMAND)
        .arg("--refresh")
        .arg("upgrade")
        .arg("-y")
        .args(&installed_package_refs)
        .output()
        .map_err(|error| {
            trf(
                "Falha ao iniciar a atualização com privilégios administrativos: {error}",
                &[("error", error.to_string())],
            )
        })?;
    let output_text = command_output_text(&output.stdout, &output.stderr);

    if output.status.success() {
        Ok(output_text)
    } else {
        Err(fallback_command_error(
            output_text,
            &tr("A atualização falhou, mas não retornou saída textual."),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn process_output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn parses_dnf4_check_update_lines() {
        let output = "\
galaxybook-setup.x86_64 1.1.0-1.fc44 caioregis
galaxybook-camera.x86_64 1.1.0-1.fc44 caioregis
akmod-galaxybook-ov02c10.x86_64 1.1.0-1.fc44 caioregis
kmod-galaxybook-ov02c10.noarch 1.1.0-1.fc44 caioregis
unrelated.x86_64 1.0-1 fedora
";
        assert_eq!(
            parse_dnf_check_update_package_names(
                output,
                &[
                    "galaxybook-setup",
                    "galaxybook-camera",
                    "galaxybook-sound",
                    "akmod-galaxybook-ov02c10",
                    "galaxybook-ov02c10-kmod-common",
                    "kmod-galaxybook-ov02c10",
                    "akmod-galaxybook-max98390",
                    "galaxybook-max98390-kmod-common",
                ],
            ),
            vec![
                "galaxybook-setup",
                "galaxybook-camera",
                "akmod-galaxybook-ov02c10",
                "kmod-galaxybook-ov02c10",
            ]
        );
    }

    #[test]
    fn parses_dnf5_table_lines() {
        let output = "\
Updating and loading repositories:
Repositories loaded.
Available upgrades
Name                          Arch   Version       Repository
galaxybook-max98390-kmod-common noarch 1.1.0-1.fc44 caioregis
kmod-galaxybook-ov02c10       noarch 1.1.0-1.fc44 caioregis
galaxybook-sound              x86_64 1.1.0-1.fc44 caioregis
";
        assert_eq!(
            parse_dnf_check_update_package_names(
                output,
                &[
                    "galaxybook-setup",
                    "galaxybook-camera",
                    "galaxybook-sound",
                    "akmod-galaxybook-ov02c10",
                    "galaxybook-ov02c10-kmod-common",
                    "kmod-galaxybook-ov02c10",
                    "akmod-galaxybook-max98390",
                    "galaxybook-max98390-kmod-common",
                ],
            ),
            vec![
                "galaxybook-max98390-kmod-common",
                "kmod-galaxybook-ov02c10",
                "galaxybook-sound",
            ]
        );
    }

    #[test]
    fn update_parser_rejects_near_prefixes_and_unrecognized_arch_suffixes() {
        let output = "\
galaxybook-camera.debug.x86_64 1.1.0-1.fc44 caioregis
galaxybook-camera-plugins.x86_64 1.1.0-1.fc44 caioregis
galaxybook-camera.unknown 1.1.0-1.fc44 caioregis
galaxybook-camera.x86_64 1.1.0-1.fc44 caioregis
galaxybook-setup.i686 1.1.0-1.fc44 caioregis
";
        assert_eq!(
            parse_dnf_check_update_package_names(
                output,
                &["galaxybook-camera", "galaxybook-setup"],
            ),
            vec!["galaxybook-camera", "galaxybook-setup"]
        );
        assert!(!dnf_package_token_matches(
            "galaxybook-camera.debug.x86_64",
            "galaxybook-camera",
        ));
    }

    #[test]
    fn update_probes_are_wrapped_in_bounded_timeouts() {
        let rpm_args = bounded_probe_arguments(
            RPM_COMMAND,
            RPM_QUERY_TIMEOUT,
            &["-qa", "--qf", "%{NAME}\n"],
        )
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
        assert_eq!(
            rpm_args,
            vec![
                "--kill-after=5s",
                "8s",
                "/usr/bin/rpm",
                "-qa",
                "--qf",
                "%{NAME}\n",
            ]
        );

        let dnf_args = bounded_probe_arguments(
            DNF_COMMAND,
            DNF_QUERY_TIMEOUT,
            &["-q", "--refresh", "check-update", "pkg"],
        )
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
        assert_eq!(
            dnf_args,
            vec![
                "--kill-after=5s",
                "120s",
                "/usr/bin/dnf",
                "-q",
                "--refresh",
                "check-update",
                "pkg",
            ]
        );
        assert_eq!(TIMEOUT_COMMAND, "/usr/bin/timeout");
    }

    #[test]
    fn installed_package_inventory_is_exact_and_preserves_requested_order() {
        let inventory = "galaxybook-camera\ngalaxybook-setup\nother-package\n";
        assert_eq!(
            parse_installed_package_names(
                inventory,
                &["galaxybook-setup", "galaxybook-sound", "galaxybook-camera"],
            ),
            vec!["galaxybook-setup", "galaxybook-camera"]
        );

        assert!(parse_installed_package_names("package-extra\n", &["package"]).is_empty());
    }

    #[test]
    fn installed_package_inventory_rejects_success_with_stderr() {
        let output = process_output(
            0,
            "galaxybook-setup\ngalaxybook-camera\n",
            "rpm database warning\n",
        );

        assert!(
            installed_package_names_from_output(
                &output,
                &["galaxybook-setup", "galaxybook-camera"],
            )
            .is_err()
        );
    }

    #[test]
    fn installed_package_inventory_rejects_empty_truncated_or_malformed_success() {
        let packages = ["galaxybook-setup"];

        for stdout in [
            "",
            "galaxybook-setup",
            "galaxybook-setup\ninvalid package\n",
            "-invalid\n",
            "galaxybook-setup\n\n",
        ] {
            assert!(
                installed_package_names_from_output(&process_output(0, stdout, ""), &packages)
                    .is_err(),
                "accepted inconclusive RPM inventory {stdout:?}",
            );
        }

        assert_eq!(
            installed_package_names_from_output(
                &process_output(0, "unrelated-package\n", ""),
                &packages,
            )
            .expect("a complete inventory may contain none of the requested packages"),
            Vec::<String>::new(),
        );
    }

    #[test]
    fn dnf_result_requires_conclusive_status_channels_and_output() {
        let packages = ["galaxybook-setup", "galaxybook-camera"];
        let valid_update = "galaxybook-camera.x86_64 1.1.0-1.fc44 updates\n";

        assert_eq!(
            package_update_names_from_output(&process_output(0, "", ""), &packages)
                .expect("empty successful DNF result should mean no updates"),
            Vec::<String>::new(),
        );
        assert_eq!(
            package_update_names_from_output(&process_output(0, " \n\t", ""), &packages)
                .expect("whitespace-only successful DNF result should mean no updates"),
            Vec::<String>::new(),
        );
        assert_eq!(
            package_update_names_from_output(&process_output(100, valid_update, ""), &packages)
                .expect("status 100 with an exact requested package should be accepted"),
            vec!["galaxybook-camera"],
        );

        for output in [
            process_output(0, "", "warning\n"),
            process_output(0, "unexpected output\n", ""),
            process_output(100, valid_update, "warning\n"),
            process_output(100, "", ""),
            process_output(100, "unrelated.x86_64 1.0-1 updates\n", ""),
        ] {
            assert!(package_update_names_from_output(&output, &packages).is_err());
        }

        let mut invalid_utf8 = process_output(100, "", "");
        invalid_utf8.stdout = vec![0xff, 0xfe];
        assert!(package_update_names_from_output(&invalid_utf8, &packages).is_err());
    }

    #[test]
    fn timeout_statuses_do_not_collide_with_dnf_update_status() {
        assert!(timeout_exit_code(Some(124)));
        assert!(timeout_exit_code(Some(137)));
        assert!(!timeout_exit_code(Some(0)));
        assert!(!timeout_exit_code(Some(1)));
        assert!(!timeout_exit_code(Some(100)));
        assert!(!timeout_exit_code(None));
    }
}
