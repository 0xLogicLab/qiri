//! Test-only helpers shared across the TUI crate.

use std::sync::LazyLock;

use codex_protocol::openai_models::ModelPreset;
pub(crate) use codex_utils_absolute_path::test_support::PathBufExt;
pub(crate) use codex_utils_absolute_path::test_support::test_path_buf;
use core_test_support::test_model_catalog as bundled_models_response;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) static TEST_MODEL_PRESETS: LazyLock<Vec<ModelPreset>> = LazyLock::new(|| {
    let mut response = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    response.models.sort_by_key(|model| model.priority);
    let mut presets: Vec<ModelPreset> = response.models.into_iter().map(Into::into).collect();
    ModelPreset::mark_default_by_picker_visibility(&mut presets);
    presets
});

pub(crate) fn test_path_display(path: &str) -> String {
    test_path_buf(path).display().to_string()
}

pub(crate) fn session_source_cli<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SessionSource::Cli)
}

pub(crate) fn skill_scope_user<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SkillScope::User)
}

pub(crate) fn skill_scope_repo<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SkillScope::Repo)
}

fn from_app_server_wire<T>(value: impl Serialize) -> T
where
    T: DeserializeOwned,
{
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .unwrap_or_else(|err| {
            panic!("app-server wire value should map to legacy helper type: {err}")
        })
}

pub(crate) use crate::boxed_future;

/// Caller-supplied model capabilities for tests; never part of the provider defaults.
pub(crate) fn explicit_bedrock_test_catalog(
    runtime: bool,
    gov: bool,
) -> codex_protocol::openai_models::ModelsResponse {
    let source = bundled_models_response().expect("test catalog");
    let mut models = Vec::new();
    for prefix in if runtime {
        vec!["global.openai.", "us.openai."]
    } else {
        vec!["openai."]
    } {
        for mut model in source.models.clone() {
            if model.slug.contains("daybreak") || model.slug == "codex-auto-review" {
                continue;
            }
            if gov && !matches!(model.slug.as_str(), "gpt-5.6-terra" | "gpt-5.6-luna") {
                continue;
            }
            model.service_tiers.clear();
            model.additional_speed_tiers.clear();
            model.default_service_tier = None;
            if matches!(model.slug.as_str(), "gpt-6-astra" | "gpt-6.1-sol") {
                model
                    .service_tiers
                    .push(codex_protocol::openai_models::ModelServiceTier {
                        id: "ultrafast".into(),
                        name: "Ultrafast".into(),
                        description: "Explicit test tier".into(),
                    });
            }
            model.slug = format!("{prefix}{}", model.slug);
            models.push(model);
        }
    }
    codex_protocol::openai_models::ModelsResponse { models }
}
