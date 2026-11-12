//! Retained configuration context for one model session. No account routing discovery.
#[derive(Debug)]
pub struct WorkspaceRoutingSession {
    pub cwd: std::path::PathBuf,
    pub config_layer_stack: codex_config::ConfigLayerStack,
}
