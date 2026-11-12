use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::ConfigRequirementsToml;
use codex_config::LoaderOverrides;
use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::super::DebugSandboxConfigOptions;
use super::super::ManagedRequirementsMode;
use super::super::load_debug_sandbox_config_with_codex_home;
use super::bootstrap_cloud_config_bundle;

const CLOUD_MANAGED_PERMISSION_PROFILE_REQUIREMENTS: &str = r#"
default_permissions = "managed-cloud"

[allowed_permission_profiles]
managed-cloud = true

[permissions.managed-cloud]
extends = ":workspace"

[permissions.managed-cloud.network]
enabled = true
"#;

#[tokio::test]
async fn sandbox_state_skips_cloud_bootstrap() -> Result<()> {
    let options = DebugSandboxConfigOptions {
        sandbox_state: crate::SandboxStateArgs {
            sandbox_state_json: Some("{}".into()),
            ..Default::default()
        },
        permissions_profile: None,
        cwd: None,
        managed_requirements_mode: ManagedRequirementsMode::Include,
        loader_overrides: LoaderOverrides::without_managed_config_for_tests(),
    };
    bootstrap_cloud_config_bundle(
        &[],
        &options,
        || panic!("supplied state must skip CODEX_HOME resolution"),
        /*strict_config*/ false,
    )
    .await?;
    Ok(())
}
