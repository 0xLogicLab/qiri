#![cfg(test)]

//! Unit tests for the crate and its related test suites.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::FeedbackDiagnostic;
use pretty_assertions::assert_eq;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[test]
fn ring_buffer_drops_front_when_full() {
    let fb = CodexFeedback::with_capacity(/*max_bytes*/ 8);
    {
        let mut w = fb.make_writer().make_writer();
        w.write_all(b"abcdefgh").unwrap();
        w.write_all(b"ij").unwrap();
    }
    let snap = fb.snapshot(/*session_id*/ None);
    // Capacity 8: after writing 10 bytes, we should keep the last 8.
    pretty_assertions::assert_eq!(std::str::from_utf8(&snap.bytes).unwrap(), "cdefghij");
}

#[test]
fn logger_layer_filters_noisy_trace_payloads() {
    let fb = CodexFeedback::new();
    let _guard = tracing_subscriber::registry()
        // Keep another TRACE subscriber interested so bridged records are
        // emitted; feedback must still reject them with its own filter.
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink))
        .with(fb.logger_layer())
        .set_default();

    tracing::trace!(target: "codex_api::responses_websocket_timing", payload = "secret");
    tracing::event!(
        target: "codex_otel.log_only", tracing::Level::INFO,
        event.name = "codex.agent_response", response = "private-agent-response"
    );
    tracing::info!(target: "codex_otel.log_only", rationale = "private-guardian-rationale");
    tracing::trace!(target: "codex_http_client::transport", "transport-trace");
    tracing::trace!(target: "codex_api::sse", "sse-trace");
    tracing::trace!(target: "codex_api::sse::responses", "nested-sse-trace");
    tracing::debug!(target: "codex_http_client::transport", "transport-debug");
    tracing::debug!(target: "codex_api::sse::responses", "sse-debug");
    tracing::trace!(target: "codex_feedback_test", "unrelated-trace");
    log::trace!(target: "codex_feedback_test", "unrelated-log-trace");
    log::trace!(
        target: "tungstenite::handshake::client",
        "websocket-handshake-payload"
    );
    log::trace!(target: "tungstenite::protocol", "websocket-frame-payload");
    log::debug!(target: "tungstenite::protocol", "websocket-debug");

    let logs = String::from_utf8(fb.snapshot(/*session_id*/ None).bytes).unwrap();
    for excluded in [
        "secret",
        "private-agent-response",
        "private-guardian-rationale",
        "transport-trace",
        "sse-trace",
        "nested-sse-trace",
        "websocket-handshake-payload",
        "websocket-frame-payload",
    ] {
        assert!(!logs.contains(excluded));
    }
    for retained in [
        "transport-debug",
        "sse-debug",
        "unrelated-trace",
        "unrelated-log-trace",
        "websocket-debug",
    ] {
        assert!(logs.contains(retained));
    }
}

#[test]
fn metadata_layer_records_tags_from_feedback_target() {
    let fb = CodexFeedback::new();
    let _guard = tracing_subscriber::registry()
        .with(fb.metadata_layer())
        .set_default();

    tracing::info!(target: FEEDBACK_TAGS_TARGET, model = "gpt-5", cached = true, "tags");

    let snap = fb.snapshot(/*session_id*/ None);
    pretty_assertions::assert_eq!(snap.tags.get("model").map(String::as_str), Some("gpt-5"));
    pretty_assertions::assert_eq!(snap.tags.get("cached").map(String::as_str), Some("true"));
}

#[test]
fn feedback_attachments_gate_connectivity_diagnostics() {
    let extra_filename = format!("codex-feedback-extra-{}.jsonl", ThreadId::new());
    let extra_path = std::env::temp_dir().join(&extra_filename);
    let extra_attachment_path = FeedbackAttachmentPath {
        path: extra_path.clone(),
        attachment_filename_override: None,
    };
    fs::write(&extra_path, "rollout").expect("extra attachment should be written");

    let snapshot_with_diagnostics = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::new(vec![FeedbackDiagnostic {
            headline: "Proxy environment variables are set and may affect connectivity."
                .to_string(),
            details: vec!["HTTPS_PROXY = https://example.com:443".to_string()],
        }]));

    let attachments_with_diagnostics = snapshot_with_diagnostics
        .feedback_attachments(
            &sentry::protocol::EnvelopeHeaders::default(),
            /*include_logs*/ true,
            vec![FeedbackAttachment {
                filename: DOCTOR_REPORT_ATTACHMENT_FILENAME.to_string(),
                content_type: Some("application/json".to_string()),
                buffer: b"{\"overallStatus\":\"ok\"}".to_vec(),
            }],
            &[extra_attachment_path],
            Some(vec![1]),
        )
        .collect::<Result<Vec<_>>>()
        .unwrap();

    assert_eq!(
        attachments_with_diagnostics
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<Vec<_>>(),
        vec![
            "codex-logs.log",
            DOCTOR_REPORT_ATTACHMENT_FILENAME,
            FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME,
            extra_filename.as_str()
        ]
    );
    assert_eq!(attachments_with_diagnostics[0].buffer, vec![1]);
    assert_eq!(
        attachments_with_diagnostics[1].buffer,
        b"{\"overallStatus\":\"ok\"}".to_vec()
    );
    assert_eq!(
            attachments_with_diagnostics[2].buffer,
            b"Connectivity diagnostics\n\n- Proxy environment variables are set and may affect connectivity.\n  - HTTPS_PROXY = https://example.com:443".to_vec()
        );
    assert_eq!(attachments_with_diagnostics[3].buffer, b"rollout".to_vec());
    assert_eq!(
        attachments_with_diagnostics[3].content_type.as_deref(),
        Some("text/plain")
    );
    assert_eq!(
        OsStr::new(attachments_with_diagnostics[3].filename.as_str()),
        OsStr::new(extra_filename.as_str())
    );
    let attachments_without_diagnostics = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .with_feedback_diagnostics(FeedbackDiagnostics::default())
        .feedback_attachments(
            &sentry::protocol::EnvelopeHeaders::default(),
            /*include_logs*/ true,
            Vec::new(),
            &[],
            Some(vec![1]),
        )
        .collect::<Result<Vec<_>>>()
        .unwrap();

    assert_eq!(
        attachments_without_diagnostics
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<Vec<_>>(),
        vec!["codex-logs.log"]
    );
    assert_eq!(attachments_without_diagnostics[0].buffer, vec![1]);
    fs::remove_file(extra_path).expect("extra attachment should be removed");
}

#[test]
fn path_backed_attachments_use_binary_content_types() {
    let suffix = ThreadId::new();
    let gzip_filename = format!("codex-desktop-app-logs-{suffix}.tar.gz");
    let unknown_filename = format!("codex-feedback-extra-{suffix}.binunknown");
    let gzip_path = std::env::temp_dir().join(&gzip_filename);
    let unknown_path = std::env::temp_dir().join(&unknown_filename);
    let gzip_bytes = b"\x1f\x8b\x08\x00\xff";
    let unknown_bytes = b"\x00\x9f\x92\x96";
    fs::write(&gzip_path, gzip_bytes).expect("gzip attachment should be written");
    fs::write(&unknown_path, unknown_bytes).expect("unknown attachment should be written");

    let attachments = CodexFeedback::new()
        .snapshot(/*session_id*/ None)
        .feedback_attachments(
            &sentry::protocol::EnvelopeHeaders::default(),
            /*include_logs*/ false,
            Vec::new(),
            &[
                FeedbackAttachmentPath {
                    path: gzip_path.clone(),
                    attachment_filename_override: None,
                },
                FeedbackAttachmentPath {
                    path: unknown_path.clone(),
                    attachment_filename_override: None,
                },
            ],
            /*logs_override*/ None,
        )
        .collect::<Result<Vec<_>>>()
        .unwrap();

    fs::remove_file(gzip_path).expect("gzip attachment should be removed");
    fs::remove_file(unknown_path).expect("unknown attachment should be removed");
    assert_eq!(
        attachments
            .iter()
            .map(|attachment| (
                attachment.filename.as_str(),
                attachment.content_type.as_deref(),
                attachment.buffer.as_slice(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                gzip_filename.as_str(),
                Some("application/gzip"),
                gzip_bytes.as_slice(),
            ),
            (
                unknown_filename.as_str(),
                Some("application/octet-stream"),
                unknown_bytes.as_slice(),
            ),
        ]
    );
}

#[test]
fn upload_tags_include_client_tags_and_preserve_reserved_fields() {
    let mut tags = BTreeMap::new();
    tags.insert("thread_id".to_string(), "wrong-thread".to_string());
    tags.insert("turn_id".to_string(), "wrong-turn".to_string());
    tags.insert(
        "classification".to_string(),
        "wrong-classification".to_string(),
    );
    tags.insert("cli_version".to_string(), "wrong-version".to_string());
    tags.insert("session_source".to_string(), "wrong-source".to_string());
    tags.insert("reason".to_string(), "wrong-reason".to_string());
    tags.insert("account_id".to_string(), "actual-account".to_string());
    tags.insert("model".to_string(), "gpt-5".to_string());
    tags.insert("effort".to_string(), "Some(High)".to_string());
    let snapshot = FeedbackSnapshot {
        bytes: Vec::new(),
        tags,
        feedback_diagnostics: FeedbackDiagnostics::default(),
        thread_id: "thread-123".to_string(),
    };
    let mut client_tags = BTreeMap::new();
    client_tags.insert("thread_id".to_string(), "wrong-client-thread".to_string());
    client_tags.insert("turn_id".to_string(), "turn-456".to_string());
    client_tags.insert(
        "classification".to_string(),
        "wrong-client-classification".to_string(),
    );
    client_tags.insert(
        "cli_version".to_string(),
        "wrong-client-version".to_string(),
    );
    client_tags.insert(
        "session_source".to_string(),
        "wrong-client-source".to_string(),
    );
    client_tags.insert("reason".to_string(), "wrong-client-reason".to_string());
    client_tags.insert("client_tag".to_string(), "from-client".to_string());
    client_tags.insert("model".to_string(), "mewthree".to_string());
    client_tags.insert("effort".to_string(), "Some(Ultra)".to_string());

    let upload_tags = snapshot.upload_tags(
        "bug",
        Some("actual reason"),
        Some(&client_tags),
        Some(&SessionSource::Cli),
    );

    assert_eq!(
        upload_tags.get("thread_id").map(String::as_str),
        Some("thread-123")
    );
    assert_eq!(
        upload_tags.get("turn_id").map(String::as_str),
        Some("turn-456")
    );
    assert_eq!(
        upload_tags.get("classification").map(String::as_str),
        Some("bug")
    );
    assert_eq!(
        upload_tags.get("session_source").map(String::as_str),
        Some("cli")
    );
    assert_eq!(
        upload_tags.get("reason").map(String::as_str),
        Some("actual reason")
    );
    assert_eq!(
        upload_tags.get("account_id").map(String::as_str),
        Some("actual-account")
    );
    assert_eq!(
        upload_tags.get("model").map(String::as_str),
        Some("mewthree")
    );
    assert_eq!(
        upload_tags.get("effort").map(String::as_str),
        Some("Some(Ultra)")
    );
    assert_eq!(
        upload_tags.get("client_tag").map(String::as_str),
        Some("from-client")
    );
}

#[path = "feedback_event_tests.rs"]
mod feedback_event_tests;
#[path = "metadata_tests.rs"]
mod metadata_tests;
#[path = "save_rollout_tests.rs"]
mod save_rollout_tests;
