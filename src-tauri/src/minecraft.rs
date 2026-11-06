// Pebble Launcher v0.1: complete Minecraft module. Replace the entire file.
use crate::{auth, models::*, net, storage};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, process::Stdio, sync::{Arc, atomic::{AtomicUsize, Ordering}}};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, sync::Mutex};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const ASSET_BASE: &str = "https://resources.download.minecraft.net";

#[derive(Default)]
pub struct MinecraftState { pub running: Mutex<HashSet<String>> }

#[derive(Serialize, Clone)]
#[serde(rename_all="camelCase")]
struct GameEvent { version: String, message: String }

fn event(app: &AppHandle, version: &str, message: &str) {
    let _ = app.emit("game-event", GameEvent { version: version.into(), message: message.into() });
}

#[tauri::command]
pub async fn list_versions() -> Result<Vec<VersionEntry>, String> {
    let client = net::client()?;
    let mut versions: VersionManifest = net::get_json(&client, MANIFEST_URL).await?;
    versions.versions.retain(|v| matches!(v.kind.as_str(), "release" | "snapshot"));
    Ok(versions.versions)
}

#[tauri::command]
pub fn list_installed(app: AppHandle) -> Result<Vec<String>, String> {
    let root = storage::root(&app)?;
    let dir = root.join("installed");
    if !dir.exists() { return Ok(Vec::new()); }
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Some(name) = entry.file_name().to_str().and_then(|s| s.strip_suffix(".ok")) {
            if storage::safe_id(name).is_ok() { ids.push(name.to_owned()); }
        }
    }
    ids.sort();
    Ok(ids)
}

#[tauri::command]
pub fn is_installed(app: AppHandle, version_id: String) -> Result<bool, String> {
    storage::safe_id(&version_id)?;
    let root = storage::root(&app)?;
    Ok(storage::version_json(&root, &version_id).exists() &&
        storage::version_jar(&root, &version_id).exists() &&
        root.join("installed").join(format!("{version_id}.ok")).exists())
}

fn allowed(library: &Library) -> bool {
    rules_allow(library.rules.as_deref())
}

fn rules_allow(rules: Option<&[Rule]>) -> bool {
    let Some(rules) = rules else { return true; };
    let os = if cfg!(target_os = "windows") { "windows" } else { "linux" };
    let arch = if cfg!(target_pointer_width = "64") { "x86_64" } else { "x86" };
    let mut enabled = false;
    for rule in rules {
        let os_match = rule.os.as_ref().is_none_or(|target| {
            let name_ok = target.name.as_deref().is_none_or(|name| name == os);
            let arch_ok = target.arch.as_deref().is_none_or(|a| a == arch || (a == "x86" && arch == "x86"));
            // Rules requiring a specific OS version are not assumed to match.
            name_ok && arch_ok && target.version.is_none()
        });
        // No optional features (demo, QuickPlay, custom resolution) are enabled.
        let features_match = rule.features.as_ref().is_none_or(|f| f.values().all(|v| !*v));
        if os_match && features_match { enabled = rule.action == "allow"; }
    }
    enabled
}

fn classifiers(lib: &Library) -> Option<&Download> {
    let os = if cfg!(target_os = "windows") { "windows" } else { "linux" };
    let classifier = lib.natives.as_ref()?.get(os)?.replace("${arch}", if cfg!(target_pointer_width = "64") { "64" } else { "32" });
    lib.downloads.as_ref()?.classifiers.as_ref()?.get(&classifier)
}

fn file_for(root: &Path, download: &Download) -> Result<PathBuf, String> {
    let relative = download.path.as_deref().ok_or("Minecraft library missing artifact path")?;
    Ok(root.join("libraries").join(storage::checked_relative(relative)?))
}

#[tauri::command]
pub async fn install_version(app: AppHandle, version_id: String) -> Result<(), String> {
    storage::safe_id(&version_id)?;
    let client = net::client()?;
    let root = storage::root(&app)?;
    let manifest: VersionManifest = net::get_json(&client, MANIFEST_URL).await?;
    let version = manifest.versions.into_iter().find(|v| v.id == version_id)
        .ok_or("This Minecraft version does not exist in Mojang's manifest")?;
    let version_path = storage::version_json(&root, &version_id);
    net::ensure_download(&client, &version.url, &version_path, version.sha1.as_deref()).await?;
    let meta: VersionMeta = serde_json::from_slice(&tokio::fs::read(&version_path).await.map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid version metadata: {e}"))?;
    if meta.id != version_id { return Err("Version metadata does not match requested version".into()); }
    if meta.arguments.is_none() { return Err("Legacy pre-1.13 argument format is not supported yet".into()); }
    let client_jar = meta.downloads.get("client").ok_or("Minecraft client JAR missing from metadata")?;
    let mut plan: Vec<(String, PathBuf, Option<String>)> = Vec::new();
    plan.push((client_jar.url.clone(), storage::version_jar(&root, &version_id), client_jar.sha1.clone()));
    for lib in &meta.libraries {
        if !allowed(lib) { continue; }
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            plan.push((artifact.url.clone(), file_for(&root, artifact)?, artifact.sha1.clone()));
        }
        if let Some(native) = classifiers(lib) {
            plan.push((native.url.clone(), file_for(&root, native)?, native.sha1.clone()));
        }
    }
    let assets_index_name = meta.asset_index.id.as_deref().unwrap_or(&meta.assets);
    storage::safe_id(assets_index_name)?;
    let index_path = root.join("assets/indexes").join(format!("{assets_index_name}.json"));
    net::ensure_download(&client, &meta.asset_index.url, &index_path, meta.asset_index.sha1.as_deref()).await?;
    let asset_index: AssetIndex = serde_json::from_slice(&tokio::fs::read(&index_path).await.map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid asset index: {e}"))?;
    for obj in asset_index.objects.values() {
        let hash = &obj.hash;
        if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Bad SHA-1 hash in Mojang asset index".into());
        }
        let prefix = &hash[..2];
        plan.push((format!("{ASSET_BASE}/{prefix}/{hash}"), root.join("assets/objects").join(prefix).join(hash), Some(hash.clone())));
    }
    if let Some(logging) = meta.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let file = &logging.file;
        let id = file.id.as_deref().ok_or("Logging file has no ID")?;
        storage::safe_id(id)?;
        plan.push((file.url.clone(), root.join("logging").join(id), file.sha1.clone()));
    }
    // Convert the manifest into a bounded, concurrent download queue.
    let mut seen = HashSet::new();
    plan.retain(|(_, path, _)| seen.insert(path.clone()));
    let total = plan.len();
    let completed = Arc::new(AtomicUsize::new(0));
    let _ = app.emit("install-progress", InstallProgress { version: version_id.clone(), done: 0, total, message: "Downloading Minecraft files".into() });
    // Keep the concurrent stream in its own scope: it must be dropped before
    // we finish the install and use the original version ID.
    {
        let progress_version_id = version_id.clone();
        let download_client = client.clone();
        let download_app = app.clone();
        let download_completed = completed.clone();
        let mut downloads = stream::iter(plan.into_iter().map(move |(url, dest, sha)| {
            let client = download_client.clone();
            let completed = download_completed.clone();
            let app = download_app.clone();
            let download_version_id = progress_version_id.clone();
            async move {
                net::ensure_download(&client, &url, &dest, sha.as_deref()).await?;
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                if done % 20 == 0 || done == total {
                    let _ = app.emit("install-progress", InstallProgress {
                        version: download_version_id,
                        done,
                        total,
                        message: "Verifying and downloading".into(),
                    });
                }
                Ok::<(), String>(())
            }
        })).buffer_unordered(12);
        while let Some(result) = downloads.next().await { result?; }
    }
    extract_natives(&root, &meta, &version_id).await?;
    let marker = root.join("installed").join(format!("{version_id}.ok"));
    if let Some(parent) = marker.parent() { tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?; }
    tokio::fs::write(marker, b"verified-v0.1").await.map_err(|e| e.to_string())?;
    let _ = app.emit("install-progress", InstallProgress { version: version_id.clone(), done: total, total, message: "Installation complete".into() });
    Ok(())
}

async fn extract_natives(root: &Path, meta: &VersionMeta, version_id: &str) -> Result<(), String> {
    let libs: Vec<(PathBuf, Vec<String>)> = meta.libraries.iter().filter(|l| allowed(l))
        .filter_map(|lib| classifiers(lib).map(|d| (lib, d)))
        .map(|(lib, d)| Ok((file_for(root, d)?, lib.extract.as_ref().and_then(|e| e.exclude.clone()).unwrap_or_default())))
        .collect::<Result<Vec<_>, String>>()?;
    let out = root.join("natives").join(version_id);
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        for (path, excludes) in libs {
            let f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
            for i in 0..zip.len() {
                let mut item = zip.by_index(i).map_err(|e| e.to_string())?;
                if item.is_dir() || excludes.iter().any(|prefix| item.name().starts_with(prefix)) { continue; }
                let Some(name) = item.enclosed_name() else { continue; };
                let Some(extension) = name.extension().and_then(|e| e.to_str()) else { continue; };
                if !matches!(extension, "dll" | "so" | "dylib" | "jnilib") { continue; }
                // Native JARs store platform library files at their own root.
                let Some(filename) = name.file_name() else { continue; };
                let destination = out.join(filename);
                let mut target = std::fs::File::create(destination).map_err(|e| e.to_string())?;
                std::io::copy(&mut item, &mut target).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }).await.map_err(|e| e.to_string())?
}

fn expand_arguments(entries: &[serde_json::Value], vars: &HashMap<&str, String>) -> Vec<String> {
    let mut result = Vec::new();
    for entry in entries {
        if let Some(s) = entry.as_str() {
            result.push(substitute(s, vars));
        } else if let Some(obj) = entry.as_object() {
            let rules = obj.get("rules").and_then(|r| serde_json::from_value::<Vec<Rule>>(r.clone()).ok());
            if !rules_allow(rules.as_deref()) { continue; }
            if let Some(value) = obj.get("value") {
                if let Some(s) = value.as_str() { result.push(substitute(s, vars)); }
                if let Some(list) = value.as_array() {
                    for item in list { if let Some(s) = item.as_str() { result.push(substitute(s, vars)); } }
                }
            }
        }
    }
    result
}
fn substitute(text: &str, vars: &HashMap<&str, String>) -> String {
    vars.iter().fold(text.to_owned(), |s, (key, value)| s.replace(&format!("${{{key}}}"), value))
}

async fn verify_java(java: &str, expected: u32) -> Result<(), String> {
    let output = Command::new(java).arg("-version").output().await.map_err(|_| format!("Java not found: {java}; select a Java {expected} runtime"))?;
    if !output.status.success() { return Err(format!("Java failed to run: {java}")); }
    let out = format!("{}{}", String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout));
    let major = out.lines().filter(|l| l.contains("version") || l.starts_with("openjdk"))
        .find_map(|line| {
            let field = line.split('"').nth(1).unwrap_or_else(|| line.split_whitespace().nth(1).unwrap_or(""));
            let nums: Vec<&str> = field.split('.').collect();
            if nums.first() == Some(&"1") { nums.get(1)?.parse::<u32>().ok() }
            else { nums.first()?.parse::<u32>().ok() }
        });
    if major != Some(expected) {
        return Err(format!("Minecraft requires Java {expected}; detected Java {:?}. Select the exact required runtime in Settings.", major));
    }
    Ok(())
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    version_id: String,
    java_path: String,
    memory_mb: u32,
    minecraft: State<'_, MinecraftState>,
    auth: State<'_, auth::AuthState>,
) -> Result<(), String> {
    storage::safe_id(&version_id)?;
    if !(1024..=32768).contains(&memory_mb) { return Err("Memory must be between 1024 and 32768 MB".into()); }
    if java_path.trim().is_empty() { return Err("Provide a Java executable path".into()); }
    let root = storage::root(&app)?;
    if !is_installed(app.clone(), version_id.clone())? {
        return Err("Install this Minecraft version before launching".into());
    }
    let meta: VersionMeta = serde_json::from_slice(&tokio::fs::read(storage::version_json(&root, &version_id)).await.map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let (account, token) = auth::access_for_launch(&auth).await?;
    let required_java = meta.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    verify_java(&java_path, required_java).await?;
    let args = meta.arguments.as_ref().ok_or("Unsupported legacy Minecraft version")?;
    let game_dir = root.join("instances").join(&version_id).join("game");
    tokio::fs::create_dir_all(&game_dir).await.map_err(|e| e.to_string())?;
    let natives_dir = root.join("natives").join(&version_id);
    let mut classpath = Vec::new();
    for lib in &meta.libraries {
        if allowed(lib) {
            if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
                classpath.push(file_for(&root, artifact)?);
            }
        }
    }
    classpath.push(storage::version_jar(&root, &version_id));
    let cp = std::env::join_paths(&classpath).map_err(|e| e.to_string())?.to_string_lossy().to_string();
    let mut vars: HashMap<&str, String> = HashMap::new();
    let pairs = [
        ("natives_directory", natives_dir.to_string_lossy().to_string()),
        ("launcher_name", "PebbleLauncher".into()),
        ("launcher_version", "0.1.0".into()),
        ("classpath", cp),
        ("classpath_separator", if cfg!(windows) { ";".into() } else { ":".into() }),
        ("library_directory", root.join("libraries").to_string_lossy().to_string()),
        ("auth_player_name", account.name.clone()),
        ("version_name", meta.id.clone()),
        ("game_directory", game_dir.to_string_lossy().to_string()),
        ("assets_root", root.join("assets").to_string_lossy().to_string()),
        ("game_assets", root.join("assets").to_string_lossy().to_string()),
        ("assets_index_name", meta.asset_index.id.clone().unwrap_or(meta.assets.clone())),
        ("auth_uuid", account.id.clone()),
        ("auth_access_token", token.clone()),
        ("user_type", "msa".into()),
        ("version_type", meta.kind.clone()),
        ("user_properties", "{}".into()),
        ("clientid", "".into()),
        ("auth_xuid", "".into()),
        ("resolution_width", "854".into()),
        ("resolution_height", "480".into()),
    ];
    for (key, value) in pairs { vars.insert(key, value); }
    let mut cmd = Command::new(&java_path);
    cmd.arg(format!("-Xmx{memory_mb}M"));
    cmd.args(expand_arguments(&args.jvm, &vars));
    if let Some(logging) = meta.logging.as_ref().and_then(|l| l.client.as_ref()) {
        if let Some(id) = logging.file.id.as_deref() {
            cmd.arg(logging.argument.replace("${path}", &root.join("logging").join(id).to_string_lossy()));
        }
    }
    cmd.arg(&meta.main_class);
    cmd.args(expand_arguments(&args.game, &vars));
    cmd.current_dir(&game_dir).stdout(Stdio::piped()).stderr(Stdio::piped());
    {
        let mut active = minecraft.running.lock().await;
        if !active.insert(version_id.clone()) { return Err("This version is already running".into()); }
    }
    match cmd.spawn() {
        Err(e) => {
            minecraft.running.lock().await.remove(&version_id);
            Err(format!("Could not start Java: {e}"))
        },
        Ok(mut child) => {
            event(&app, &version_id, "Game process started");
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let app_clone = app.clone();
            tokio::spawn(async move {
                if let Some(stdout) = stdout {
                    let app = app_clone.clone(); let version = version_id.clone(); let secret = token.clone();
                    tokio::spawn(async move {
                        let mut lines = BufReader::new(stdout).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            event(&app, &version, &line.replace(&secret, "[redacted]"));
                        }
                    });
                }
                if let Some(stderr) = stderr {
                    let app = app_clone.clone(); let version = version_id.clone(); let secret = token.clone();
                    tokio::spawn(async move {
                        let mut lines = BufReader::new(stderr).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            event(&app, &version, &line.replace(&secret, "[redacted]"));
                        }
                    });
                }
                let status = child.wait().await;
                let message = status.map(|s| format!("Game exited: {s}")).unwrap_or_else(|e| format!("Game process error: {e}"));
                event(&app_clone, &version_id, &message);
                let state = app_clone.state::<MinecraftState>();
                state.running.lock().await.remove(&version_id);
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argument_substitution() {
        let vars = HashMap::from([("auth_player_name", "Alex".to_string())]);
        assert_eq!(substitute("--username=${auth_player_name}", &vars), "--username=Alex");
    }
    #[test]
    fn rejects_unsafe_paths() {
        assert!(storage::checked_relative("../../evil").is_err());
        assert!(storage::safe_id("../evil").is_err());
    }
}
// End of complete minecraft.rs
