use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::WebSearchToolType;
pub(crate) fn static_model_catalog() -> ModelsResponse {
    ModelsResponse { models: Vec::new() }
}
pub(super) fn static_gov_model_catalog() -> ModelsResponse {
    static_model_catalog()
}
pub(super) fn normalize_bedrock_catalog(mut catalog: ModelsResponse) -> ModelsResponse {
    // Custom catalogs own their tier definitions; only normalize wire compatibility here.
    for model in &mut catalog.models {
        // Bedrock rejects the `search_content_types` field used by multimodal search.
        model.web_search_tool_type = WebSearchToolType::Text;
    }
    catalog
}
