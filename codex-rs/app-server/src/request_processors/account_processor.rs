use super::bedrock_auth::BedrockProviderConfig;
use super::bedrock_auth::clear_user_model_provider_if_bedrock;
use super::bedrock_auth::prepare_bedrock_provider;
use super::config_processor::map_error as map_config_error;
use super::*;
use crate::auth_mode::auth_mode_to_api;
use crate::model_catalog::ModelCatalog;
use crate::outgoing_message::AccountNotification;
use codex_login::login_with_bedrock_access_keys;
use codex_mcp::ema_auth_scope;
use codex_model_provider::is_supported_amazon_bedrock_region;
use codex_rmcp_client::EnterpriseOAuthCredentialGuard;

mod bedrock_gov_cloud;
mod bedrock_setup;
mod enterprise_login;
mod gateway_oauth;
mod gcp_setup;

pub(super) use enterprise_login::EnterpriseLoginCompletion;
pub(super) use enterprise_login::EnterpriseLoginTarget;
pub(crate) use gcp_setup::handle_gcp_setup;

enum CloudLoginProvider {
    AmazonBedrock,
    Gcp,
}

enum BedrockLoginCredentials {
    ApiKey(String),
    AccessKeys {
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    },
}

#[derive(Clone)]
pub(crate) struct AccountRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config: Arc<Config>,
    config_manager: ConfigManager,
    model_catalog: Arc<ModelCatalog>,
    gateway_login: Arc<std::sync::Mutex<Option<gateway_oauth::ActiveGatewayLogin>>>,
    gateway_client: Arc<std::sync::Mutex<Option<Arc<codex_login::GatewayAuthManager>>>>,
    _gateway_notifications: Arc<tokio_util::task::AbortOnDropHandle<()>>,
    pub(super) enterprise_login: Arc<enterprise_login::EnterpriseLoginState>,
}

impl AccountRequestProcessor {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        config: Arc<Config>,
        config_manager: ConfigManager,
        model_catalog: Arc<ModelCatalog>,
    ) -> Arc<Self> {
        let gateway_notifications = crate::gateway_oauth_notifications::spawn(
            Arc::clone(&auth_manager),
            config_manager.clone(),
            Arc::clone(&outgoing),
        );
        let enterprise_login = Arc::new(enterprise_login::EnterpriseLoginState::new(
            Arc::clone(&auth_manager),
            Arc::clone(&thread_manager),
            config_manager.clone(),
        ));
        let processor = Arc::new(Self {
            _gateway_notifications: Arc::new(gateway_notifications),
            auth_manager,
            thread_manager,
            outgoing,
            config,
            config_manager,
            model_catalog,
            gateway_login: Arc::new(std::sync::Mutex::new(/*t*/ None)),
            gateway_client: Arc::new(std::sync::Mutex::new(/*t*/ None)),
            enterprise_login,
        });
        processor
    }

    pub(crate) async fn login_account(
        &self,
        request_id: ConnectionRequestId,
        params: LoginAccountParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.login_v2(request_id, params).await.map(|()| None)
    }

    pub(crate) async fn logout_account(
        &self,
        request_id: ConnectionRequestId,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.logout_v2(request_id).await.map(|()| None)
    }

    pub(crate) async fn get_auth_status(
        &self,
        params: GetAuthStatusParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.get_auth_status_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn cancel_active_login(&self) {
        self.cancel_gateway_login();
        self.enterprise_login.cancel(/*login_id*/ None).await;
    }

    pub(crate) fn clear_external_auth(&self) {
        self.auth_manager.clear_external_auth();
    }

    fn current_account_updated_notification(&self) -> AccountUpdatedNotification {
        let auth = self.auth_manager.auth_cached();
        AccountUpdatedNotification {
            auth_mode: auth
                .as_ref()
                .map(CodexAuth::api_auth_mode)
                .map(auth_mode_to_api),
            plan_type: auth.as_ref().and_then(CodexAuth::account_plan_type),
        }
    }

    async fn load_latest_config(&self) -> Config {
        match self
            .config_manager
            .load_latest_config(/*fallback_cwd*/ None)
            .await
        {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!("failed to reload config, using startup config: {err}");
                self.config.as_ref().clone()
            }
        }
    }

    async fn maybe_refresh_plugin_caches_for_current_config(
        config_manager: &ConfigManager,
        thread_manager: &Arc<ThreadManager>,
        auth: Option<CodexAuth>,
    ) {
        thread_manager
            .plugins_manager()
            .clear_recommended_plugins_cache();

        match config_manager
            .load_latest_config(/*fallback_cwd*/ None)
            .await
        {
            Ok(config) => {
                Self::spawn_effective_plugins_changed_task(
                    Arc::clone(thread_manager),
                    config_manager.clone(),
                );
                let plugins_config = config.plugins_config_input();
                let refresh_thread_manager = Arc::clone(thread_manager);
                let refresh_config_manager = config_manager.clone();
                let on_effective_plugins_changed: Arc<
                    dyn Fn(codex_core_plugins::EffectivePluginsChange) + Send + Sync,
                > = Arc::new(move |_change| {
                    Self::spawn_effective_plugins_changed_task(
                        Arc::clone(&refresh_thread_manager),
                        refresh_config_manager.clone(),
                    );
                });
                thread_manager
                    .plugins_manager()
                    .maybe_start_curated_repo_sync_for_config(
                        &plugins_config,
                        Some(Arc::clone(&on_effective_plugins_changed)),
                    );
                thread_manager
                    .plugins_manager()
                    .maybe_start_remote_plugin_caches_refresh(
                        &plugins_config,
                        auth,
                        /*prefetch_recommendations*/ true,
                        Some(on_effective_plugins_changed),
                    );
            }
            Err(err) => {
                warn!(
                    "failed to reload config after account changed, skipping remote installed plugins cache refresh: {err}"
                );
            }
        }
    }

    fn spawn_effective_plugins_changed_task(
        thread_manager: Arc<ThreadManager>,
        config_manager: ConfigManager,
    ) {
        tokio::spawn(async move {
            thread_manager.plugins_manager().clear_cache();
            thread_manager.skills_service().clear_cache();
            crate::mcp_refresh::reload_mcp_config_best_effort(&thread_manager, &config_manager)
                .await;
            thread_manager.invalidate_mcp_runtimes().await;
        });
    }

    async fn login_v2(
        &self,
        request_id: ConnectionRequestId,
        params: LoginAccountParams,
    ) -> Result<(), JSONRPCErrorError> {
        if self.auth_manager.is_workload_identity_selected() {
            return Err(self.configured_auth_owned_by_host_error());
        }
        self.enterprise_login.cancel(/*login_id*/ None).await;
        match params {
            LoginAccountParams::ApiKey { api_key } => {
                self.login_api_key_v2(request_id, LoginApiKeyParams { api_key })
                    .await;
            }
            LoginAccountParams::Chatgpt { .. }
            | LoginAccountParams::ChatgptDeviceCode
            | LoginAccountParams::ChatgptAuthTokens { .. } => {
                return Err(invalid_request("OpenAI account login is not supported"));
            }
            LoginAccountParams::AmazonBedrock { api_key, region } => {
                self.login_amazon_bedrock_v2(
                    request_id,
                    BedrockLoginCredentials::ApiKey(api_key),
                    region,
                )
                .await;
            }
            LoginAccountParams::AmazonBedrockAccessKeys {
                access_key_id,
                secret_access_key,
                session_token,
                region,
            } => {
                self.login_amazon_bedrock_v2(
                    request_id,
                    BedrockLoginCredentials::AccessKeys {
                        access_key_id,
                        secret_access_key,
                        session_token,
                    },
                    region,
                )
                .await;
            }
        }
        Ok(())
    }

    fn external_auth_active_error(&self) -> JSONRPCErrorError {
        invalid_request(
            "External auth is active. Use account/login/start (chatgptAuthTokens) to update it or account/logout to clear it.",
        )
    }

    fn configured_auth_owned_by_host_error(&self) -> JSONRPCErrorError {
        invalid_request(
            "Configured external authentication is owned by the app-server host and cannot be changed through account RPCs.",
        )
    }

    fn ensure_provider_login_allowed(
        &self,
        provider: CloudLoginProvider,
    ) -> Result<(), JSONRPCErrorError> {
        if self.auth_manager.is_workload_identity_selected() {
            return Err(self.configured_auth_owned_by_host_error());
        }
        if self.auth_manager.is_external_chatgpt_auth_active() {
            return Err(self.external_auth_active_error());
        }
        if !self
            .auth_manager
            .is_login_method_allowed(ForcedLoginMethod::Api)
        {
            let operation = match provider {
                CloudLoginProvider::AmazonBedrock => "Amazon Bedrock login",
                CloudLoginProvider::Gcp => "Google Cloud setup",
            };
            return Err(invalid_request(format!(
                "{operation} is disabled. Use ChatGPT login instead."
            )));
        }
        Ok(())
    }

    async fn login_api_key_common(
        &self,
        params: &LoginApiKeyParams,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        if self.auth_manager.is_external_chatgpt_auth_active() {
            return Err(self.external_auth_active_error());
        }

        if !self
            .auth_manager
            .is_login_method_allowed(ForcedLoginMethod::Api)
        {
            return Err(invalid_request(
                "API key login is disabled. Use ChatGPT login instead.",
            ));
        }

        match login_with_api_key(
            &self.config.codex_home,
            &params.api_key,
            self.config.cli_auth_credentials_store_mode,
            self.config.auth_keyring_backend_kind(),
        ) {
            Ok(()) => {
                self.auth_manager.reload().await;
                self.config_manager.clear_chatgpt_config_loader();
                Ok(())
            }
            Err(err) => Err(internal_error(format!("failed to save api key: {err}"))),
        }
    }

    async fn login_api_key_v2(&self, request_id: ConnectionRequestId, params: LoginApiKeyParams) {
        let result = self
            .login_api_key_common(&params)
            .await
            .map(|()| LoginAccountResponse::ApiKey {});
        let logged_in = result.is_ok();
        if logged_in {
            self.model_catalog.invalidate().await;
        }
        self.outgoing.send_result(request_id, result).await;

        if logged_in {
            self.send_login_success_notifications(/*login_id*/ None)
                .await;
        }
    }

    async fn login_amazon_bedrock_v2(
        &self,
        request_id: ConnectionRequestId,
        credentials: BedrockLoginCredentials,
        region: String,
    ) {
        let result = async {
            self.ensure_provider_login_allowed(CloudLoginProvider::AmazonBedrock)?;

            match &credentials {
                BedrockLoginCredentials::ApiKey(api_key) => {
                    if api_key.trim().is_empty() {
                        return Err(invalid_request("Amazon Bedrock API key must not be empty."));
                    }
                }
                BedrockLoginCredentials::AccessKeys {
                    access_key_id,
                    secret_access_key,
                    ..
                } => {
                    if access_key_id.trim().is_empty() || secret_access_key.trim().is_empty() {
                        return Err(invalid_request(
                            "AWS access key ID and secret access key must not be empty.",
                        ));
                    }
                }
            }
            let region = region.trim();
            if !is_supported_amazon_bedrock_region(region) {
                return Err(invalid_request(format!(
                    "Amazon Bedrock does not support region `{region}`"
                )));
            }

            self.cancel_active_login().await;
            let write = prepare_bedrock_provider(
                &self.config_manager,
                BedrockProviderConfig {
                    region: matches!(&credentials, BedrockLoginCredentials::AccessKeys { .. })
                        .then_some(region),
                    profile: None,
                },
            )
            .await?;
            self.config_manager
                .commit_provider_config(write)
                .await
                .map_err(map_config_error)?;

            match credentials {
                BedrockLoginCredentials::ApiKey(api_key) => login_with_bedrock_api_key(
                    &self.config.codex_home,
                    api_key.trim(),
                    region,
                    self.config.cli_auth_credentials_store_mode,
                    self.config.auth_keyring_backend_kind(),
                ),
                BedrockLoginCredentials::AccessKeys {
                    access_key_id,
                    secret_access_key,
                    session_token,
                } => {
                    let session_token = session_token
                        .as_deref()
                        .map(str::trim)
                        .filter(|token| !token.is_empty());
                    login_with_bedrock_access_keys(
                        &self.config.codex_home,
                        access_key_id.trim(),
                        secret_access_key.trim(),
                        session_token,
                        self.config.cli_auth_credentials_store_mode,
                        self.config.auth_keyring_backend_kind(),
                    )
                }
            }
            .map_err(|err| internal_error(format!("failed to save Amazon Bedrock auth: {err}")))?;
            self.auth_manager.reload().await;
            self.config_manager.clear_chatgpt_config_loader();
            Ok(LoginAccountResponse::AmazonBedrock {})
        }
        .await;
        let logged_in = result.is_ok();
        if logged_in {
            self.model_catalog.invalidate().await;
        }
        self.outgoing.send_result(request_id, result).await;

        if logged_in {
            self.send_login_success_notifications(/*login_id*/ None)
                .await;
        }
    }

    async fn send_login_success_notifications(&self, login_id: Option<Uuid>) {
        self.thread_manager.invalidate_mcp_runtimes().await;
        self.send_account_login_notifications(AccountLoginCompletedNotification {
            login_id: login_id.map(|id| id.to_string()),
            success: true,
            error: None,
            onboarding_entrypoint: None,
        })
        .await;
    }

    async fn send_account_login_notifications(&self, payload: AccountLoginCompletedNotification) {
        let auth_changes = self.auth_manager.auth_change_state_receiver();
        let owner_generation = auth_changes.borrow().owner_generation;
        if payload.success && auth_changes.borrow().owner_generation == owner_generation {
            Self::maybe_refresh_plugin_caches_for_current_config(
                &self.config_manager,
                &self.thread_manager,
                self.auth_manager.auth_cached(),
            )
            .await;
        }

        let success = payload.success;
        self.outgoing
            .send_account_notification(
                /*connection_id*/ None,
                &auth_changes,
                owner_generation,
                AccountNotification::LoginCompleted(payload),
            )
            .await;

        if success {
            let notification = self.current_account_updated_notification();
            self.outgoing
                .send_account_notification(
                    /*connection_id*/ None,
                    &auth_changes,
                    owner_generation,
                    AccountNotification::Updated(notification),
                )
                .await;
        }
    }

    async fn logout_common(&self) -> std::result::Result<Option<AuthMode>, JSONRPCErrorError> {
        if self.auth_manager.is_workload_identity_selected() {
            return Err(self.configured_auth_owned_by_host_error());
        }
        self.cancel_active_login().await;
        let config =
            logout_configured_account(&self.config_manager, &self.auth_manager, &self.config)
                .await?;
        self.thread_manager.invalidate_mcp_runtimes().await;
        clear_account_configuration(&self.config_manager, &config).await?;

        self.model_catalog.invalidate().await;

        Self::maybe_refresh_plugin_caches_for_current_config(
            &self.config_manager,
            &self.thread_manager,
            self.auth_manager.auth_cached(),
        )
        .await;

        // Reflect the current auth method after logout (likely None).
        Ok(self
            .auth_manager
            .auth_cached()
            .as_ref()
            .map(CodexAuth::api_auth_mode)
            .map(auth_mode_to_api))
    }

    async fn logout_v2(&self, request_id: ConnectionRequestId) -> Result<(), JSONRPCErrorError> {
        let result = self.logout_common().await;
        let account_updated =
            result
                .as_ref()
                .ok()
                .cloned()
                .map(|auth_mode| AccountUpdatedNotification {
                    auth_mode,
                    plan_type: None,
                });
        self.outgoing
            .send_result(request_id, result.map(|_| LogoutAccountResponse {}))
            .await;

        if let Some(payload) = account_updated {
            self.outgoing
                .send_server_notification(ServerNotification::AccountUpdated(payload))
                .await;
        }
        Ok(())
    }

    async fn get_auth_status_response(
        &self,
        _params: GetAuthStatusParams,
    ) -> Result<GetAuthStatusResponse, JSONRPCErrorError> {
        let config = self.load_latest_config().await;
        if config.model_provider.requires_openai_auth {
            return Err(invalid_request(
                "QIRI does not support OpenAI account authentication",
            ));
        }
        Ok(GetAuthStatusResponse {
            auth_method: None,
            auth_token: None,
            requires_openai_auth: Some(false),
        })
    }
}

/// Remove account credentials using the current account's policy, even during startup recovery.
pub(crate) async fn logout_configured_account(
    config_manager: &ConfigManager,
    auth_manager: &Arc<AuthManager>,
    fallback_config: &Config,
) -> Result<Config, JSONRPCErrorError> {
    // Another process may have changed the persisted workspace. Reload both
    // account authority and its policy before selecting a grant to remove.
    auth_manager.reload().await;
    config_manager.update_managed_config_loader(Arc::clone(auth_manager), fallback_config);
    let config = config_manager
        .load_latest_config(/*fallback_cwd*/ None)
        .await;
    let scope = ema_auth_scope(auth_manager.auth_cached().as_ref());
    let enterprise_policy_failed = scope.is_some() && config.is_err();
    // Startup policy may belong to another workspace. Never use that fallback
    // to select a credential; a policy failure must not block primary logout.
    let enterprise_grant = config.as_ref().ok().and_then(|config| {
        scope
            .as_ref()
            .zip(config.mcp_enterprise_managed_auth.as_ref())
            .map(|(scope, profile)| {
                (
                    profile.idp.credential_name(scope),
                    profile.idp.issuer.clone(),
                    config.auth_keyring_backend_kind(),
                )
            })
    });
    let config = config.unwrap_or_else(|_| fallback_config.clone());

    // Retain the credential lock through primary logout. Otherwise a second
    // process can commit after deletion but before the account is removed.
    let enterprise_guard =
        if let Some((credential_name, issuer, keyring_backend)) = enterprise_grant {
            EnterpriseOAuthCredentialGuard::acquire(&credential_name, &issuer, keyring_backend)
                .await
                .map(Some)
        } else {
            Ok(None)
        };
    let cleanup_failed = match &enterprise_guard {
        Ok(Some(guard)) => guard.delete_tokens().is_err(),
        Ok(None) => false,
        Err(_) => true,
    };
    if enterprise_policy_failed || cleanup_failed {
        tracing::warn!("Failed to remove enterprise authorization; continuing account logout");
    }

    match auth_manager.logout_with_revoke().await {
        Ok(_) => {}
        Err(err) => {
            return Err(internal_error(format!("logout failed: {err}")));
        }
    }
    drop(enterprise_guard);
    Ok(config)
}

/// Discard account-scoped cloud and provider settings after credentials have been removed.
pub(crate) async fn clear_account_configuration(
    config_manager: &ConfigManager,
    config: &Config,
) -> Result<(), JSONRPCErrorError> {
    config_manager.clear_chatgpt_config_loader();
    if config.model_provider.is_amazon_bedrock() {
        clear_user_model_provider_if_bedrock(config_manager, config).await?;
    }
    Ok(())
}
