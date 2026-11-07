#![cfg(test)]

use super::*;
use codex_tui::DaemonUpdateSource;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;

#[test]
fn upstream_package_update_is_rejected() -> anyhow::Result<()> {
    const CHILD: &str = "CODEX_TEST_PACKAGE_UPDATE_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let error = run_update_action(UpdateAction::Qiri, /*cli_executable*/ None)
            .expect_err("upstream update must be disabled");
        assert!(
            error
                .to_string()
                .contains("QIRI updates are not published yet")
        );
        return Ok(());
    }

    let project = tempfile::tempdir()?;
    let bin = tempfile::tempdir()?;
    codex_utils_cargo_bin::write_executable(
        &bin.path().join("pnpm"),
        "#!/bin/sh\npwd -P > \"$0.cwd\"\n",
    )?;
    assert_cmd::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "daemon_update_tests::upstream_package_update_is_rejected",
        ])
        .current_dir(project.path())
        .env("TMPDIR", project.path())
        .env("PATH", bin.path())
        .env(CHILD, "1")
        .assert()
        .success();

    assert!(!bin.path().join("pnpm.cwd").exists());
    Ok(())
}

#[test]
fn daemon_handoff_uses_selected_executable_and_propagates_failure() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let executable = dir.path().join("launching CLI");
    let receipt = dir.path().join("launching CLI.args");
    codex_utils_cargo_bin::write_executable(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n",
    )?;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(/*mode*/ 0o700))?;
    run_update_action(
        UpdateAction::Daemon(DaemonUpdateSource::ThisCli),
        Some(&executable),
    )?;
    assert_eq!(
        std::fs::read_to_string(&receipt)?,
        "app-server\ndaemon\nupdate\n--from-cli\n--yes\n"
    );
    codex_utils_cargo_bin::write_executable(&executable, "#!/bin/sh\nexit 7\n")?;
    let error = run_update_action(
        UpdateAction::Daemon(DaemonUpdateSource::ThisCli),
        Some(&executable),
    )
    .unwrap_err();
    assert!(error.to_string().contains("Daemon update failed"));
    Ok(())
}
