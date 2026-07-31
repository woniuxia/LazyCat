use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePackageEnvironmentKind {
    Test,
    Production,
}

impl ReleasePackageEnvironmentKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            _ => Err("上线包环境无效".into()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Production => "production",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePackageType {
    LocalArchive,
    ServerUpload,
}

impl ReleasePackageType {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local_archive" => Ok(Self::LocalArchive),
            "server_upload" => Ok(Self::ServerUpload),
            _ => Err("packageType must be local_archive or server_upload".into()),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::LocalArchive => "local_archive",
            Self::ServerUpload => "server_upload",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageProjectConfig {
    pub id: i64,
    pub name: String,
    pub frontend_project_path: String,
    pub backend_project_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageEnvironmentConfig {
    pub id: i64,
    pub project_id: i64,
    #[serde(skip_serializing)]
    pub project_name: String,
    pub environment: ReleasePackageEnvironmentKind,
    pub configured: bool,
    pub output_root: String,
    pub package_type: ReleasePackageType,
    #[serde(skip_serializing)]
    pub frontend_project_path: String,
    pub frontend_expected_branch: String,
    pub frontend_build_command: String,
    pub frontend_success_keyword: String,
    pub frontend_post_upload_command: String,
    pub frontend_artifact_path: String,
    pub frontend_artifact_mode: String,
    #[serde(skip_serializing)]
    pub backend_project_path: String,
    pub backend_expected_branch: String,
    pub backend_build_command: String,
    pub backend_success_keyword: String,
    pub backend_post_upload_command: String,
    pub backend_artifact_path: String,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_username: String,
    pub ssh_auth_type: String,
    pub vault_entry_id: Option<i64>,
    pub ssh_private_key_path: String,
    pub frontend_remote_dir: String,
    pub backend_remote_path: String,
    pub health_check_enabled: bool,
    pub health_check_url: String,
    pub health_check_max_retries: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareResult {
    pub package_type: ReleasePackageType,
    pub default_folder_name: String,
    pub output_root: String,
    pub archive_path: String,
    pub frontend_artifact_mode: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseTarget {
    Frontend,
    Backend,
}
