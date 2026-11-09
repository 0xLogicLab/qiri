/// Update action the CLI should perform after the TUI exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Replace the local daemon using the installed QIRI CLI package.
    Daemon(DaemonUpdateSource),
    /// Reserved for the QIRI release channel.
    Qiri,
}

impl UpdateAction {
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Daemon(source) => ("qiri", source.command_args()),
            Self::Qiri => ("qiri", &["update"]),
        }
    }

    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

#[cfg(not(debug_assertions))]
pub fn get_update_action() -> Option<UpdateAction> {
    // No QIRI release feed exists yet; do not offer automatic updates.
    None
}

/// Package source explicitly selected by the user in the daemon menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonUpdateSource {
    ThisCli,
}

impl DaemonUpdateSource {
    pub fn command_args(self) -> &'static [&'static str] {
        match self {
            Self::ThisCli => &["app-server", "daemon", "update", "--from-cli", "--yes"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qiri_update_never_invokes_an_upstream_installer() {
        assert_eq!(UpdateAction::Qiri.command_args(), ("qiri", &["update"][..]));
        assert_eq!(UpdateAction::Qiri.command_str(), "qiri update");
    }
}
