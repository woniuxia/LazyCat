use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use super::release_package_model::ReleaseTarget;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEntry {
    pub relative_path: String,
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifest {
    pub target: ReleaseTarget,
    pub source_path: PathBuf,
    pub entries: Vec<ArtifactEntry>,
    pub file_count: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchivedTarget {
    pub target: ReleaseTarget,
    pub archive_entry_name: String,
    pub artifact_mode: String,
}

fn relative_path_string(path: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err("产物相对路径包含不支持的路径片段".into());
        };
        let value = value
            .to_str()
            .ok_or_else(|| "产物相对路径不是有效 UTF-8".to_string())?;
        parts.push(value);
    }
    if parts.is_empty() {
        return Err("产物文件缺少相对路径".into());
    }
    Ok(parts.join("/"))
}

impl ArtifactManifest {
    pub fn from_directory(target: ReleaseTarget, source: &Path) -> Result<Self, String> {
        if !source.is_dir() {
            return Err(format!("部署产物目录不存在：{}", source.display()));
        }
        let mut entries = Vec::new();
        for entry in WalkDir::new(source).follow_links(false) {
            let entry = entry.map_err(|error| format!("遍历部署产物失败：{error}"))?;
            if entry.file_type().is_symlink() {
                return Err(format!(
                    "部署产物不能包含符号链接：{}",
                    entry.path().display()
                ));
            }
            if entry.file_type().is_dir() {
                continue;
            }
            if !entry.file_type().is_file() {
                return Err(format!(
                    "部署产物包含不支持的文件类型：{}",
                    entry.path().display()
                ));
            }
            let relative = entry
                .path()
                .strip_prefix(source)
                .map_err(|error| format!("计算部署产物相对路径失败：{error}"))?;
            let size = entry
                .metadata()
                .map_err(|error| format!("读取部署产物信息失败：{error}"))?
                .len();
            entries.push(ArtifactEntry {
                relative_path: relative_path_string(relative)?,
                size,
            });
        }
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(Self::new(target, source, entries))
    }

    pub fn from_file(target: ReleaseTarget, source: &Path) -> Result<Self, String> {
        if source
            .symlink_metadata()
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(format!("部署产物不能是符号链接：{}", source.display()));
        }
        if !source.is_file() {
            return Err(format!("部署产物文件不存在：{}", source.display()));
        }
        let name = source
            .file_name()
            .ok_or_else(|| "部署产物文件缺少文件名".to_string())?;
        let entry = ArtifactEntry {
            relative_path: relative_path_string(Path::new(name))?,
            size: source
                .metadata()
                .map_err(|error| format!("读取部署产物信息失败：{error}"))?
                .len(),
        };
        Ok(Self::new(target, source, vec![entry]))
    }

    fn new(target: ReleaseTarget, source: &Path, entries: Vec<ArtifactEntry>) -> Self {
        let file_count = entries.len() as u64;
        let total_bytes = entries.iter().map(|entry| entry.size).sum();
        Self {
            target,
            source_path: source.to_path_buf(),
            entries,
            file_count,
            total_bytes,
        }
    }

    pub fn verify_source(&self) -> Result<(), String> {
        let current = if self.source_path.is_dir() {
            Self::from_directory(self.target, &self.source_path)?
        } else {
            Self::from_file(self.target, &self.source_path)?
        };
        if current.entries != self.entries
            || current.file_count != self.file_count
            || current.total_bytes != self.total_bytes
        {
            return Err("部署产物在打包后发生变化，请重新打包".into());
        }
        Ok(())
    }
}
