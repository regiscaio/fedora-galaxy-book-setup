use std::process::Command;

pub(crate) struct PrivilegedCommandResult {
    pub(crate) output: String,
    pub(crate) success: bool,
}

pub(crate) fn execute_privileged_shell_command(
    command: &str,
) -> Result<PrivilegedCommandResult, String> {
    let output = Command::new("pkexec")
        .arg("/usr/bin/env")
        .arg("PATH=/usr/sbin:/usr/bin:/sbin:/bin")
        .arg("/usr/bin/bash")
        .arg("-lc")
        .arg(command)
        .output()
        .map_err(|error| format!("Falha ao iniciar a ação privilegiada: {error}"))?;

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

    Ok(PrivilegedCommandResult {
        output: combined,
        success: output.status.success(),
    })
}
