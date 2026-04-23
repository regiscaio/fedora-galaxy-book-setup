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

    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    if let Some(input) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
        }
    }

    let output = child.wait_with_output()?;
    Ok(collect_command_output(output))
}

pub(crate) fn execute_privileged_shell_command(
    command: &str,
) -> Result<PrivilegedCommandResult, String> {
    run_shell_command_with_optional_input(
        "pkexec",
        &[
            "/usr/bin/env",
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
            "/usr/bin/bash",
            "-lc",
            command,
        ],
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
        "pkexec",
        &[
            "/usr/bin/env",
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
            "/usr/bin/bash",
            "-lc",
            command,
        ],
        Some(stdin_data),
    )
    .map_err(|error| {
        trf(
            "Falha ao iniciar a ação privilegiada: {error}",
            &[("error", error.to_string())],
        )
    })
}

pub(crate) fn execute_user_shell_command(
    command: &str,
) -> Result<PrivilegedCommandResult, String> {
    run_shell_command_with_optional_input("/usr/bin/bash", &["-lc", command], None)
        .map_err(|error| {
            trf(
                "Falha ao iniciar a ação local: {error}",
                &[("error", error.to_string())],
            )
        })
}
