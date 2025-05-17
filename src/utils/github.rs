use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct GithubReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GithubRelease {
    pub name: String,
    pub tag_name: String,
    pub assets: Vec<GithubReleaseAsset>,
    pub prerelease: bool,
    pub draft: bool,
    #[serde(skip)]
    pub latest: bool,
}

impl PartialEq for GithubRelease {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl GithubRelease {
    pub fn display_name(&self) -> String {
        let suffix = match self {
            s if s.draft => " (draft)",
            s if s.prerelease => " (prerelease)",
            s if s.latest => " (latest)",
            _ => "",
        };
        format!("{}{}", self.name, suffix)
    }
}

pub fn get_releases(repo_owner: &str, repo_name: &str) -> Result<Vec<GithubRelease>> {
    let request_url = format!(
        "https://api.github.com/repos/{}/{}/releases",
        repo_owner, repo_name
    );
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(request_url)
        .header(reqwest::header::USER_AGENT, "rust-web-api-client")
        .send()?;

    if !response.status().is_success() {
        bail!("Failed to fetch releases: {}", response.status());
    }

    let mut releases: Vec<GithubRelease> = response.json()?;
    releases
        .iter_mut()
        .find(|r| !r.prerelease && !r.draft)
        .take()
        .ok_or(anyhow::Error::msg("No stable releases found"))?
        .latest = true;
    Ok(releases)
}

pub fn download_versioned_asset(
    asset: &GithubReleaseAsset,
    repo_owner: &str,
    repo_name: &str,
    release: &GithubRelease,
    cache_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    let dest_path = cache_dir
        .join(&repo_owner)
        .join(&repo_name)
        .join(&release.name)
        .join(&asset.name);
    crate::utils::file_download::download_file(&asset.browser_download_url, &dest_path)?;
    Ok(dest_path)
}
