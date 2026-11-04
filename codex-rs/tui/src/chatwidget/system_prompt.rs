//! Session-only system role selection for `/sys`.

use super::*;

const SYSTEM_PROMPT_USAGE: &str = "Usage: /sys [show|reset|<system prompt>]";

impl ChatWidget {
    pub(crate) fn open_system_prompt_popup(&mut self) {
        if !self.is_session_configured() {
            self.add_info_message(
                "System role selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let current = self
            .effective_collaboration_mode()
            .settings
            .developer_instructions;
        let default_is_current = current == self.default_developer_instructions;
        let default_prompt = self.default_developer_instructions.clone();
        let items = vec![
            SelectionItem {
                name: "QIRI AI4S default".to_string(),
                description: Some("Restore the role configured when QIRI started".to_string()),
                is_current: default_is_current,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SetSessionDeveloperInstructions {
                        instructions: default_prompt.clone(),
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Custom role".to_string(),
                description: Some("Enter a temporary system role for this session".to_string()),
                is_current: !default_is_current,
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::OpenSystemPromptEditor);
                })],
                dismiss_parent_on_child_accept: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Show active prompt".to_string(),
                description: Some("Display the complete effective role prompt".to_string()),
                actions: vec![Box::new(|tx| {
                    tx.send(AppEvent::ShowSystemPrompt);
                })],
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        let active_label = if default_is_current {
            "Current: QIRI AI4S default"
        } else {
            "Current: custom role"
        };
        let header = Box::new(
            Paragraph::new(vec![
                Line::from("Select System Role".bold()),
                Line::from(active_label.dim()),
            ])
            .wrap(Wrap { trim: false }),
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            items,
            header,
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn open_system_prompt_editor(&mut self) {
        let current = self
            .effective_collaboration_mode()
            .settings
            .developer_instructions;
        let initial_text = if current == self.default_developer_instructions {
            String::new()
        } else {
            current.unwrap_or_default()
        };
        let tx = self.app_event_tx.clone();
        let view = CustomPromptView::new(
            "Custom system role".to_string(),
            "Describe the role, domain, constraints, and working style".to_string(),
            initial_text,
            Some("Applies only to this session".to_string()),
            Box::new(move |instructions| {
                tx.send(AppEvent::SetSessionDeveloperInstructions {
                    instructions: Some(instructions),
                });
            }),
        )
        .preserve_whitespace();
        self.bottom_pane.show_text_prompt(view);
    }

    pub(crate) fn show_system_prompt(&mut self) {
        let current = self
            .effective_collaboration_mode()
            .settings
            .developer_instructions;
        let label = if current == self.default_developer_instructions {
            "QIRI AI4S default"
        } else {
            "Custom role"
        };
        let prompt = current.unwrap_or_else(|| "(no developer instructions)".to_string());
        self.add_info_message(
            format!("Active system role: {label}\n\n{prompt}"),
            Some(SYSTEM_PROMPT_USAGE.to_string()),
        );
    }

    pub(super) fn handle_system_prompt_command_args(&mut self, args: &str) {
        if !self.is_session_configured() {
            self.add_error_message(
                "Session is still starting; try /sys again in a moment.".to_string(),
            );
            return;
        }
        match args.trim() {
            value if value.eq_ignore_ascii_case("show") => self.show_system_prompt(),
            value if value.eq_ignore_ascii_case("reset") => {
                self.app_event_tx
                    .send(AppEvent::SetSessionDeveloperInstructions {
                        instructions: self.default_developer_instructions.clone(),
                    });
            }
            value if value.starts_with('-') => {
                self.add_error_message(SYSTEM_PROMPT_USAGE.to_string());
            }
            value => {
                self.app_event_tx
                    .send(AppEvent::SetSessionDeveloperInstructions {
                        instructions: Some(value.to_string()),
                    });
            }
        }
    }
}
