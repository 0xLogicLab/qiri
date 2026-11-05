use super::*;
use crate::context::ContextualUserFragment;
use crate::context::world_state::WorldState;
use codex_protocol::models::ResponseItem;
use codex_protocol_test_support::ContentItemTestExt;
use pretty_assertions::assert_eq;

#[test]
fn role_updates_and_removal_replace_the_previous_role() {
    let mut history: Vec<ResponseItem> = Vec::new();
    let mut previous = None;

    for (instructions, expected) in [
        (None, None),
        (Some("startup role"), Some("startup role".to_string())),
        (
            Some("custom role"),
            Some(format!("{REPLACEMENT_NOTICE}\n\ncustom role")),
        ),
        (None, Some(REMOVAL_NOTICE.to_string())),
    ] {
        let mut world_state = WorldState::default();
        world_state.add_section(DeveloperInstructionsState::new(instructions));
        let (snapshot, fragments) =
            world_state.render_history_fragment_diff(previous.as_ref(), &history);
        let updates = fragments
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(
            updates,
            expected
                .map(DeveloperInstructions::new)
                .map(ContextualUserFragment::into)
                .into_iter()
                .collect::<Vec<_>>()
        );
        history.extend(updates);
        previous = Some(snapshot);
    }

    assert!(
        WorldState::default()
            .render_history_fragment_diff(previous.as_ref(), &history)
            .1
            .is_empty()
    );
    assert_eq!(
        DeveloperInstructions::new(format!("{REPLACEMENT_NOTICE}\n\ncustom role"))
            .render()
            .expect_text(),
        format!("{REPLACEMENT_NOTICE}\n\ncustom role")
    );
}
