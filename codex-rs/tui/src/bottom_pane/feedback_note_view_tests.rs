use super::*;
use pretty_assertions::assert_eq;
use std::time::Duration;

fn render(view: &FeedbackNoteView, width: u16) -> String {
    let height = view.desired_height(width);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf);
    render_buffer(area, &buf)
}

fn render_buffer(area: Rect, buf: &Buffer) -> String {
    let mut lines: Vec<String> = (0..area.height)
        .map(|row| {
            let mut line = String::new();
            for col in 0..area.width {
                let symbol = buf[(area.x + col, area.y + row)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(&crate::terminal_hyperlinks::strip_osc8(symbol));
                }
            }
            line.trim_end().to_string()
        })
        .collect();

    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn make_view(category: FeedbackCategory) -> FeedbackNoteView {
    let (tx_raw, _rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    FeedbackNoteView::new(
        category,
        /*turn_id*/ None,
        tx,
        /*include_logs*/ true,
        FeedbackAudience::External,
    )
}

#[test]
fn feedback_category_titles() {
    for (category, title) in [
        (FeedbackCategory::BadResult, "▌ Tell us more (bad result)"),
        (FeedbackCategory::GoodResult, "▌ Tell us more (good result)"),
        (FeedbackCategory::Bug, "▌ Tell us more (bug)"),
        (FeedbackCategory::Other, "▌ Tell us more (other)"),
    ] {
        let rendered = render(&make_view(category), /*width*/ 60);
        assert_eq!(rendered.lines().next(), Some(title));
    }
}

#[test]
fn feedback_view_bug() {
    let view = make_view(FeedbackCategory::Bug);
    let rendered = render(&view, /*width*/ 60);
    insta::assert_snapshot!("feedback_view_bug", rendered);
}

#[test]
fn feedback_view_safety_check() {
    let view = make_view(FeedbackCategory::SafetyCheck);
    let rendered = render(&view, /*width*/ 60);
    insta::assert_snapshot!("feedback_view_safety_check", rendered);
}

#[test]
fn feedback_paste_burst_preserves_multiline_note_until_submit() {
    for include_logs in [false, true] {
        for note in ["x\nrest", "id\n\nbody", "foo\nbar", "x\t\nrest"] {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let mut view = FeedbackNoteView::new(
                FeedbackCategory::Bug,
                /*turn_id*/ None,
                AppEventSender::new(tx),
                include_logs,
                FeedbackAudience::External,
            );
            let now = Instant::now();
            for (index, ch) in note.chars().enumerate() {
                let code = match ch {
                    '\n' => KeyCode::Enter,
                    '\t' => KeyCode::Tab,
                    ch => KeyCode::Char(ch),
                };
                view.handle_key_event_at(
                    KeyEvent::from(code),
                    now + Duration::from_millis(index as u64),
                );
                assert!(!view.is_complete());
                assert!(rx.try_recv().is_err());
            }
            let expected_note = note.replace('\t', "");
            assert_eq!(view.textarea.text(), expected_note);
            if !include_logs && note == "foo\nbar" {
                insta::assert_snapshot!("feedback_multiline_paste", render(&view, /*width*/ 60));
            }

            view.handle_key_event_at(
                KeyEvent::from(KeyCode::Enter),
                now + Duration::from_millis(200),
            );

            let AppEvent::SubmitFeedback {
                category,
                reason,
                turn_id,
                include_logs: submitted_logs,
            } = rx.try_recv().expect("submit feedback event")
            else {
                panic!("expected feedback submission");
            };
            assert_eq!(
                (category, reason, turn_id, submitted_logs),
                (
                    FeedbackCategory::Bug,
                    Some(expected_note),
                    None,
                    include_logs
                )
            );
            assert!(view.is_complete());
            assert!(rx.try_recv().is_err());
        }
    }
}

#[test]
fn feedback_bracketed_paste_does_not_reach_chat_composer() {
    for include_logs in [false, true] {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let tx = AppEventSender::new(tx);
        let mut pane = super::super::tests::test_pane(tx.clone());
        pane.show_view(Box::new(FeedbackNoteView::new(
            FeedbackCategory::Bug,
            /*turn_id*/ None,
            tx,
            include_logs,
            FeedbackAudience::External,
        )));
        // An explicit paste ends any preceding keystroke burst.
        pane.handle_key_event(KeyEvent::from(KeyCode::Char('x')));
        pane.handle_paste("\nrest".to_string());
        assert!(rx.try_recv().is_err());
        assert!(matches!(
            pane.handle_key_event(KeyEvent::from(KeyCode::Enter)),
            super::super::InputResult::None
        ));
        let AppEvent::SubmitFeedback {
            reason,
            include_logs: submitted_logs,
            ..
        } = rx.try_recv().expect("submit feedback event")
        else {
            panic!("expected feedback submission");
        };
        assert_eq!(
            (reason, submitted_logs),
            (Some("x\nrest".into()), include_logs)
        );
        assert!(pane.composer_is_empty());
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn submit_feedback_emits_submit_event_with_trimmed_note() {
    let (tx_raw, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut view = FeedbackNoteView::new(
        FeedbackCategory::Bug,
        Some("turn-123".to_string()),
        tx,
        /*include_logs*/ true,
        FeedbackAudience::OpenAiEmployee,
    );
    view.textarea.insert_str("  something broke  ");

    view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let event = rx.try_recv().expect("submit feedback event");
    assert!(matches!(
        event,
        AppEvent::SubmitFeedback {
            category: FeedbackCategory::Bug,
            reason: Some(reason),
            turn_id: Some(turn_id),
            include_logs: true,
        } if reason == "something broke" && turn_id == "turn-123"
    ));
    assert_eq!(view.is_complete(), true);
}

#[test]
fn submit_feedback_omits_empty_note() {
    let (tx_raw, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut view = FeedbackNoteView::new(
        FeedbackCategory::GoodResult,
        /*turn_id*/ None,
        tx,
        /*include_logs*/ false,
        FeedbackAudience::External,
    );

    view.submit();

    let event = rx.try_recv().expect("submit feedback event");
    assert!(matches!(
        event,
        AppEvent::SubmitFeedback {
            category: FeedbackCategory::GoodResult,
            reason: None,
            turn_id: None,
            include_logs: false,
        }
    ));
}

#[test]
fn feedback_note_cancel_does_not_submit() {
    let (tx_raw, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let mut view = FeedbackNoteView::new(
        FeedbackCategory::Bug,
        /*turn_id*/ None,
        AppEventSender::new(tx_raw),
        /*include_logs*/ false,
        FeedbackAudience::OpenAiEmployee,
    );
    view.textarea.insert_str("unfinished feedback");

    view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(view.is_complete(), true);
    assert!(rx.try_recv().is_err());
}
