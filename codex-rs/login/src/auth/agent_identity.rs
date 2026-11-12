//! Legacy identity data and local key validation. No service registration or JWKS transport.
use super::storage::AgentIdentityAuthRecord;
use crate::outbound_proxy::AuthRouteConfig;
use codex_agent_identity::public_key_ssh_from_private_key_pkcs8_base64;
use codex_protocol::account::PlanType as AccountPlanType;
use std::sync::Arc;
use thiserror::Error;
#[derive(Clone, Debug, Error)]
pub enum AgentIdentityAuthError {
    #[error(
        "agent identity bootstrap unavailable after {attempts} attempts during {operation}: {message}"
    )]
    BootstrapUnavailable {
        operation: &'static str,
        attempts: usize,
        message: String,
    },
}

impl AgentIdentityAuthError {
    pub(super) fn bootstrap_unavailable(error: &std::io::Error) -> Option<&Self> {
        match error
            .get_ref()
            .and_then(|source| source.downcast_ref::<Self>())
        {
            Some(error @ Self::BootstrapUnavailable { .. }) => Some(error),
            None => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentIdentityAuth {
    record: Arc<AgentIdentityAuthRecord>,
}

impl AgentIdentityAuth {
    pub async fn from_record(
        record: AgentIdentityAuthRecord,
        _agent_identity_authapi_base_url: &str,
        _auth_route_config: &AuthRouteConfig,
    ) -> std::io::Result<Self> {
        public_key_ssh_from_private_key_pkcs8_base64(&record.agent_private_key)
            .map_err(std::io::Error::other)?;
        if record.task_id.as_deref().is_none_or(str::is_empty) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Service identity registration has been removed",
            ));
        }
        Ok(Self {
            record: Arc::new(record),
        })
    }

    #[cfg(test)]
    fn from_initialized_record(mut record: AgentIdentityAuthRecord, run_task_id: String) -> Self {
        record.task_id = Some(run_task_id);
        Self {
            record: Arc::new(record),
        }
    }

    pub fn record(&self) -> &AgentIdentityAuthRecord {
        self.record.as_ref()
    }

    pub fn run_task_id(&self) -> &str {
        match self.record.task_id.as_deref() {
            Some(task_id) => task_id,
            None => unreachable!("AgentIdentityAuth should only be constructed with a task_id"),
        }
    }

    pub fn account_id(&self) -> &str {
        &self.record.account_id
    }

    pub fn chatgpt_user_id(&self) -> &str {
        &self.record.chatgpt_user_id
    }

    pub fn email(&self) -> Option<&str> {
        self.record.email.as_deref()
    }

    pub fn plan_type(&self) -> AccountPlanType {
        self.record.plan_type
    }

    pub fn is_fedramp_account(&self) -> bool {
        self.record.chatgpt_account_is_fedramp
    }
}
