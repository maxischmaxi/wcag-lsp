use serde::Deserialize;
use std::io::Write;
use tower_lsp_server::Client;
use tower_lsp_server::ls_types::MessageType;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum UpdateError {
    Http(reqwest::Error),
    Json(serde_json::Error),
    Semver(semver::Error),
    AssetNotFound(String),
    Io(std::io::Error),
    Extract(String),
    Replace(String),
    UnsupportedPlatform,
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "HTTP error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Semver(e) => write!(f, "semver error: {e}"),
            Self::AssetNotFound(name) => write!(f, "asset not found: {name}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Extract(msg) => write!(f, "extract error: {msg}"),
            Self::Replace(msg) => write!(f, "replace error: {msg}"),
            Self::UnsupportedPlatform => write!(f, "unsupported platform"),
        }
    }
}

impl From<reqwest::Error> for UpdateError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}
impl From<serde_json::Error> for UpdateError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<semver::Error> for UpdateError {
    fn from(e: semver::Error) -> Self {
        Self::Semver(e)
    }
}
impl From<std::io::Error> for UpdateError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// GitHub API types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn current_target() -> Result<&'static str, UpdateError> {
    if cfg!(target_arch = "x86_64") && cfg!(target_os = "linux") {
        Ok("x86_64-unknown-linux-musl")
    } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "linux") {
        Ok("aarch64-unknown-linux-musl")
    } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "macos") {
        Ok("x86_64-apple-darwin")
    } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
        Ok("aarch64-apple-darwin")
    } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "windows") {
        Ok("x86_64-pc-windows-msvc")
    } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "windows") {
        Ok("aarch64-pc-windows-msvc")
    } else {
        Err(UpdateError::UnsupportedPlatform)
    }
}

pub fn asset_name_for_target(target: &str) -> String {
    if target.contains("windows") {
        format!("wcag-lsp-{target}.zip")
    } else {
        format!("wcag-lsp-{target}.tar.gz")
    }
}

pub fn is_newer(remote_tag: &str, local_version: &str) -> Result<bool, UpdateError> {
    let remote = remote_tag.strip_prefix('v').unwrap_or(remote_tag);
    let local = local_version.strip_prefix('v').unwrap_or(local_version);
    let remote_ver = semver::Version::parse(remote)?;
    let local_ver = semver::Version::parse(local)?;
    Ok(remote_ver > local_ver)
}

// ---------------------------------------------------------------------------
// Extract
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
pub fn extract_binary(archive_bytes: &[u8]) -> Result<Vec<u8>, UpdateError> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    use tar::Archive;

    let decoder = GzDecoder::new(archive_bytes);
    let mut archive = Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| UpdateError::Extract(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| UpdateError::Extract(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| UpdateError::Extract(e.to_string()))?
            .to_path_buf();

        if path.file_name().and_then(|n| n.to_str()) == Some("wcag-lsp") {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| UpdateError::Extract(e.to_string()))?;
            return Ok(buf);
        }
    }

    Err(UpdateError::Extract(
        "wcag-lsp binary not found in archive".to_string(),
    ))
}

#[cfg(target_os = "windows")]
pub fn extract_binary(archive_bytes: &[u8]) -> Result<Vec<u8>, UpdateError> {
    use std::io::{Cursor, Read};

    let reader = Cursor::new(archive_bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| UpdateError::Extract(e.to_string()))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| UpdateError::Extract(e.to_string()))?;
        if file.name().ends_with("wcag-lsp.exe") {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| UpdateError::Extract(e.to_string()))?;
            return Ok(buf);
        }
    }

    Err(UpdateError::Extract(
        "wcag-lsp.exe not found in archive".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Replace
// ---------------------------------------------------------------------------

pub fn replace_binary(binary_data: &[u8]) -> Result<(), UpdateError> {
    let dir = std::env::temp_dir();
    let tmp_path = dir.join("wcag-lsp-update-tmp");

    {
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(binary_data)?;
        f.flush()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    self_replace::self_replace(&tmp_path).map_err(|e| UpdateError::Replace(e.to_string()))?;

    // Best-effort cleanup
    let _ = std::fs::remove_file(&tmp_path);

    Ok(())
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

async fn run_update(client: &Client) -> Result<(), UpdateError> {
    let target = current_target()?;
    let expected_asset = asset_name_for_target(target);
    let local_version = env!("CARGO_PKG_VERSION");

    let http = reqwest::Client::builder()
        .user_agent("wcag-lsp-updater")
        .build()?;

    let release: GitHubRelease = http
        .get("https://api.github.com/repos/maxischmaxi/wcag-lsp/releases/latest")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if !is_newer(&release.tag_name, local_version)? {
        return Ok(());
    }

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == expected_asset)
        .ok_or_else(|| UpdateError::AssetNotFound(expected_asset.clone()))?;

    let archive_bytes = http
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let binary_data = extract_binary(&archive_bytes)?;
    replace_binary(&binary_data)?;

    client
        .show_message(
            MessageType::INFO,
            format!(
                "wcag-lsp updated to {}. Please restart your editor to use the new version.",
                release.tag_name
            ),
        )
        .await;

    Ok(())
}

pub async fn check_for_update(client: Client) {
    if let Err(e) = run_update(&client).await {
        client
            .log_message(MessageType::INFO, format!("Update check skipped: {e}"))
            .await;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_true() {
        assert!(is_newer("v0.2.0", "0.1.0").unwrap());
    }

    #[test]
    fn test_is_newer_false_equal() {
        assert!(!is_newer("v0.1.0", "0.1.0").unwrap());
    }

    #[test]
    fn test_is_newer_false_older() {
        assert!(!is_newer("v0.1.0", "0.2.0").unwrap());
    }

    #[test]
    fn test_is_newer_no_v_prefix() {
        assert!(is_newer("0.3.0", "0.1.0").unwrap());
    }

    #[test]
    fn test_is_newer_invalid() {
        assert!(is_newer("not-a-version", "0.1.0").is_err());
    }

    #[test]
    fn test_current_target_returns_ok() {
        // Should succeed on any supported CI/dev platform
        assert!(current_target().is_ok());
    }

    #[test]
    fn test_asset_name_unix() {
        let name = asset_name_for_target("x86_64-unknown-linux-musl");
        assert_eq!(name, "wcag-lsp-x86_64-unknown-linux-musl.tar.gz");
    }

    #[test]
    fn test_asset_name_windows() {
        let name = asset_name_for_target("x86_64-pc-windows-msvc");
        assert_eq!(name, "wcag-lsp-x86_64-pc-windows-msvc.zip");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_extract_binary_from_tar_gz() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::Builder;

        let binary_content = b"fake-binary-content";

        let mut archive_buf = Vec::new();
        {
            let encoder = GzEncoder::new(&mut archive_buf, Compression::default());
            let mut builder = Builder::new(encoder);

            let mut header = tar::Header::new_gnu();
            header.set_size(binary_content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();

            builder
                .append_data(&mut header, "wcag-lsp", &binary_content[..])
                .unwrap();
            builder.into_inner().unwrap().finish().unwrap();
        }

        let result = extract_binary(&archive_buf).unwrap();
        assert_eq!(result, binary_content);
    }
}
