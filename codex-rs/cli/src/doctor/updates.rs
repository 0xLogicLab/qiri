//! Local update-feed status. No upstream desktop or release probes are installed.
use super::CheckStatus;
use super::DoctorCheck;
use codex_core::config::Config;

pub(super) async fn updates_check(config: &Config) -> DoctorCheck {
    let details = vec![
        format!(
            "check for update on startup: {}",
            config.check_for_update_on_startup
        ),
        "update action: qiri update".to_string(),
        "latest version: unavailable until QIRI publishes a release feed".to_string(),
    ];

    let status = CheckStatus::Ok;
    let summary = "QIRI update feed is not configured".to_string();

    DoctorCheck::new("updates.status", "updates", status, summary).details(details)
}
