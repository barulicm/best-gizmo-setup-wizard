use anyhow::{bail, Result};

pub fn run_command(command: &str) -> Result<std::process::Output> {
    let output = std::process::Command::new("powershell")
        .arg("-Command")
        .arg(command)
        .output()?;
    if !output.status.success() {
        bail!(
            "Running powershell command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output)
}
