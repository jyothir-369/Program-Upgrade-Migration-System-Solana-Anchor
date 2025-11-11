use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

// Minimal Pubkey substitute for backend (base58 string)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pubkey(pub String);

#[derive(Debug, Error)]
pub enum UpgradeError {
    #[error("multisig error: {0}")]
    Multisig(String),
    #[error("program client error: {0}")]
    Program(String),
    #[error("notification error: {0}")]
    Notification(String),
    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    ProgramUpgrade,
    ProgramUpgradeExecuted,
    ProgramUpgradeCancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub notification_type: NotificationType,
    pub proposal_id: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ProposalParams {
    pub instruction: Vec<u8>,
    pub description: String,
    pub timelock_secs: i64,
}

#[async_trait]
pub trait MultisigManager: Send + Sync {
    async fn propose_transaction(&self, params: ProposalParams) -> Result<String, UpgradeError>;
    async fn approve(&self, proposal_id: &str) -> Result<(), UpgradeError>;
    async fn cancel(&self, proposal_id: &str) -> Result<(), UpgradeError>;
}

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn notify_community(&self, n: Notification) -> Result<(), UpgradeError>;
}

#[async_trait]
pub trait ProgramClient: Send + Sync {
    async fn build_upgrade_ix(&self, new_program_buffer: &Pubkey) -> Result<Vec<u8>, UpgradeError>;
    async fn record_upgrade_metadata(&self, proposal_id: &str, new_buffer: &Pubkey, hash: &[u8;32]) -> Result<(), UpgradeError>;
}

#[derive(Clone)]
pub struct ProgramUpgrade {
    multisig: Arc<dyn MultisigManager>,
    notification_service: Arc<dyn NotificationService>,
    program_client: Arc<dyn ProgramClient>,
    state: Arc<Mutex<UpgradeServiceState>>, // in-memory cache; authoritative trail is DB
}

#[derive(Default)]
struct UpgradeServiceState {
    pub open_proposals: usize,
}

impl ProgramUpgrade {
    pub fn new(
        multisig: Arc<dyn MultisigManager>,
        notification_service: Arc<dyn NotificationService>,
        program_client: Arc<dyn ProgramClient>,
    ) -> Self {
        Self {
            multisig,
            notification_service,
            program_client,
            state: Arc::new(Mutex::new(UpgradeServiceState::default())),
        }
    }

    #[instrument(skip(self))]
    pub async fn propose_upgrade(&self, new_program_buffer: Pubkey, version_label: &str) -> Result<String, UpgradeError> {
        let ix = self.program_client.build_upgrade_ix(&new_program_buffer).await?;
        let description = format!("Upgrade to {}", version_label);
        let proposal_id = self
            .multisig
            .propose_transaction(ProposalParams {
                instruction: ix,
                description: description.clone(),
                timelock_secs: 48 * 60 * 60,
            })
            .await?;

        self.notification_service
            .notify_community(Notification {
                notification_type: NotificationType::ProgramUpgrade,
                proposal_id: proposal_id.clone(),
                message: description,
                created_at: Utc::now(),
            })
            .await?;

        {
            let mut s = self.state.lock().await;
            s.open_proposals += 1;
        }

        Ok(proposal_id)
    }

    #[instrument(skip(self))]
    pub async fn approve_upgrade(&self, proposal_id: &str) -> Result<(), UpgradeError> {
        self.multisig.approve(proposal_id).await
    }

    #[instrument(skip(self))]
    pub async fn cancel_upgrade(&self, proposal_id: &str) -> Result<(), UpgradeError> {
        self.multisig.cancel(proposal_id).await?;
        self.notification_service
            .notify_community(Notification {
                notification_type: NotificationType::ProgramUpgradeCancelled,
                proposal_id: proposal_id.to_string(),
                message: "Upgrade proposal cancelled".into(),
                created_at: Utc::now(),
            })
            .await
    }

    #[instrument(skip(self))]
    pub async fn record_execution(&self, proposal_id: &str, new_program_buffer: &Pubkey, new_program_hash: [u8;32]) -> Result<(), UpgradeError> {
        self.program_client
            .record_upgrade_metadata(proposal_id, new_program_buffer, &new_program_hash)
            .await?;
        self.notification_service
            .notify_community(Notification {
                notification_type: NotificationType::ProgramUpgradeExecuted,
                proposal_id: proposal_id.to_string(),
                message: "Upgrade executed".into(),
                created_at: Utc::now(),
            })
            .await?;
        {
            let mut s = self.state.lock().await;
            if s.open_proposals > 0 {
                s.open_proposals -= 1;
            }
        }
        Ok(())
    }
}

// Simple in-memory stubs to ease local development
pub struct InMemoryMultisig;
#[async_trait]
impl MultisigManager for InMemoryMultisig {
    async fn propose_transaction(&self, params: ProposalParams) -> Result<String, UpgradeError> {
        debug!(?params.timelock_secs, "proposed");
        Ok(Uuid::new_v4().to_string())
    }
    async fn approve(&self, _proposal_id: &str) -> Result<(), UpgradeError> { Ok(()) }
    async fn cancel(&self, _proposal_id: &str) -> Result<(), UpgradeError> { Ok(()) }
}

pub struct LogNotifier;
#[async_trait]
impl NotificationService for LogNotifier {
    async fn notify_community(&self, n: Notification) -> Result<(), UpgradeError> {
        info!(kind=?n.notification_type, id = %n.proposal_id, msg = %n.message, "notify");
        Ok(())
    }
}

pub struct NoopProgramClient;
#[async_trait]
impl ProgramClient for NoopProgramClient {
    async fn build_upgrade_ix(&self, _new_program_buffer: &Pubkey) -> Result<Vec<u8>, UpgradeError> {
        Ok(vec![])
    }
    async fn record_upgrade_metadata(&self, _proposal_id: &str, _new_buffer: &Pubkey, _hash: &[u8;32]) -> Result<(), UpgradeError> {
        Ok(())
    }
}
