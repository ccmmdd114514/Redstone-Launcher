//! 官方元数据结构：version_manifest_v2 / version.json / asset index。
//! 字段全部按官方 JSON 命名（camelCase）反序列化，不做任何硬编码假设。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Deserialize, Serialize)]
pub struct VersionManifest {
    pub versions: Vec<ManifestVersion>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub main_class: String,
    pub libraries: Vec<Library>,
    pub downloads: HashMap<String, Artifact>,
    pub asset_index: AssetIndexRef,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
    pub java_version: Option<JavaVersion>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub path: Option<String>,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Arguments {
    pub game: Vec<ArgElement>,
    pub jvm: Vec<ArgElement>,
}

/// arguments 里的元素有两种形态：纯字符串，或带 rules 的条件项。
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ArgElement {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    /// 旧版本用 natives 映射（如 {"windows": "natives-windows"}）
    pub natives: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

impl Library {
    /// 库名里的 classifier（如 natives-windows / natives-windows-arm64）。
    pub fn classifier(&self) -> Option<&str> {
        self.name.split(':').nth(3)
    }

    /// 是否为平台原生库（classifier 以 natives- 开头）。旧版本另看 natives 映射。
    pub fn is_native(&self) -> bool {
        if self
            .classifier()
            .map(|c| c.starts_with("natives-"))
            .unwrap_or(false)
        {
            return true;
        }
        self.natives.is_some()
    }

    /// 当前平台（Windows x86_64）真正要用的原生库。
    /// 其他架构（arm64 / x86）的原生库整个跳过：既不进 classpath，也不解压。
    pub fn native_artifact(&self) -> Option<&Artifact> {
        match self.classifier() {
            Some(c) if c.starts_with("natives-") => {
                if c == "natives-windows" {
                    self.downloads.as_ref()?.artifact.as_ref()
                } else {
                    None
                }
            }
            _ => {
                let classifier = self.natives.as_ref()?.get("windows")?;
                self.downloads
                    .as_ref()?
                    .classifiers
                    .as_ref()?
                    .get(classifier)
            }
        }
    }

    pub fn artifact(&self) -> Option<&Artifact> {
        self.downloads.as_ref()?.artifact.as_ref()
    }

    /// 本地相对路径：优先用官方给的 path，否则按 name 推导。
    pub fn relative_path(&self, artifact: &Artifact) -> String {
        if let Some(p) = &artifact.path {
            return p.replace('/', "\\");
        }
        let parts: Vec<&str> = self.name.split(':').collect();
        let group = parts.first().unwrap_or(&"");
        let name = parts.get(1).unwrap_or(&"");
        let ver = parts.get(2).unwrap_or(&"");
        let classifier = parts.get(3).copied().unwrap_or("");
        let file = if classifier.is_empty() {
            format!("{name}-{ver}.jar")
        } else {
            format!("{name}-{ver}-{classifier}.jar")
        };
        format!("{}\\{name}\\{ver}\\{file}", group.replace('.', "\\"))
    }
}
