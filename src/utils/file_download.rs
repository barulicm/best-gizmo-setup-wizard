use anyhow::{Result, bail};
use std::io::Write;

pub fn download_file(url: &str, dest_path: &std::path::Path) -> Result<()> {
    let response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        bail!("Failed to download file: {}", response.status());
    }
    let content = response.bytes()?;
    let dest_dir = dest_path.parent().ok_or(anyhow::Error::msg(format!(
        "Could not get parent of download destination from: {:?}",
        dest_path
    )))?;
    std::fs::create_dir_all(dest_dir)?;
    let mut dest = std::fs::File::create(dest_path)?;
    dest.write_all(&content)?;
    Ok(())
}
