use chrono::{Datelike, Days, NaiveDate, Weekday};
use std::path::{Path, PathBuf};

pub fn validate_folder_name(raw: &str) -> Result<(), String> {
    if raw.is_empty() || raw.trim() != raw || matches!(raw, "." | "..") {
        return Err("归档目录名不能为空，且不能包含首尾空格、`.` 或 `..`".into());
    }
    if raw
        .chars()
        .any(|ch| ch < '\u{20}' || "<>:\"/\\|?*".contains(ch))
    {
        return Err("归档目录名包含 Windows 不允许的字符".into());
    }
    if raw.ends_with('.') || raw.ends_with(' ') || Path::new(raw).components().count() != 1 {
        return Err("归档目录名必须是单级 Windows 文件夹名".into());
    }
    let stem = raw.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        return Err("归档目录名不能使用 Windows 保留设备名".into());
    }
    Ok(())
}

pub fn default_folder_name(today: NaiveDate, project_name: &str) -> String {
    let today_num = today.weekday().num_days_from_monday();
    let thursday_num = Weekday::Thu.num_days_from_monday();
    let offset = (thursday_num + 7 - today_num) % 7;
    let date = today
        .checked_add_days(Days::new(offset.into()))
        .expect("valid next Thursday");
    format!("{}-{project_name}", date.format("%Y%m%d"))
}

pub fn resolve_artifact_path(project_path: &Path, artifact_path: &str) -> PathBuf {
    let artifact = PathBuf::from(artifact_path);
    if artifact.is_absolute() {
        artifact
    } else {
        project_path.join(artifact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::path::Path;

    #[test]
    fn thursday_is_inclusive_and_other_days_advance() {
        assert_eq!(
            default_folder_name(NaiveDate::from_ymd_opt(2026, 7, 23).unwrap(), "客户门户"),
            "20260723-客户门户"
        );
        assert_eq!(
            default_folder_name(NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(), "客户门户"),
            "20260730-客户门户"
        );
    }

    #[test]
    fn folder_name_rejects_paths_and_windows_reserved_names() {
        for value in [
            "", ".", "..", "a/b", "a\\b", "CON", "LPT1.txt", "name.", "name ",
        ] {
            assert!(
                validate_folder_name(value).is_err(),
                "must reject {value:?}"
            );
        }
        assert!(validate_folder_name("20260723-客户门户").is_ok());
    }

    #[test]
    fn artifact_paths_resolve_relative_to_project() {
        assert_eq!(
            resolve_artifact_path(Path::new(r"D:\work\web"), "dist"),
            Path::new(r"D:\work\web").join("dist")
        );
        assert_eq!(
            resolve_artifact_path(Path::new(r"D:\work\web"), r"E:\shared\dist"),
            Path::new(r"E:\shared\dist")
        );
    }
}
