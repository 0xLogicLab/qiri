use super::extract_prompt_request_with_offset;
use pretty_assertions::assert_eq;

#[test]
fn extract_prompt_request_returns_text_after_last_delimiter() {
    let message =
        "# Context\n## My request for Codex:\nFirst\n## My request for Codex:\n  Second\n";

    assert_eq!(
        extract_prompt_request_with_offset(message),
        ("Second", message.find("Second").expect("request offset"))
    );
}

#[test]
fn extract_prompt_request_recognizes_qiri_delimiter() {
    let message = "# Context\n## My request for QIRI:\n  Continue\n";
    assert_eq!(
        extract_prompt_request_with_offset(message),
        (
            "Continue",
            message.find("Continue").expect("request offset")
        )
    );
}

#[test]
fn extract_prompt_request_uses_last_delimiter_across_brands() {
    let message = "## My request for QIRI: first\n## My request for Codex: last";
    assert_eq!(
        extract_prompt_request_with_offset(message),
        ("last", message.find("last").unwrap())
    );
}
