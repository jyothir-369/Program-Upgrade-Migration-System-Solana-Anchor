use anchor_lang::prelude::*;

declare_id!("UpgrdMgr1111111111111111111111111111111111");

#[program]
pub mod upgrade_manager {
    use super::*;

    pub fn init_config(ctx: Context<InitConfig>, members: Vec<Pubkey>, threshold: u8, timelock_duration: i64) -> Result<()> {
        let cfg = &mut ctx.accounts.multisig_config;
        cfg.members = members;
        cfg.threshold = threshold;
        cfg.upgrade_authority = ctx.accounts.authority.key();
        let st = &mut ctx.accounts.program_state;
        st.authority = cfg.key();
        st.upgrade_buffer = Pubkey::default();
        st.timelock_duration = timelock_duration.max(48 * 60 * 60);
        st.pending_upgrade = None;
        Ok(())
    }

    pub fn propose_upgrade(ctx: Context<ProposeUpgrade>, new_program_buffer: Pubkey, description: String) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;
        proposal.id = ctx.accounts.proposal.key().to_bytes()[..8].try_into().map(u64::from_le_bytes).unwrap_or(0);
        proposal.proposer = ctx.accounts.proposer.key();
        proposal.program = ctx.accounts.target_program.key();
        proposal.new_buffer = new_program_buffer;
        proposal.description = description;
        proposal.proposed_at = clock.unix_timestamp;
        proposal.timelock_until = 0;
        proposal.approvals = vec![];
        proposal.approval_threshold = ctx.accounts.multisig_config.threshold;
        proposal.status = UpgradeStatus::Proposed;
        proposal.executed_at = None;
        emit!(ProposalEvent { proposal: proposal.key(), program: proposal.program, buffer: new_program_buffer });
        Ok(())
    }

    pub fn approve_upgrade(ctx: Context<ApproveUpgrade>, _proposal_id: Pubkey) -> Result<()> {
        let cfg = &ctx.accounts.multisig_config;
        require!(cfg.members.contains(&ctx.accounts.approver.key()), UpgradeError::NotMultisigMember);
        let proposal = &mut ctx.accounts.proposal;
        require!(!proposal.approvals.contains(&ctx.accounts.approver.key()), UpgradeError::AlreadyApproved);
        require!(proposal.status == UpgradeStatus::Proposed || proposal.status == UpgradeStatus::Approved, UpgradeError::InvalidStatus);
        proposal.approvals.push(ctx.accounts.approver.key());
        if (proposal.approvals.len() as u8) >= proposal.approval_threshold {
            let clock = Clock::get()?;
            proposal.status = UpgradeStatus::TimelockActive;
            proposal.timelock_until = clock.unix_timestamp + ctx.accounts.program_state.timelock_duration;
        } else {
            proposal.status = UpgradeStatus::Approved;
        }
        Ok(())
    }

    pub fn execute_upgrade(ctx: Context<ExecuteUpgrade>, _proposal_id: Pubkey, new_program_hash: [u8;32]) -> Result<()> {
        let clock = Clock::get()?;
        let proposal = &mut ctx.accounts.proposal;
        require!(proposal.status == UpgradeStatus::TimelockActive, UpgradeError::InvalidStatus);
        require!(clock.unix_timestamp >= proposal.timelock_until, UpgradeError::TimelockNotElapsed);
        require!((proposal.approvals.len() as u8) >= proposal.approval_threshold, UpgradeError::InsufficientApprovals);
        let st = &mut ctx.accounts.program_state;
        st.pending_upgrade = Some(PendingUpgrade {
            new_program_hash,
            scheduled_time: clock.unix_timestamp,
            proposal_time: proposal.proposed_at,
            approved_by: proposal.approvals.clone(),
        });
        proposal.status = UpgradeStatus::Executed;
        proposal.executed_at = Some(clock.unix_timestamp);
        emit!(UpgradeExecutedEvent { proposal: proposal.key(), program: proposal.program });
        Ok(())
    }

    pub fn cancel_upgrade(ctx: Context<CancelUpgrade>, _proposal_id: Pubkey) -> Result<()> {
        let cfg = &ctx.accounts.multisig_config;
        require!(cfg.members.contains(&ctx.accounts.canceller.key()), UpgradeError::NotMultisigMember);
        let proposal = &mut ctx.accounts.proposal;
        require!(proposal.status != UpgradeStatus::Executed, UpgradeError::AlreadyExecuted);
        proposal.status = UpgradeStatus::Cancelled;
        Ok(())
    }

    pub fn migrate_account(ctx: Context<MigrateAccount>, _old_account: Pubkey) -> Result<()> {
        let clock = Clock::get()?;
        let ver = &mut ctx.accounts.account_version;
        require!(!ver.migrated, UpgradeError::AlreadyMigrated);
        ver.migrated = true;
        ver.migrated_at = Some(clock.unix_timestamp);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + MultisigConfig::MAX_SIZE)]
    pub multisig_config: Account<'info, MultisigConfig>,
    #[account(init, payer = authority, space = 8 + ProgramUpgradeState::MAX_SIZE, seeds=[b"program_state"], bump)]
    pub program_state: Account<'info, ProgramUpgradeState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ProposeUpgrade<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,
    /// CHECK: program
    pub target_program: UncheckedAccount<'info>,
    #[account(mut, has_one = upgrade_authority)]
    pub multisig_config: Account<'info, MultisigConfig>,
    pub upgrade_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub program_state: Account<'info, ProgramUpgradeState>,
    #[account(init, payer = proposer, space = 8 + UpgradeProposal::MAX_SIZE, seeds=[b"proposal", target_program.key().as_ref(), proposer.key().as_ref(), program_state.key().as_ref()], bump)]
    pub proposal: Account<'info, UpgradeProposal>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApproveUpgrade<'info> {
    #[account(mut)]
    pub approver: Signer<'info>,
    #[account(has_one = upgrade_authority)]
    pub multisig_config: Account<'info, MultisigConfig>,
    #[account(mut, seeds=[b"program_state"], bump)]
    pub program_state: Account<'info, ProgramUpgradeState>,
    #[account(mut)]
    pub proposal: Account<'info, UpgradeProposal>,
}

#[derive(Accounts)]
pub struct ExecuteUpgrade<'info> {
    pub executor: Signer<'info>,
    #[account(has_one = upgrade_authority)]
    pub multisig_config: Account<'info, MultisigConfig>,
    #[account(mut, seeds=[b"program_state"], bump)]
    pub program_state: Account<'info, ProgramUpgradeState>,
    #[account(mut)]
    pub proposal: Account<'info, UpgradeProposal>,
}

#[derive(Accounts)]
pub struct CancelUpgrade<'info> {
    #[account(mut)]
    pub canceller: Signer<'info>,
    #[account(has_one = upgrade_authority)]
    pub multisig_config: Account<'info, MultisigConfig>,
    #[account(mut)]
    pub proposal: Account<'info, UpgradeProposal>,
}

#[derive(Accounts)]
pub struct MigrateAccount<'info> {
    pub migrator: Signer<'info>,
    #[account(init_if_needed, payer = migrator, space = 8 + AccountVersion::MAX_SIZE, seeds=[b"acct_ver", migrator.key().as_ref()], bump)]
    pub account_version: Account<'info, AccountVersion>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct UpgradeProposal {
    pub id: u64,
    pub proposer: Pubkey,
    pub program: Pubkey,
    pub new_buffer: Pubkey,
    pub description: String,
    pub proposed_at: i64,
    pub timelock_until: i64,
    pub approvals: Vec<Pubkey>,
    pub approval_threshold: u8,
    pub status: UpgradeStatus,
    pub executed_at: Option<i64>,
}

impl UpgradeProposal {
    pub const MAX_DESC: usize = 256;
    pub const MAX_APPROVALS: usize = 16;
    pub const MAX_SIZE: usize = 8 + 32 + 32 + 32 + 4 + Self::MAX_DESC + 8 + 8 + 4 + (Self::MAX_APPROVALS * 32) + 1 + 1 + 9;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum UpgradeStatus {
    Proposed,
    Approved,
    TimelockActive,
    Executed,
    Cancelled,
}

#[account]
pub struct MultisigConfig {
    pub members: Vec<Pubkey>,
    pub threshold: u8,
    pub upgrade_authority: Pubkey,
}

impl MultisigConfig {
    pub const MAX_MEMBERS: usize = 8;
    pub const MAX_SIZE: usize = 4 + (Self::MAX_MEMBERS * 32) + 1 + 32;
}

#[account]
pub struct ProgramUpgradeState {
    pub authority: Pubkey,
    pub upgrade_buffer: Pubkey,
    pub timelock_duration: i64,
    pub pending_upgrade: Option<PendingUpgrade>,
}

impl ProgramUpgradeState {
    pub const MAX_SIZE: usize = 32 + 32 + 8 + (1 + PendingUpgrade::MAX_SIZE);
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct PendingUpgrade {
    pub new_program_hash: [u8; 32],
    pub scheduled_time: i64,
    pub proposal_time: i64,
    pub approved_by: Vec<Pubkey>,
}

impl PendingUpgrade {
    pub const MAX_APPROVERS: usize = 16;
    pub const MAX_SIZE: usize = 32 + 8 + 8 + 4 + (Self::MAX_APPROVERS * 32);
}

#[account]
pub struct AccountVersion {
    pub version: u32,
    pub migrated: bool,
    pub migrated_at: Option<i64>,
}

impl AccountVersion {
    pub const MAX_SIZE: usize = 4 + 1 + 9;
}

#[event]
pub struct ProposalEvent {
    pub proposal: Pubkey,
    pub program: Pubkey,
    pub buffer: Pubkey,
}

#[event]
pub struct UpgradeExecutedEvent {
    pub proposal: Pubkey,
    pub program: Pubkey,
}

#[error_code]
pub enum UpgradeError {
    #[msg("not multisig member")]
    NotMultisigMember,
    #[msg("already approved")]
    AlreadyApproved,
    #[msg("invalid status")]
    InvalidStatus,
    #[msg("timelock not elapsed")]
    TimelockNotElapsed,
    #[msg("insufficient approvals")]
    InsufficientApprovals,
    #[msg("already executed")]
    AlreadyExecuted,
    #[msg("already migrated")]
    AlreadyMigrated,
}
