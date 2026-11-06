use sha1::{Digest, Sha1};
use std::path::Path;

pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("PebbleLauncher/0.1 (personal Java launcher)")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(180))
        .build().map_err(|e| e.to_string())
}

pub async fn get_json<T: serde::de::DeserializeOwned>(client: &reqwest::Client, url: &str) -> Result<T, String> {
    validate_url(url)?;
    client.get(url).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json::<T>().await.map_err(|e| e.to_string())
}

fn validate_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    let hostname = parsed.host_str().unwrap_or_default();
    if parsed.scheme() != "https" || !matches!(hostname,
        "piston-meta.mojang.com" | "piston-data.mojang.com" |
        "libraries.minecraft.net" | "resources.download.minecraft.net" |
        "launcher.mojang.com" | "launchermeta.mojang.com" |
        "maven.minecraftforge.net" | "maven.fabricmc.net") {
        return Err(format!("Untrusted Minecraft download source: {hostname}"));
    }
    Ok(())
}

pub async fn ensure_download(client: &reqwest::Client, url: &str, dest: &Path, expected_sha1: Option<&str>) -> Result<(), String> {
    validate_url(url)?;
    if dest.exists() {
        if let Some(expected) = expected_sha1 {
            if verify_file(dest, expected).await? { return Ok(()); }
        } else {
            return Ok(());
        }
    }
    let response = client.get(url).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| format!("Download failed: {e}"))?;
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if let Some(expected) = expected_sha1 {
        let actual = format!("{:x}", Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(format!("File integrity check failed for {}", dest.display()));
        }
    }
    let parent = dest.parent().ok_or("No parent directory for download")?;
    tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    let ext = dest.extension().and_then(|extension| extension.to_str()).unwrap_or("bin");
    let temp = dest.with_extension(format!("{ext}.partial"));
    tokio::fs::write(&temp, &bytes).await.map_err(|e| e.to_string())?;
    // Windows rename cannot replace an existing destination.
    if dest.exists() { tokio::fs::remove_file(dest).await.map_err(|e| e.to_string())?; }
    tokio::fs::rename(&temp, dest).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn verify_file(path: &Path, expected: &str) -> Result<bool, String> {
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha1::digest(&bytes)).eq_ignore_ascii_case(expected))
}
