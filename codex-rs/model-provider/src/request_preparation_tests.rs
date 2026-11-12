use super::prepare_provider_request;
use crate::create_model_provider;
use crate::provider_uses_first_party_auth_path;
use base64::Engine;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn explicit_provider_token_survives_credential_rotation_during_setup() {
    let home = TempDir::new().unwrap();
    let initial = CodexAuth::from_api_key("key-a");
    let manager = AuthManager::from_auth_for_testing_with_home(initial, home.path().to_path_buf());
    let mut info =
        ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into()));
    info.experimental_bearer_token = Some("provider-token".into());
    let provider = create_model_provider(info, Some(manager.clone()));

    let mut attempts = 0;
    let auth = prepare_provider_request(
        provider.as_ref(),
        provider_uses_first_party_auth_path(provider.info()),
        |_| {
            attempts += 1;
            async {
                let (_, auth) = provider.api_provider_and_auth().await?;
                codex_login::login_with_api_key(
                    home.path(),
                    "key-b",
                    codex_login::AuthCredentialsStoreMode::File,
                    codex_login::AuthKeyringBackendKind::default(),
                )?;
                manager.reload().await;
                Ok(auth)
            }
        },
    )
    .await
    .unwrap();

    assert_eq!(manager.auth_cached().unwrap().api_key(), Some("key-b"));
    assert_eq!(attempts, 1);
    let mut headers = http::HeaderMap::new();
    auth.add_auth_headers(&mut headers).unwrap();
    assert_eq!(
        headers[http::header::AUTHORIZATION],
        "Bearer provider-token"
    );
}
