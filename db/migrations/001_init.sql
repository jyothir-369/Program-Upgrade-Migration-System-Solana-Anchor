-- Upgrade/Audit schema
CREATE TABLE IF NOT EXISTS upgrade_proposals (
    id UUID PRIMARY KEY,
    onchain_proposal_pubkey TEXT NOT NULL,
    target_program TEXT NOT NULL,
    new_buffer_pubkey TEXT NOT NULL,
    description TEXT NOT NULL,
    proposed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    timelock_until TIMESTAMPTZ,
    approval_threshold SMALLINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('Proposed','Approved','TimelockActive','Executed','Cancelled')),
    executed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS upgrade_approvals (
    id UUID PRIMARY KEY,
    proposal_id UUID NOT NULL REFERENCES upgrade_proposals(id) ON DELETE CASCADE,
    approver_pubkey TEXT NOT NULL,
    approved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(proposal_id, approver_pubkey)
);

CREATE TABLE IF NOT EXISTS upgrade_executions (
    id UUID PRIMARY KEY,
    proposal_id UUID NOT NULL REFERENCES upgrade_proposals(id) ON DELETE CASCADE,
    buffer_hash BYTEA NOT NULL,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS account_migrations (
    id UUID PRIMARY KEY,
    account_pubkey TEXT NOT NULL,
    from_version INTEGER NOT NULL,
    to_version INTEGER NOT NULL,
    migrated BOOLEAN NOT NULL DEFAULT FALSE,
    migrated_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY,
    actor_pubkey TEXT,
    action TEXT NOT NULL,
    proposal_id UUID,
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_upgrade_proposals_status ON upgrade_proposals(status);
CREATE INDEX IF NOT EXISTS idx_upgrade_approvals_proposal ON upgrade_approvals(proposal_id);
CREATE INDEX IF NOT EXISTS idx_account_migrations_account ON account_migrations(account_pubkey);
