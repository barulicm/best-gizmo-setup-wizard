use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct DriveInfo {
    pub drive_path: std::path::PathBuf,
    pub file_system_label: String,
}

impl PartialEq for DriveInfo {
    fn eq(&self, other: &Self) -> bool {
        self.drive_path == other.drive_path
    }
}

impl std::fmt::Display for DriveInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let drive_name = if self.file_system_label.is_empty() {
            String::from("unnamed")
        } else {
            self.file_system_label.clone()
        };
        write!(
            f,
            "{} ({})",
            drive_name,
            self.drive_path
                .to_str()
                .expect("Could not represent drive path as string")
        )
    }
}

#[cfg(target_os = "windows")]
impl DriveInfo {
    fn get_drive_letter(&self) -> Option<String> {
        match self.drive_path.components().next() {
            Some(std::path::Component::Prefix(component)) => match component.kind() {
                std::path::Prefix::Disk(letter) => String::from_utf8(vec![letter]).ok(),
                _ => None,
            },
            _ => None,
        }
    }
}

#[cfg(target_os = "windows")]
pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let powershell_command = "Get-Volume | Where-Object {$_.DriveType -eq 'Removable'} | Select-Object DriveLetter, FileSystemLabel | ConvertTo-Json";
    let output = crate::utils::shell::run_powershell_command(powershell_command)
        .with_context(|| "Running Get-Volume failed")?;
    let mut drive_info_str = String::from_utf8(output.stdout)?;
    if !drive_info_str.starts_with("[") {
        drive_info_str = String::from("[") + &drive_info_str + "]";
    }
    let json_val = serde_json::from_str::<serde_json::Value>(&drive_info_str)?;
    let mut result = vec![];
    if let serde_json::Value::Array(json_arr) = json_val {
        for item in json_arr {
            if let serde_json::Value::Object(json_obj) = item {
                result.push(DriveInfo {
                    drive_path: std::path::PathBuf::from(
                        json_obj["DriveLetter"]
                            .as_str()
                            .ok_or(anyhow!("Missing field DriveLetter in PowerShell output."))?
                            .to_string()
                            + ":\\",
                    ),
                    file_system_label: json_obj["FileSystemLabel"]
                        .as_str()
                        .ok_or(anyhow!(
                            "Missing field FileSystemLabel in PowerShell output."
                        ))?
                        .to_string(),
                });
            }
        }
    }
    Ok(result)
}

#[cfg(target_os = "windows")]
pub fn format_drive(drive: &DriveInfo, team_number: &str) -> Result<()> {
    let powershell_command = format!(
        "Format-Volume -DriveLetter {} -FileSystem FAT32 -NewFileSystemLabel 'GIZMO{}'",
        drive
            .get_drive_letter()
            .ok_or(anyhow!("Could not determine drive letter."))?,
        team_number
    );
    crate::utils::shell::run_powershell_command(&powershell_command)
        .with_context(|| "Running Format-Volume failed")?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn write_filesystem_cache(drive: &DriveInfo) -> Result<()> {
    let powershell_command = format!(
        "Write-VolumeCache -DriveLetter {}",
        drive
            .get_drive_letter()
            .ok_or(anyhow!("Could not determine drive letter."))?
    );
    crate::utils::shell::run_powershell_command(&powershell_command)
        .with_context(|| "Writing filesystem cache failed")?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let username = std::env::var("USER")?;
    let bash_command = format!("ls /media/{username}");
    let command_output = crate::utils::shell::run_bash_command(&bash_command)
        .with_context(|| "Listing removable drives failed.")?;
    let command_output = String::from_utf8(command_output.stdout)?;
    let mut drives = vec![];
    for line in command_output.lines() {
        drives.push(DriveInfo {
            drive_path: std::path::PathBuf::from(format!("/media/{username}/{line}")),
            file_system_label: line.to_string(),
        });
    }
    Ok(drives)
}

#[cfg(target_os = "linux")]
pub fn format_drive(drive: &DriveInfo, team_number: &str) -> Result<()> {
    let drive_path_str = drive
        .drive_path
        .to_str()
        .ok_or(anyhow!("Failed to convert disk path to string."))?;
    let block_device_path = {
        let cmd_output = crate::utils::shell::run_bash_command(
            format!("df {drive_path_str} | awk 'NR>1{{print $1}}'").as_str(),
        )
        .with_context(|| "Failed to look up drive block device.")?;
        String::from_utf8(cmd_output.stdout)?
    };
    crate::utils::shell::run_bash_command(
        format!("udisksctl unmount -b {block_device_path}").as_str(),
    )
    .with_context(|| "Unmounting disk failed.")?;
    crate::utils::shell::run_admin_bash_command(
        format!("mkfs.vfat -F 32 -n 'GIZMO{team_number}' {block_device_path}").as_str(),
    )
    .with_context(|| "Formatting disk failed.")?;
    crate::utils::shell::run_bash_command(
        format!("udisksctl mount -b {block_device_path}").as_str(),
    )
    .with_context(|| "Mounting disk failed.")?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn write_filesystem_cache(drive: &DriveInfo) -> Result<()> {
    let drive_path_str = drive
        .drive_path
        .to_str()
        .ok_or(anyhow!("Failed to convert disk path to string."))?;
    let bash_command = format!("sync {drive_path_str}");
    crate::utils::shell::run_bash_command(&bash_command)
        .with_context(|| "Writing filesystem cache failed")?;
    Ok(())
}
