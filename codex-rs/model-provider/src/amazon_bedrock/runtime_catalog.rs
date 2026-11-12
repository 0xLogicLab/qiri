use codex_protocol::openai_models::ModelsResponse;
pub(crate) fn static_runtime_model_catalog() -> ModelsResponse {
    ModelsResponse { models: Vec::new() }
}
