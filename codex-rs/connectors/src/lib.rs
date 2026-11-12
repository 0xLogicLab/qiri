use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use std::time::Instant;

use serde::Deserialize;
use serde::Serialize;

pub mod accessible;
mod app_info;
mod app_tool_policy;
mod connector_runtime;
mod directory_cache;
pub mod filter;
pub mod merge;
pub mod metadata;
mod metadata_store;
mod plugin_config;
mod runtime_projection;
mod snapshot;

pub use app_info::AppBranding;
pub use app_info::AppInfo;
pub use app_info::AppMetadata;
pub use app_info::AppReview;
pub use app_info::AppScreenshot;
pub use app_tool_policy::AppToolPolicy;
pub use app_tool_policy::AppToolPolicyEvaluator;
pub use app_tool_policy::AppToolPolicyInput;
pub use app_tool_policy::app_is_enabled;
pub use app_tool_policy::apps_config_from_layer_stack;
pub use app_tool_policy::apps_config_origins_from_layer_stack;
pub use connector_runtime::ConnectorRuntimeContext;
pub use connector_runtime::ConnectorRuntimeContextKey;
pub use connector_runtime::ConnectorRuntimeFetchSource;
pub use connector_runtime::ConnectorRuntimeFetchTicket;
pub use connector_runtime::ConnectorRuntimeManager;
pub use connector_runtime::ConnectorRuntimePayload;
pub use connector_runtime::ConnectorRuntimeSnapshot;
pub use connector_runtime::connector_runtime_cache_path;
pub use connector_runtime::connector_runtime_context_key;
pub use directory_cache::ConnectorDirectoryCacheContext;
pub use metadata_store::ConnectorMetadata;
pub use metadata_store::ConnectorMetadataStore;
pub use metadata_store::ConnectorToolSummary;
pub use plugin_config::parse_plugin_app_config;
pub use plugin_config::parse_plugin_app_config_value;
pub use runtime_projection::ConnectorRuntimeTool;
pub use runtime_projection::InstalledConnectorRuntime;
pub use runtime_projection::connector_tool_is_synthetic;
pub use runtime_projection::installed_connector_runtime;
pub use snapshot::ConnectorSnapshot;
pub use snapshot::PluginConnectorSource;

pub const CONNECTORS_CACHE_TTL: Duration = Duration::from_secs(3600);
/// TTL for app/read metadata; it starts aligned with the connector directory cache.
pub const CONNECTOR_METADATA_CACHE_TTL: Duration = CONNECTORS_CACHE_TTL;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorDirectoryCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

impl ConnectorDirectoryCacheKey {
    pub fn new(
        chatgpt_base_url: String,
        account_id: Option<String>,
        chatgpt_user_id: Option<String>,
        is_workspace_account: bool,
    ) -> Self {
        Self {
            chatgpt_base_url,
            account_id,
            chatgpt_user_id,
            is_workspace_account,
        }
    }
}

#[derive(Clone)]
struct CachedConnectorDirectory {
    key: ConnectorDirectoryCacheKey,
    expires_at: Instant,
    connectors: Arc<[AppInfo]>,
}

static CONNECTOR_DIRECTORY_CACHE: LazyLock<StdMutex<Option<CachedConnectorDirectory>>> =
    LazyLock::new(|| StdMutex::new(None));

pub fn cached_directory_connectors(
    cache_context: &ConnectorDirectoryCacheContext,
) -> Option<Arc<[AppInfo]>> {
    if let Some(cached_connectors) = cached_directory_connectors_in_memory(&cache_context.cache_key)
    {
        return Some(cached_connectors);
    }

    let directory_cache::CachedConnectorDirectoryDiskLoad::Hit { connectors } =
        directory_cache::load_cached_directory_connectors_from_disk(cache_context)
    else {
        return None;
    };
    let connectors = Arc::from(connectors);
    write_cached_directory_connectors_in_memory(
        cache_context.cache_key.clone(),
        Arc::clone(&connectors),
        Duration::ZERO,
    );
    Some(connectors)
}

fn cached_directory_connectors_in_memory(
    cache_key: &ConnectorDirectoryCacheKey,
) -> Option<Arc<[AppInfo]>> {
    let cache_guard = CONNECTOR_DIRECTORY_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache_guard
        .as_ref()
        .filter(|cached| cached.key == *cache_key)
        .map(|cached| Arc::clone(&cached.connectors))
}

fn write_cached_directory_connectors_in_memory(
    cache_key: ConnectorDirectoryCacheKey,
    connectors: Arc<[AppInfo]>,
    ttl: Duration,
) {
    let mut cache_guard = CONNECTOR_DIRECTORY_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *cache_guard = Some(CachedConnectorDirectory {
        key: cache_key,
        expires_at: Instant::now() + ttl,
        connectors,
    });
}

fn connector_name_slug(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        "app".to_string()
    } else {
        normalized.to_string()
    }
}

fn normalize_connector_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
