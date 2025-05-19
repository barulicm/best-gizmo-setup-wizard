use anyhow::{Result, bail};

#[cfg(target_os = "windows")]
pub fn run_powershell_command(command: &str) -> Result<std::process::Output> {
    let mut c = std::process::Command::new("powershell");
    c.arg("-Command").arg(command);
    run_command(c)
}

#[cfg(target_os = "linux")]
pub fn run_bash_command(command: &str) -> Result<std::process::Output> {
    let mut c = std::process::Command::new("bash");
    c.arg("-c").arg(command);
    run_command(c)
}

#[cfg(target_os = "linux")]
pub fn run_admin_bash_command(command: &str) -> Result<std::process::Output> {
    let mut c = std::process::Command::new("pkexec");
    c.arg("bash").arg("-c").arg(command);
    run_command(c)
}

fn run_command(mut command: std::process::Command) -> Result<std::process::Output> {
    let output = command.output()?;
    if !output.status.success() {
        bail!(
            "Running shell command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output)
}
