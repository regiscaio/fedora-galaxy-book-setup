use std::io::Write;
use std::process::{Command, Stdio};

use galaxybook_setup::trf;

pub(crate) struct PrivilegedCommandResult {
    pub(crate) output: String,
    pub(crate) success: bool,
}

fn collect_command_output(output: std::process::Output) -> PrivilegedCommandResult {
    let mut combined = String::new();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stdout.is_empty() {
        combined.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push_str("\n\n");
        }
        combined.push_str(&stderr);
    }

    PrivilegedCommandResult {
        output: combined,
        success: output.status.success(),
    }
}

fn run_shell_command_with_optional_input(
    program: &str,
    args: &[&str],
    stdin_data: Option<&str>,
) -> Result<PrivilegedCommandResult, std::io::Error> {
    let mut command = Command::new(program);
    command.args(args);
    if stdin_data.is_some() {
        command.stdin(Stdio::piped());
    }

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(input) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
        }
    }

    let output = child.wait_with_output()?;
    Ok(collect_command_output(output))
}

fn privileged_shell_arguments(command: &str) -> [&str; 7] {
    [
        "/usr/bin/env",
        "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
        "/usr/bin/bash",
        "--noprofile",
        "--norc",
        "-c",
        command,
    ]
}

fn user_shell_arguments(command: &str) -> [&str; 4] {
    ["--noprofile", "--norc", "-c", command]
}

pub(crate) fn execute_privileged_shell_command(
    command: &str,
) -> Result<PrivilegedCommandResult, String> {
    run_shell_command_with_optional_input(
        "/usr/bin/pkexec",
        &privileged_shell_arguments(command),
        None,
    )
    .map_err(|error| {
        trf(
            "Falha ao iniciar a ação privilegiada: {error}",
            &[("error", error.to_string())],
        )
    })
}

pub(crate) fn execute_privileged_shell_command_with_input(
    command: &str,
    stdin_data: &str,
) -> Result<PrivilegedCommandResult, String> {
    run_shell_command_with_optional_input(
        "/usr/bin/pkexec",
        &privileged_shell_arguments(command),
        Some(stdin_data),
    )
    .map_err(|error| {
        trf(
            "Falha ao iniciar a ação privilegiada: {error}",
            &[("error", error.to_string())],
        )
    })
}

pub(crate) fn execute_user_shell_command(command: &str) -> Result<PrivilegedCommandResult, String> {
    run_shell_command_with_optional_input("/usr/bin/bash", &user_shell_arguments(command), None)
        .map_err(|error| {
            trf(
                "Falha ao iniciar a ação local: {error}",
                &[("error", error.to_string())],
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{
        privileged_shell_arguments, run_shell_command_with_optional_input, user_shell_arguments,
    };

    #[test]
    fn shell_actions_do_not_load_login_or_profile_files() {
        let privileged = privileged_shell_arguments("true");
        let user = user_shell_arguments("true");

        assert_eq!(
            privileged,
            [
                "/usr/bin/env",
                "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
                "/usr/bin/bash",
                "--noprofile",
                "--norc",
                "-c",
                "true",
            ]
        );
        assert_eq!(user, ["--noprofile", "--norc", "-c", "true"]);
        assert!(!privileged.contains(&"-l"));
        assert!(!user.contains(&"-l"));
    }

    #[test]
    fn privileged_input_is_sent_only_through_stdin() {
        let sensitive_input = "fixture-only-sensitive-value";
        let command = "IFS= read -r value; test ${#value} -eq 28";
        let arguments = privileged_shell_arguments(command);
        assert!(
            arguments
                .iter()
                .all(|argument| !argument.contains(sensitive_input))
        );

        let result = run_shell_command_with_optional_input(
            "/usr/bin/bash",
            &["--noprofile", "--norc", "-c", command],
            Some(sensitive_input),
        )
        .expect("não foi possível testar transporte por stdin");
        assert!(result.success);
        assert!(!result.output.contains(sensitive_input));
    }
}
