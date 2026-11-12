//! Catalog display names are presentation only; model selection retains wire slugs.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn custom_model_display_name_in_pickers_preserves_selection_slug() {
    let slug = "us.openai.gpt-5.6-luna";
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some(slug)).await;
    let mut preset = get_available_model(&chat, "gpt-5.5");
    preset.id = slug.to_string();
    preset.model = slug.to_string();
    preset.display_name = "GPT-5.6 Luna".to_string();
    preset.description = "Custom provider model".to_string();
    preset.default_reasoning_effort = ReasoningEffortConfig::High;
    preset.supported_reasoning_efforts = vec![
        ReasoningEffortPreset {
            effort: ReasoningEffortConfig::Low,
            description: "Quick answers".to_string(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffortConfig::High,
            description: "Deeper reasoning".to_string(),
        },
    ];
    let mut auto = preset.clone();
    auto.id = "codex-auto-fast".to_string();
    auto.model = auto.id.clone();
    auto.display_name = "Auto Fast".to_string();
    chat.model_catalog = Arc::new(ModelCatalog::new(vec![auto, preset.clone()]));
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));
    chat.open_model_popup_with_presets(chat.model_catalog.models.clone());
    assert_chatwidget_snapshot!(
        "custom_model_display_name_quick_picker",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(events.try_recv(), Ok(AppEvent::OpenAllModelsPopup));
    chat.open_all_models_popup();
    assert_chatwidget_snapshot!(
        "custom_model_display_name_all_models",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Enter.into());
    let selected =
        assert_matches!(events.try_recv(), Ok(AppEvent::OpenReasoningPopup { model }) => model);
    assert_eq!(selected, preset);
    chat.open_reasoning_popup(selected);
    assert_chatwidget_snapshot!(
        "custom_model_display_name_reasoning",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Enter.into());
    let persisted = assert_matches!(events.try_recv(), Ok(AppEvent::ApplyModelSelection { model, effort }) => (model, effort));
    assert_eq!(
        persisted,
        (slug.to_string(), Some(ReasoningEffortConfig::High))
    );
}
