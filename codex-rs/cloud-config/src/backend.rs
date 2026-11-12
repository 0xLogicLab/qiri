use codex_config::CloudConfigBundle;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetryableFailureKind {
    Request { status_code: Option<u16> },
    AuthRecovery { status_code: Option<u16> },
}

impl RetryableFailureKind {
    pub(crate) fn status_code(self) -> Option<u16> {
        match self {
            Self::Request { status_code } | Self::AuthRecovery { status_code } => status_code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BundleRequestError {
    Policy(codex_http_client::NetworkPolicyDenied),
    Fatal {
        error: codex_config::CloudConfigBundleLoadError,
        metric_reason: Option<&'static str>,
    },
    Retryable(RetryableFailureKind),
    Unauthorized {
        status_code: Option<u16>,
        message: String,
    },
}

#[derive(Clone)]
pub(crate) struct ConfigClient {
    http: codex_http_client::RouteAwareClientPool,
    endpoint: String,
    bearer: Option<String>,
}

impl ConfigClient {
    pub(crate) fn for_config_endpoint(
        base_url: String,
        factory: codex_http_client::HttpClientFactory,
    ) -> Self {
        Self {
            http: codex_http_client::RouteAwareClientPool::new_without_redirects_or_request_logging(
                factory,
                codex_http_client::ClientRouteClass::Api,
            ),
            endpoint: format!("{}/codex/config/bundle", base_url.trim_end_matches('/')),
            bearer: None,
        }
    }
    pub(crate) fn config_bundle_url(&self) -> String {
        self.endpoint.clone()
    }
    pub(crate) fn with_bearer_token(mut self, token: String) -> Self {
        self.bearer = Some(token);
        self
    }
}

#[derive(serde::Deserialize)]
struct DeliveredTomlFragment {
    id: String,
    name: String,
    contents: String,
}
#[derive(serde::Deserialize)]
struct DeliveredTomlBundle {
    enterprise_managed: Option<Vec<DeliveredTomlFragment>>,
}
#[derive(serde::Deserialize)]
pub(crate) struct ConfigBundleResponse {
    config_toml: Option<DeliveredTomlBundle>,
    requirements_toml: Option<DeliveredTomlBundle>,
}

pub(crate) async fn fetch_bundle(
    client: &ConfigClient,
) -> Result<CloudConfigBundle, BundleRequestError> {
    let mut request = client.http.get(&client.endpoint);
    if let Some(token) = &client.bearer {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(bundle_request_error)?;
    if response.status().as_u16() == 401 {
        return Err(BundleRequestError::Unauthorized {
            status_code: Some(401),
            message: "Managed configuration sign-in is required".into(),
        });
    }
    let response = response.error_for_status().map_err(bundle_request_error)?;
    response
        .json()
        .await
        .map(bundle_from_response)
        .map_err(bundle_request_error)
}

fn bundle_request_error(error: codex_http_client::HttpError) -> BundleRequestError {
    if let codex_http_client::HttpError::Policy(denied) = &error {
        return BundleRequestError::Policy(*denied);
    }
    BundleRequestError::Retryable(RetryableFailureKind::Request {
        status_code: error.status().map(|s| s.as_u16()),
    })
}

pub(crate) fn bundle_from_response(response: ConfigBundleResponse) -> CloudConfigBundle {
    let config_toml = response
        .config_toml
        .and_then(|config_toml| config_toml.enterprise_managed)
        .unwrap_or_default()
        .into_iter()
        .map(config_fragment_from_delivered)
        .collect();
    let requirements_toml = response
        .requirements_toml
        .and_then(|requirements_toml| requirements_toml.enterprise_managed)
        .unwrap_or_default()
        .into_iter()
        .map(requirements_fragment_from_delivered)
        .collect();

    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: config_toml,
        },
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: requirements_toml,
        },
    }
}

fn config_fragment_from_delivered(fragment: DeliveredTomlFragment) -> CloudConfigFragment {
    CloudConfigFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}

fn requirements_fragment_from_delivered(
    fragment: DeliveredTomlFragment,
) -> CloudRequirementsFragment {
    CloudRequirementsFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}
