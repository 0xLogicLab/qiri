//! Unit tests for the provider module.

use std::future::Future;
use std::num::NonZeroU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Waker;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::auth::AgentIdentityAuthPolicy;
use codex_login::auth::BedrockApiKeyAuth;
use codex_model_provider_info::AwsAuthRefreshConfig;
use codex_model_provider_info::AwsCredentialExportConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use codex_models_manager::ModelsManagerConfig;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::account::PlanType;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::SessionSource;
use codex_utils_redacted_string::RedactedString;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header_regex;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::RemoteCompactionSupport;
use crate::auth::AgentIdentitySessionFallback;
use crate::shared_state::process_shared_state;

fn provider_info_with_command_auth() -> ModelProviderInfo {
    ModelProviderInfo {
        auth: Some(ModelProviderAuthInfo {
            command: "print-token".to_string(),
            args: Vec::new(),
            timeout_ms: NonZeroU64::new(5_000).expect("timeout should be non-zero"),
            refresh_interval_ms: 300_000,
            cwd: std::env::current_dir()
                .expect("current dir should be available")
                .try_into()
                .expect("current dir should be absolute"),
        }),
        requires_openai_auth: false,
        ..ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into()))
    }
}

fn test_codex_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("codex-model-provider-test-{}", std::process::id()))
}

fn provider_for(base_url: String) -> ModelProviderInfo {
    ModelProviderInfo {
        name: "mock".into(),
        base_url: Some(base_url),
        model_catalog_url: None,
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        gateway_oauth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: false,
        supports_standalone_web_search: false,
        capabilities: None,
        project_id: None,
        location: None,
        credential_file: None,
        include_internal_metadata: false,
    }
}

fn remote_model(slug: &str) -> ModelInfo {
    serde_json::from_value(json!({
        "slug": slug,
        "display_name": slug,
        "description": null,
        "default_reasoning_level": "medium",
        "supported_reasoning_levels": [],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 0,
        "upgrade": null,
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": null,
        "truncation_policy": {"mode": "bytes", "limit": 10_000},
        "supports_image_detail_original": false,
        "context_window": 272_000,
        "max_context_window": 272_000,
        "experimental_supported_tools": [],
    }))
    .expect("valid model")
}

fn bedrock_api_key_auth() -> CodexAuth {
    CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
        api_key: "bedrock-api-key-test".to_string(),
        region: "us-east-1".to_string(),
    })
}

#[tokio::test]
async fn scoped_auth_ignores_scope_for_non_openai_provider() {
    let provider = create_model_provider(
        create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses),
        /*auth_manager*/ None,
    );

    let auth = provider
        .api_auth_for_scope(ProviderAuthScope {
            agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
            session_source: SessionSource::Cli,
            agent_identity_session_fallback: AgentIdentitySessionFallback::default(),
        })
        .await
        .expect("auth should resolve");

    assert!(auth.auth.to_auth_headers().unwrap().is_empty());
}

#[test]
fn openai_provider_enables_remote_compaction() {
    let provider = create_model_provider(
        ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into())),
        /*auth_manager*/ None,
    );

    assert_eq!(
        provider.capabilities(),
        ProviderCapabilities {
            remote_compaction: RemoteCompactionSupport::Unsupported,
            ..ProviderCapabilities::default()
        }
    );
}

#[test]
fn configured_provider_remote_compaction_matches_provider_support() {
    let cases = [
        (
            ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into())),
            RemoteCompactionSupport::Unsupported,
        ),
        (
            ModelProviderInfo {
                name: "Azure".to_string(),
                base_url: Some("https://example.com/openai".to_string()),
                ..ModelProviderInfo::default()
            },
            RemoteCompactionSupport::V2,
        ),
        (
            ModelProviderInfo {
                name: "Custom".to_string(),
                base_url: Some("https://example.openai.azure.com/openai/v1".to_string()),
                ..ModelProviderInfo::default()
            },
            RemoteCompactionSupport::V2,
        ),
        (
            provider_for("https://example.test/v1".to_string()),
            RemoteCompactionSupport::Unsupported,
        ),
    ];

    for (provider_info, expected) in cases {
        let provider = create_model_provider(provider_info, /*auth_manager*/ None);
        assert_eq!(provider.capabilities().remote_compaction, expected);
    }
}

#[test]
fn configured_provider_uses_default_approval_review_preferred_model() {
    let provider = create_model_provider(
        ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into())),
        /*auth_manager*/ None,
    );

    assert_eq!(
        provider.approval_review_preferred_model(),
        DEFAULT_APPROVAL_REVIEW_PREFERRED_MODEL
    );
}

#[test]
fn configured_provider_does_not_choose_an_official_model_from_api_key_auth() {
    let provider = create_model_provider(
        ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into())),
        Some(AuthManager::from_auth_for_testing(CodexAuth::from_api_key(
            "openai-api-key",
        ))),
    );

    assert_eq!(
        provider.approval_review_preferred_model(),
        DEFAULT_APPROVAL_REVIEW_PREFERRED_MODEL
    );
    assert!(provider.memory_extraction_preferred_model().is_empty());
    assert!(provider.memory_consolidation_preferred_model().is_empty());
}

#[test]
fn configured_provider_uses_default_approval_review_model_with_chatgpt_auth() {
    let provider = create_model_provider(
        ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into())),
        Some(AuthManager::from_auth_for_testing(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        )),
    );

    assert_eq!(
        provider.approval_review_preferred_model(),
        DEFAULT_APPROVAL_REVIEW_PREFERRED_MODEL
    );
}

struct RotatingAuth(AtomicUsize);

impl codex_login::ExternalAuth for RotatingAuth {
    fn resolve(&self) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async move {
            let key = if self.0.fetch_add(1, Ordering::SeqCst).is_multiple_of(2) {
                "sk-proj-fedramp-synthetic-test-key"
            } else {
                "commercial-key"
            };
            Ok(CodexAuth::from_api_key(key))
        })
    }

    fn refresh(
        &self,
        _context: codex_login::ExternalAuthRefreshContext,
    ) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

#[tokio::test]
async fn configured_provider_resolves_default_and_explicit_endpoints() {
    let info = ModelProviderInfo::create_local_provider(None);
    assert!(resolve_api_provider(&info, None).is_err());
    for key in ["local-key", "another-key"] {
        let auth = CodexAuth::from_api_key(key);
        let info =
            ModelProviderInfo::create_local_provider(Some("http://localhost:8000/v1".into()));
        assert_eq!(
            resolve_api_provider(&info, Some(&auth)).unwrap().base_url,
            "http://localhost:8000/v1"
        );
    }
}

#[test]
fn create_model_provider_builds_command_auth_manager_without_base_manager() {
    let provider = create_model_provider(
        provider_info_with_command_auth(),
        /*auth_manager*/ None,
    );

    let auth_manager = provider
        .auth_manager()
        .expect("command auth provider should have an auth manager");

    assert!(auth_manager.has_external_auth());
}

#[test]
fn create_model_provider_does_not_use_openai_auth_manager_for_amazon_bedrock_provider() {
    let provider = create_model_provider(
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: Some("codex-bedrock".to_string()),
            region: None,
            credential_export: None,
            auth_refresh: None,
        })),
        Some(AuthManager::from_auth_for_testing(CodexAuth::from_api_key(
            "openai-api-key",
        ))),
    );

    assert!(provider.auth_manager().is_none());
}

#[tokio::test]
async fn shared_bedrock_auth_refresh_is_reused_only_for_matching_configuration() {
    const TEST_NAME: &str =
        "provider::tests::shared_bedrock_auth_refresh_is_reused_only_for_matching_configuration";
    const HELPER_ARG: &str = "CODEX_BEDROCK_SHARED_AUTH_REFRESH_COMMAND";
    const SUBPROCESS_ARG: &str = "CODEX_BEDROCK_SHARED_AUTH_REFRESH_SUBPROCESS";
    let arguments = std::env::args().collect::<Vec<_>>();
    if let Some(index) = arguments.iter().position(|argument| argument == HELPER_ARG) {
        let counter = &arguments[index + 2];
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mut counter = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(counter)
            .expect("refresh invocation counter should open");
        std::io::Write::write_all(&mut counter, b"1").expect("write invocation counter");
        return;
    }

    let counter = std::env::temp_dir().join(format!("bedrock-refresh-{}", std::process::id()));
    if !arguments.iter().any(|argument| argument == SUBPROCESS_ARG) {
        std::fs::create_dir(&counter).expect("AWS command directory should be created");
        let executable = std::env::current_exe().expect("test executable should be available");
        let aws = counter.join(format!("aws{}", std::env::consts::EXE_SUFFIX));
        std::fs::hard_link(&executable, &aws)
            .or_else(|_| codex_utils_cargo_bin::copy_executable(&executable, &aws))
            .expect("test executable should be installed as aws");
        let existing_path = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(counter.clone()).chain(std::env::split_paths(&existing_path)),
        )
        .expect("test executable PATH should be valid");
        let output = tokio::process::Command::new(executable)
            .args(["--exact", TEST_NAME, "--skip", SUBPROCESS_ARG])
            .env("PATH", path)
            .env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .output()
            .await
            .expect("isolated AWS refresh test should run");
        std::fs::remove_dir_all(&counter).expect("AWS command directory should be removed");
        assert!(output.status.success(), "{output:?}");
        return;
    }
    let _ = std::fs::remove_file(&counter);
    let aws = ModelProviderAwsAuthInfo {
        profile: Some("codex-bedrock".to_string()),
        region: Some("us-west-2".to_string()),
        credential_export: None,
        auth_refresh: Some(AwsAuthRefreshConfig {
            command: "aws".to_string(),
            args: Vec::from(
                [
                    "--exact",
                    TEST_NAME,
                    "--skip",
                    HELPER_ARG,
                    "--skip",
                    counter.to_str().expect("counter path should be UTF-8"),
                ]
                .map(RedactedString::from),
            ),
            timeout_ms: NonZeroU64::new(10_000).expect("timeout should be non-zero"),
        }),
    };
    let provider_info = ModelProviderInfo::create_amazon_bedrock_provider(Some(aws.clone()));

    let first = create_model_provider(
        provider_info.clone(),
        Some(AuthManager::from_auth_for_testing(CodexAuth::from_api_key(
            "openai-api-key",
        ))),
    );
    let second = create_model_provider(provider_info.clone(), /*auth_manager*/ None);
    let shared_state = process_shared_state();
    let shared_recovery = shared_state
        .aws_auth_recovery(&aws)
        .expect("test provider should have refresh configured");
    assert_eq!(Arc::strong_count(&shared_recovery), 3);
    assert!(first.auth_manager().is_none() && second.auth_manager().is_none());

    for provider in [&first, &second] {
        for (status, body, recoverable) in [
            (http::StatusCode::UNAUTHORIZED, "ExpiredToken", true),
            (http::StatusCode::FORBIDDEN, "InvalidClientTokenId", true),
            (http::StatusCode::FORBIDDEN, "AccessDeniedException", false),
        ] {
            let error = TransportError::Http {
                retry_after: None,
                status,
                url: None,
                headers: None,
                body: Some(body.to_string()),
            };
            assert_eq!(provider.is_recoverable_auth_error(&error), recoverable);
        }
    }

    let mut invalid_aws = aws.clone();
    invalid_aws.auth_refresh.as_mut().expect("refresh").command = "not-aws".into();
    let invalid_provider = create_model_provider(
        ModelProviderInfo::create_amazon_bedrock_provider(Some(invalid_aws)),
        /*auth_manager*/ None,
    );
    let error = invalid_provider
        .recover_from_unauthorized()
        .await
        .expect_err("non-aws command should be rejected");
    assert_eq!(error.retry_delay(/*retry_count*/ 1), None);
    assert_eq!(error.to_string(), "AWS auth refresh command must be `aws`");

    let (first_result, second_result) = tokio::join!(
        first.recover_from_unauthorized(),
        second.recover_from_unauthorized()
    );
    assert_eq!(
        [first_result, second_result].map(|result| result.expect("provider should recover")),
        [ProviderUnauthorizedRecovery::Recovered; 2]
    );
    let read_counter = || std::fs::read_to_string(&counter).expect("read counter");
    assert_eq!(read_counter(), "1");

    assert_eq!(
        first
            .recover_from_unauthorized()
            .await
            .expect("later generation should recover"),
        ProviderUnauthorizedRecovery::Recovered
    );
    assert_eq!(read_counter(), "11");

    let fixture = tempfile::tempdir().expect("export fixture should be created");
    let export_counter = fixture.path().join("exports");
    let export_gate = fixture.path().join("release");
    #[cfg(unix)]
    let (command, mut args) = (
        "sh".to_string(),
        vec![
            "-c".to_string(),
            r#"printf '1\n' >> "$1"
while [ ! -e "$2" ]; do :; done
printf '%s\n' '{"AccessKeyId":"exported","SecretAccessKey":"secret"}'
"#
            .to_string(),
            "export-credentials".to_string(),
        ],
    );
    #[cfg(windows)]
    let (command, mut args) = {
        let script = fixture.path().join("export.cmd");
        std::fs::write(
            &script,
            concat!(
                "@echo off\r\n>> \"%~1\" echo 1\r\n",
                ":wait\r\nif not exist \"%~2\" goto wait\r\n",
                "echo {\"AccessKeyId\":\"exported\",\"SecretAccessKey\":\"secret\"}\r\n",
            ),
        )
        .expect("export script should be written");
        (
            "cmd.exe".to_string(),
            vec![
                "/D".to_string(),
                "/Q".to_string(),
                "/C".to_string(),
                script.to_string_lossy().into_owned(),
            ],
        )
    };
    args.extend([&export_counter, &export_gate].map(|path| path.to_string_lossy().into_owned()));
    let provider_info =
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: None,
            credential_export: Some(AwsCredentialExportConfig {
                command,
                args: args.into_iter().map(RedactedString::from).collect(),
                timeout_ms: NonZeroU64::new(5_000).expect("timeout should be non-zero"),
            }),
            ..aws.clone()
        }));
    let first_export = create_model_provider(provider_info.clone(), /*auth_manager*/ None);
    let second_export = create_model_provider(provider_info, /*auth_manager*/ None);
    let first_refresh = first_export.recover_from_unauthorized();
    let second_refresh = second_export.recover_from_unauthorized();
    tokio::pin!(first_refresh, second_refresh);
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::select! {
            result = &mut first_refresh => panic!("export should wait for its gate: {result:?}"),
            () = async {
                while !export_counter.exists() {
                    tokio::task::yield_now().await;
                }
            } => {}
        }
    })
    .await
    .expect("export should start after login");
    assert_eq!(read_counter(), "111");
    {
        // Queue another recovery after login finishes, while export still holds the lock.
        let mut context = Context::from_waker(Waker::noop());
        assert!(second_refresh.as_mut().poll(&mut context).is_pending());
    }
    std::fs::write(&export_gate, []).expect("export gate should open");
    let (first_result, second_result) = tokio::join!(first_refresh, second_refresh);
    assert_eq!(
        [first_result, second_result].map(|result| result.expect("provider should recover")),
        [ProviderUnauthorizedRecovery::Recovered; 2]
    );
    assert_eq!(read_counter(), "111");
    assert_eq!(
        std::fs::read_to_string(export_counter)
            .expect("read export counter")
            .lines()
            .collect::<Vec<_>>(),
        vec!["1"]
    );
    std::fs::remove_file(&counter).expect("refresh invocation counter should be removed");

    let different_profile = ModelProviderAwsAuthInfo {
        profile: Some("another-bedrock-profile".to_string()),
        ..aws.clone()
    };
    let other_recovery = shared_state
        .aws_auth_recovery(&different_profile)
        .expect("test provider should have refresh configured");
    assert!(!Arc::ptr_eq(&shared_recovery, &other_recovery));

    let released_recovery = Arc::downgrade(&shared_recovery);
    drop((first, second, shared_recovery));
    assert!(released_recovery.upgrade().is_none());
    assert!(shared_state.aws_auth_recovery(&aws).is_some());
}

#[tokio::test]
async fn create_model_provider_uses_managed_auth_for_amazon_bedrock_provider() {
    let auth = bedrock_api_key_auth();
    let provider = create_model_provider(
        ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
        Some(AuthManager::from_auth_for_testing(auth.clone())),
    );

    assert_eq!(provider.auth().await, Some(auth));
}

#[test]
fn custom_non_openai_provider_returns_no_account_state() {
    let provider = create_model_provider(
        ModelProviderInfo {
            name: "Custom".to_string(),
            base_url: Some("http://localhost:1234/v1".to_string()),
            wire_api: WireApi::Responses,
            requires_openai_auth: false,
            ..Default::default()
        },
        /*auth_manager*/ None,
    );

    assert_eq!(
        provider.account_state(),
        Ok(ProviderAccountState {
            account: None,
            requires_openai_auth: false,
        })
    );
}

#[test]
fn amazon_bedrock_provider_returns_bedrock_account_state() {
    let provider = create_model_provider(
        ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
        /*auth_manager*/ None,
    );

    assert_eq!(
        provider.account_state(),
        Ok(ProviderAccountState {
            account: Some(ProviderAccount::AmazonBedrock {
                uses_codex_managed_credentials: false,
            }),
            requires_openai_auth: false,
        })
    );
}

#[tokio::test]
async fn configured_bedrock_catalog_preserves_service_tiers() {
    let mut configured_model = remote_model("custom-model");
    configured_model.service_tiers = vec![codex_protocol::openai_models::ModelServiceTier {
        id: "custom-tier".to_string(),
        name: "Custom tier".to_string(),
        description: "User-defined tier description.".to_string(),
    }];
    configured_model.default_service_tier = Some("custom-tier".to_string());
    let configured_catalog = ModelsResponse {
        models: vec![configured_model],
    };
    let mut expected = configured_catalog.clone();
    expected.models[0].web_search_tool_type =
        codex_protocol::openai_models::WebSearchToolType::Text;

    let mut gov_provider = ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
    gov_provider.base_url =
        Some("https://bedrock-mantle.us-gov-west-1.api.aws/openai/v1".to_string());
    for provider_info in [
        ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
        ModelProviderInfo::create_amazon_bedrock_runtime_provider(/*aws*/ None),
        gov_provider,
    ] {
        let provider = create_model_provider(provider_info, /*auth_manager*/ None);
        for manager in [
            provider.models_manager(test_codex_home(), Some(configured_catalog.clone())),
            provider.models_manager_without_cache(Some(configured_catalog.clone())),
        ] {
            let catalog = manager
                .raw_model_catalog(
                    RefreshStrategy::Online,
                    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                )
                .await;
            assert_eq!(catalog, expected);
        }
    }
}

#[tokio::test]
async fn configured_provider_models_manager_uses_provider_bearer_token() {
    let server = MockServer::start().await;
    let remote_models = vec![remote_model("provider-model")];

    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header_regex("Authorization", "Bearer provider-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(ModelsResponse {
                    models: remote_models.clone(),
                }),
        )
        .expect(2)
        .mount(&server)
        .await;

    let mut provider_info = provider_for(server.uri());
    provider_info.experimental_bearer_token = Some("provider-token".into());
    provider_info.model_catalog_url = Some(format!("{}/models", server.uri()).into());
    provider_info.http_headers = Some(std::collections::HashMap::from([(
        codex_login::default_client::RESIDENCY_HEADER_NAME.to_string(),
        "us".into(),
    )]));
    for auth in [
        None,
        Some(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    ] {
        // Disabled discovery must ignore the catalog cached by the enabled run.
        for enabled in [true, false] {
            let provider = create_model_provider(
                provider_info.clone(),
                auth.clone().map(AuthManager::from_auth_for_testing),
            );
            let manager =
                provider.models_manager(test_codex_home(), /*config_model_catalog*/ None);
            manager.set_api_key_model_discovery_enabled(enabled);
            let refresh_strategy = if enabled {
                RefreshStrategy::Online
            } else {
                RefreshStrategy::Offline
            };
            let catalog = manager
                .raw_model_catalog(
                    refresh_strategy,
                    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                )
                .await;

            assert_eq!(
                catalog
                    .models
                    .iter()
                    .any(|model| model.slug == "provider-model"),
                enabled
            );
        }
    }
}
