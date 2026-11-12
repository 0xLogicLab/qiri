//! Responses connection identity for explicitly configured provider endpoints.
use codex_login::WorkspaceRoutingSession;
use http::HeaderValue;
pub const ACCOUNT_ROUTING_HEADER: &str = "x-openai-account-routing-override";
use codex_login::default_client::ClientRedirectPolicy;
#[derive(Clone, Debug, Default)]
pub struct WorkspaceRoutingContext;
impl WorkspaceRoutingContext {
    pub fn new(_legacy_backend: String) -> Self {
        Self
    }
    pub fn with_session(self, _legacy_session: WorkspaceRoutingSession) -> Self {
        self
    }
}
/// API deployment and redirect policy resolved for one Responses request.
/// Workspace routes reject redirects even when their routing override supplies no header.
#[derive(Debug)]
pub struct ResolvedResponsesProvider {
    pub provider: codex_api::Provider,
    pub redirect_policy: ClientRedirectPolicy,
}

/// Effective destination and credentials that permit reusing a Responses socket.
#[derive(Debug, PartialEq, Eq)]
pub struct ResponsesConnectionKey {
    base_url: String,
    routing_header: Option<HeaderValue>,
    auth_revision: Option<u64>,
}

impl ResponsesConnectionKey {
    pub fn new(provider: &codex_api::Provider, auth_revision: Option<u64>) -> Self {
        Self {
            base_url: provider.base_url.clone(),
            routing_header: provider.headers.get(ACCOUNT_ROUTING_HEADER).cloned(),
            auth_revision,
        }
    }
}
