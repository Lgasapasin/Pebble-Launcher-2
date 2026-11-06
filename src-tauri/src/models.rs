use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct VersionManifest {
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMeta {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub main_class: String,
    pub java_version: Option<JavaVersion>,
    pub downloads: HashMap<String, Download>,
    pub libraries: Vec<Library>,
    pub asset_index: Download,
    pub assets: String,
    pub arguments: Option<Arguments>,
    pub logging: Option<Logging>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Download {
    pub url: String,
    pub sha1: Option<String>,
    #[allow(dead_code)]
    pub size: Option<u64>,
    pub path: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    #[allow(dead_code)]
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Download>,
    pub classifiers: Option<HashMap<String, Download>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Extract {
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: Download,
}

#[derive(Debug, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize)]
pub struct AssetObject {
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub version: String,
    pub done: usize,
    pub total: usize,
    pub message: String,
}
