use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Manager};

pub fn root(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn safe_id(id: &str) -> Result<&str, String> {
    if id.is_empty() || id.len() > 90 || id.starts_with('.')
        || !id.chars().all(|c| c.is_ascii_alphanumeric() || "-_ .".contains(c))
        || id.contains('/') || id.contains('\\') || id.contains("..")
    {
        return Err("Invalid Minecraft version identifier".into());
    }
    Ok(id)
}

pub fn checked_relative(path: &str) -> Result<&Path, String> {
    let p = Path::new(path);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("Rejected unsafe file path in Minecraft manifest".into());
    }
    Ok(p)
}

pub fn version_json(root: &Path, id: &str) -> PathBuf {
    root.join("versions").join(id).join(format!("{id}.json"))
}

pub fn version_jar(root: &Path, id: &str) -> PathBuf {
    root.join("versions").join(id).join(format!("{id}.jar"))
}
