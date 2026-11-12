//! Unit tests for the auth module.

use codex_agent_identity::generate_agent_key_material;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::auth::AgentIdentityAuthRecord;
use codex_login::auth::BedrockApiKeyAuth;
use codex_login::auth::login_with_chatgpt_auth_tokens;
use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use codex_protocol::account::PlanType;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_protocol::error::CodexErrorDetails;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

static NEXT_CODEX_HOME_ID: AtomicUsize = AtomicUsize::new(0);
const TEST_CHATGPT_ID_TOKEN: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJlbWFpbF92ZXJpZmllZCI6dHJ1ZSwiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS9hdXRoIjp7ImNoYXRncHRfdXNlcl9pZCI6InVzZXItMTIzNDUiLCJ1c2VyX2lkIjoidXNlci0xMjM0NSIsImNoYXRncHRfcGxhbl90eXBlIjoicHJvIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudC0xMjMifX0.c2ln";

async fn agent_identity_auth(chatgpt_account_is_fedramp: bool) -> AgentIdentityAuth {
    let key_material = generate_agent_key_material().expect("generate key material");
    AgentIdentityAuth::from_record(
        AgentIdentityAuthRecord {
            agent_runtime_id: "agent-runtime-1".to_string(),
            agent_private_key: key_material.private_key_pkcs8_base64,
            account_id: "account-1".to_string(),
            chatgpt_user_id: "user-1".to_string(),
            email: Some("agent@example.com".to_string()),
            plan_type: PlanType::Plus,
            chatgpt_account_is_fedramp,
            task_id: Some("task-run-1".to_string()),
        },
        "https://auth.openai.com/api/accounts",
        &codex_login::test_support::transport_default_auth_route_config(),
    )
    .await
    .expect("agent identity auth record should include task id")
}

fn provider_auth_scope(
    policy: AgentIdentityAuthPolicy,
    fallback: AgentIdentitySessionFallback,
) -> ProviderAuthScope {
    ProviderAuthScope {
        agent_identity_policy: policy,
        session_source: SessionSource::Cli,
        agent_identity_session_fallback: fallback,
    }
}

fn test_codex_home() -> PathBuf {
    let id = NEXT_CODEX_HOME_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "codex-model-provider-agent-identity-{pid}-{id}",
        pid = std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp codex home");
    path
}

fn write_chatgpt_auth_json(codex_home: &Path) {
    let auth_json = json!({
        "tokens": {
            "id_token": TEST_CHATGPT_ID_TOKEN,
            "access_token": "test-access-token",
            "refresh_token": "test-refresh-token",
            "account_id": "account-123"
        },
        "last_refresh": "2099-01-01T00:00:00Z"
    });
    std::fs::write(
        codex_home.join("auth.json"),
        serde_json::to_string_pretty(&auth_json).expect("serialize auth.json"),
    )
    .expect("write auth.json");
}

async fn chatgpt_auth_manager(
    agent_identity_authapi_base_url: String,
) -> (PathBuf, Arc<AuthManager>, CodexAuth) {
    let codex_home = test_codex_home();
    write_chatgpt_auth_json(&codex_home);
    let auth_manager = AuthManager::shared(
        codex_home.clone(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::default(),
        codex_login::test_support::transport_default_auth_route_config(),
    )
    .await;
    let auth = auth_manager.auth().await.expect("auth should load");
    let auth_manager = AuthManager::from_auth_for_testing_with_agent_identity_authapi_base_url(
        auth.clone(),
        agent_identity_authapi_base_url,
    );
    (codex_home, auth_manager, auth)
}

async fn mount_transient_agent_registration(
    server: &MockServer,
    status: u16,
    registration_count: Arc<AtomicUsize>,
) {
    Mock::given(method("POST"))
        .and(path("/v1/agent/register"))
        .respond_with(move |_request: &wiremock::Request| {
            registration_count.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(status)
        })
        .mount(server)
        .await;
}

#[test]
fn unauthenticated_auth_provider_adds_no_headers() {
    let provider =
        create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
    let auth = resolve_provider_auth(/*auth*/ None, &provider).expect("auth should resolve");

    assert!(auth.to_auth_headers().unwrap().is_empty());
}

#[test]
fn custom_provider_does_not_inherit_ambient_auth_headers() {
    let provider =
        create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
    let mut ambient_headers = HeaderMap::new();
    ambient_headers.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer ambient-token"),
    );
    ambient_headers.insert(
        "ChatGPT-Account-ID",
        HeaderValue::from_static("account-123"),
    );
    for ambient_auth in [
        CodexAuth::Headers(AuthHeaders::new(ambient_headers)),
        CodexAuth::from_api_key("sk-proj-fedramp-synthetic-test-key"),
    ] {
        let auth =
            resolve_provider_auth(Some(&ambient_auth), &provider).expect("auth should resolve");
        assert!(auth.to_auth_headers().unwrap().is_empty());
    }
}

#[test]
fn custom_provider_does_not_inherit_ambient_bedrock_auth() {
    let provider =
        create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
    let ambient_auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
        api_key: "bedrock-api-key-test".to_string(),
        region: "us-east-1".to_string(),
    });

    let auth = resolve_provider_auth(Some(&ambient_auth), &provider).expect("auth should resolve");

    assert!(auth.to_auth_headers().unwrap().is_empty());
}

#[test]
fn custom_provider_uses_explicit_bearer_instead_of_ambient_auth() {
    let mut provider =
        create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
    provider.experimental_bearer_token = Some("provider-token".into());
    let ambient_auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
        api_key: "bedrock-api-key-test".to_string(),
        region: "us-east-1".to_string(),
    });

    let headers = resolve_provider_auth(Some(&ambient_auth), &provider)
        .expect("auth should resolve")
        .to_auth_headers()
        .unwrap();

    assert_eq!(
        headers.get(AUTHORIZATION),
        Some(&HeaderValue::from_static("Bearer provider-token"))
    );
    assert_eq!(headers.len(), 1);
}

#[test]
fn custom_provider_uses_command_resolved_auth() {
    let mut provider =
        create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
    provider.auth = Some(ModelProviderAuthInfo {
        command: "print-token".to_string(),
        args: Vec::new(),
        timeout_ms: NonZeroU64::new(5_000).expect("timeout should be non-zero"),
        refresh_interval_ms: 300_000,
        cwd: std::env::current_dir()
            .expect("current directory should be available")
            .try_into()
            .expect("current directory should be absolute"),
    });
    let command_auth = CodexAuth::from_api_key("command-token");

    let headers = resolve_provider_auth(Some(&command_auth), &provider)
        .expect("auth should resolve")
        .to_auth_headers()
        .unwrap();

    assert_eq!(
        headers.get(AUTHORIZATION),
        Some(&HeaderValue::from_static("Bearer command-token"))
    );
}

#[test]
fn header_auth_adds_predefined_headers() {
    let mut expected = HeaderMap::new();
    expected.insert(
        http::header::AUTHORIZATION,
        HeaderValue::from_static("Bearer external"),
    );
    expected.insert("x-external-auth", HeaderValue::from_static("enabled"));
    let auth = CodexAuth::Headers(AuthHeaders::new(expected.clone()));

    let actual = auth_provider_from_auth(&auth).to_auth_headers().unwrap();

    assert_eq!(actual, expected);
}

#[derive(Clone, Copy)]
enum RefreshFailure {
    Transient,
    Permanent,
    Policy,
}

struct FailingRefreshAuth {
    auth: tokio::sync::watch::Receiver<CodexAuth>,
    barrier: tokio::sync::Barrier,
    failure: RefreshFailure,
}

impl codex_login::ExternalAuth for FailingRefreshAuth {
    fn resolve(&self) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.auth.borrow().clone()) })
    }

    fn refresh(
        &self,
        _context: codex_login::ExternalAuthRefreshContext,
    ) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            self.barrier.wait().await;
            self.barrier.wait().await;
            Err(std::io::Error::other("refresh service unavailable"))
        })
    }

    fn classify_error(&self, error: std::io::Error) -> codex_login::RefreshTokenError {
        match self.failure {
            RefreshFailure::Transient => codex_login::RefreshTokenError::Transient(error),
            RefreshFailure::Permanent => codex_login::auth::RefreshTokenFailedError::new(
                codex_login::auth::RefreshTokenFailedReason::Other,
                error.to_string(),
            )
            .into(),
            RefreshFailure::Policy => codex_login::RefreshTokenError::Policy(
                codex_http_client::NetworkPolicyDenied::Revoked,
            ),
        }
    }
}

#[tokio::test]
async fn failed_refresh_retries_only_with_newer_credentials() {
    for failure in [
        RefreshFailure::Transient,
        RefreshFailure::Permanent,
        RefreshFailure::Policy,
    ] {
        for (token, account, changed) in [
            ("Bearer old", "account", false),
            ("Bearer new", "account", true),
            ("Bearer new", "other-account", false),
        ] {
            let initial = CodexAuth::Headers(AuthHeaders::new(HeaderMap::from_iter([
                (AUTHORIZATION, HeaderValue::from_static("Bearer old")),
                (
                    http::HeaderName::from_static("chatgpt-account-id"),
                    HeaderValue::from_static("account"),
                ),
            ])));
            let (updates, auth) = tokio::sync::watch::channel(initial.clone());
            let external = Arc::new(FailingRefreshAuth {
                auth,
                barrier: tokio::sync::Barrier::new(2),
                failure,
            });
            let manager = AuthManager::from_auth_for_testing(initial.clone());
            manager.set_external_auth(external.clone()).await.unwrap();
            let provider = auth_provider_from_auth_manager(manager.clone(), &initial);
            let rejected = provider.to_auth_headers().expect("initial auth headers");
            let recovery = provider.refresh_after_unauthorized(rejected);
            let concurrent_update = async {
                external.barrier.wait().await;
                updates.send_replace(CodexAuth::Headers(AuthHeaders::new(HeaderMap::from_iter(
                    [
                        (AUTHORIZATION, HeaderValue::from_static(token)),
                        (
                            http::HeaderName::from_static("chatgpt-account-id"),
                            HeaderValue::from_static(account),
                        ),
                    ],
                ))));
                manager.reload().await;
                external.barrier.wait().await;
            };
            let (retry, ()) = tokio::join!(recovery, concurrent_update);
            assert_eq!(retry, changed && !matches!(failure, RefreshFailure::Policy));
            if matches!(failure, RefreshFailure::Permanent) && token == "Bearer old" {
                // Even a late rejection must not reuse a credential known to have failed.
                assert!(!provider.refresh_after_unauthorized(HeaderMap::new()).await);
            }
        }
    }
}

#[tokio::test]
async fn agent_identity_auth_provider_preserves_account_routing_headers() {
    let auth = agent_identity_auth(/*chatgpt_account_is_fedramp*/ true).await;
    let provider = auth_provider_from_auth(&CodexAuth::AgentIdentity(auth));

    let headers = provider.to_auth_headers().unwrap();

    assert!(
        headers
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("AgentAssertion "))
    );
    assert_eq!(
        headers
            .get("ChatGPT-Account-ID")
            .and_then(|value| value.to_str().ok()),
        Some("account-1")
    );
    assert_eq!(
        headers
            .get("X-OpenAI-Fedramp")
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );
}
