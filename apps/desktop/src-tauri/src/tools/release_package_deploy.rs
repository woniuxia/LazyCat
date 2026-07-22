use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use super::release_package::ReleaseTarget;

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
#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use super::ArtifactManifest;
    use crate::tools::release_package::ReleaseTarget;
    use crate::tools::release_package_archive::extract_retry_zip;
    use zip::write::FileOptions;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lazycat-release-deploy-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_test_zip(path: &Path, entry_name: &str, content: &[u8]) {
        let file = File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(entry_name, FileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn manifest_rejects_changed_files() {
        let root = TestDir::new();
        let source = root.path().join("dist");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "v1").unwrap();
        let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();
        fs::write(source.join("index.html"), "changed").unwrap();
        assert!(manifest.verify_source().unwrap_err().contains("发生变化"));
    }

    #[test]
    fn manifests_support_empty_directories_and_backend_files() {
        let root = TestDir::new();
        let empty = root.path().join("empty");
        fs::create_dir(&empty).unwrap();
        let frontend = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &empty).unwrap();
        assert_eq!(frontend.file_count, 0);
        assert_eq!(frontend.total_bytes, 0);

        let jar = root.path().join("app.jar");
        fs::write(&jar, "jar").unwrap();
        let backend = ArtifactManifest::from_file(ReleaseTarget::Backend, &jar).unwrap();
        assert_eq!(backend.file_count, 1);
        assert_eq!(backend.total_bytes, 3);
    }

    #[test]
    fn retry_zip_extraction_is_cleaned_when_the_guard_drops() {
        let root = TestDir::new();
        let zip_path = root.path().join("good.zip");
        let destination = root.path().join("extract");
        write_test_zip(&zip_path, "dist/index.html", b"ok");

        let extraction = extract_retry_zip(&zip_path, &destination).unwrap();
        assert!(extraction.path().join("dist/index.html").is_file());
        drop(extraction);
        assert!(!destination.exists());
    }
    #[test]
    fn retry_zip_extraction_rejects_path_escape() {
        let root = TestDir::new();
        let zip_path = root.path().join("bad.zip");
        write_test_zip(&zip_path, "../escape.txt", b"bad");
        assert!(extract_retry_zip(&zip_path, &root.path().join("extract")).is_err());
        assert!(!root.path().join("escape.txt").exists());
    }
}
