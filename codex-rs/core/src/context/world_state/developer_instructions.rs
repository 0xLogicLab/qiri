use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::DeveloperInstructions;
use serde::Deserialize;
use serde::Serialize;

const REPLACEMENT_NOTICE: &str = "These session developer instructions replace all previously provided session developer instructions.";
const REMOVAL_NOTICE: &str =
    "The previously provided session developer instructions no longer apply.";

/// Session developer instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct DeveloperInstructionsState {
    instructions: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DeveloperInstructionsSnapshot {
    instructions: Option<WorldStateHash>,
}

impl DeveloperInstructionsState {
    pub(crate) fn new(instructions: Option<&str>) -> Self {
        Self {
            instructions: instructions
                .filter(|instructions| !instructions.is_empty())
                .map(str::to_string),
        }
    }
}

impl WorldStateSection for DeveloperInstructionsState {
    const ID: &'static str = "developer_instructions";
    type Snapshot = DeveloperInstructionsSnapshot;

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let current_fragment = self
            .instructions
            .as_ref()
            .map(|instructions| DeveloperInstructions::new(instructions.clone()));
        let current = DeveloperInstructionsSnapshot {
            instructions: current_fragment.as_ref().map(WorldStateHash::from_fragment),
        };
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &current) {
            return (None, Vec::new());
        }
        let previous_had_instructions = match previous {
            PreviousSectionState::Absent => false,
            PreviousSectionState::Unknown => true,
            PreviousSectionState::Known(previous) => previous.instructions.is_some(),
        };
        let fragment = match (&self.instructions, previous_had_instructions) {
            (Some(instructions), false) => DeveloperInstructions::new(instructions.clone()),
            (Some(instructions), true) => {
                DeveloperInstructions::new(format!("{REPLACEMENT_NOTICE}\n\n{instructions}"))
            }
            (None, true) => DeveloperInstructions::new(REMOVAL_NOTICE),
            (None, false) => return (Some(current), Vec::new()),
        };
        (
            Some(current),
            vec![WorldStateUpdate::fragment(fragment).standalone()],
        )
    }
}

#[cfg(test)]
#[path = "developer_instructions_tests.rs"]
mod tests;
