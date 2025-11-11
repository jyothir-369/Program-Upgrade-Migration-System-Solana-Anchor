# Program Upgrade & Migration System (Solana / Anchor)

A secure, governance-controlled upgrade and state migration framework for a decentralized perpetual futures exchange on Solana. Includes:

- Anchor on-chain program for proposals, approvals, timelock, execution recording, cancellations, and per-account migrations.
- Rust backend service scaffolding for proposal orchestration (multisig + notifications + client integration).
- PostgreSQL schema for audit trail (proposals, approvals, executions, migrations, logs).

---

## 1) Prerequisites

- Windows 10/11
- Git
- Rust toolchain (1.75+)
  - Install: https://rustup.rs/
  - After install: `rustup default stable`
- Solana CLI (v1.18.x recommended)
  - Download: https://docs.solana.com/cli/install-solana-cli-tools
  - Verify: `solana --version`
- Node.js LTS (for Anchor tooling)
  - Download: https://nodejs.org/en/download/
- Anchor Framework 0.29+
  - Install: `npm i -g @coral-xyz/anchor-cli`
  - Verify: `anchor --version`
- PostgreSQL 14+
  - Download: https://www.postgresql.org/download/
  - Ensure `psql` is in PATH

Optional (recommended):
- VS Code with Rust-Analyzer
- `sqlx-cli` if you plan to manage DB migrations via Rust (not wired yet)

---

## 2) Clone and Configure

```powershell
git clone https://github.com/nevilsonani/goquant.git

```

Project layout:

- `programs/upgrade_manager` — Anchor program
- `backend/upgrade_service` — Rust backend library (service skeleton)
- `db/migrations` — SQL schema for audit/history
- `Anchor.toml`, `Cargo.toml` — workspace configs

---

## 3) Solana Local Environment

Set up a local validator keypair and airdrop for tests.

```powershell
# Generate a local keypair if not present
solana-keygen new --no-bip39-passphrase

# Use localnet
solana config set --url localhost

# Start a local validator in a separate terminal (or use Solana-Test-Validator in another shell)
solana-test-validator --reset

# Airdrop some SOL for fees
solana airdrop 10
```

---

## 4) Build Anchor Program

```powershell
# From repo root
anchor build
```

This compiles the program in `programs/upgrade_manager`. The program id currently set to a placeholder:

- `declare_id!("UpgrdMgr1111111111111111111111111111111111");`

To use a real program ID on localnet or devnet:

1. Create a new keypair for the program:
   ```powershell
   solana-keygen new -o target/deploy/upgrade_manager-keypair.json --no-bip39-passphrase
   ```
2. Update `Anchor.toml` `[programs.localnet]` with the new program id from `solana address -k target/deploy/upgrade_manager-keypair.json`.
3. Update `declare_id!("<PROGRAM_ID>");` in `programs/upgrade_manager/src/lib.rs`.
4. Rebuild: `anchor build`.

---

## 5) Deploy Anchor Program (localnet)

```powershell
# With local validator running
anchor deploy
```

This uploads the program and buffer to your local validator and writes IDL to `target/idl`.

---

## 6) Backend Service (Library) Build

The backend is a Rust library with service scaffolding. Build the workspace:

```powershell
cargo build --workspace
```

You can integrate `backend/upgrade_service` into a binary or server of your choice. It provides:

- `ProgramUpgrade` service with methods to propose/approve/cancel and record execution.
- Traits to integrate with Squads multisig, notification channels, and an Anchor client.

---

## 7) Database Setup

Create a database and apply the initial schema.

```powershell
# Start PostgreSQL locally, then:
psql -U postgres -h localhost -c "CREATE DATABASE upgrade_mgmt;"

# Apply migrations (using psql here; replace user/host/db as needed)
psql -U postgres -h localhost -d upgrade_mgmt -f db/migrations/001_init.sql
```

Recommended tables were created:

- `upgrade_proposals`, `upgrade_approvals`, `upgrade_executions`
- `account_migrations`, `audit_logs`

---

## 8) Typical Local Flow

1. Build and deploy the program to localnet.
2. Initialize config (multisig + timelock) using an Anchor client script or program-specific instruction invoker.
3. Propose an upgrade with a buffer pubkey recorded on-chain.
4. Approvals via multisig members until threshold met; timelock is activated.
5. After 48h (or configured duration on local), execute: record `new_program_hash` and proceed with off-chain loader upgrade by governance.
6. Use `migrate_account` to mark/perform account migrations.

Note: The actual program upgrade (invoking the BPF upgradeable loader) is done by the governance/upgrade authority transaction, not by this program. This program records and enforces policy/timelock/approvals.

---

## 9) Environment Notes

- `Anchor.toml` sets:
  - `cluster = localnet`
  - program name `upgrade_manager`
- Update the program ID in both `Anchor.toml` and `declare_id!` before deploying beyond localnet.
- Ensure your wallet at `~/.config/solana/id.json` has sufficient SOL on the selected cluster.

---

## 10) Next Integrations (Optional)

- Implement `MultisigManager` with Squads SDK to create proposals and approvals on-chain.
- Implement `ProgramClient` using Anchor client to send program instructions.
- Add an HTTP/CLI front-end for operations and rich notifications (Discord/Twitter/Email).
- Add buffer hash verification (e.g., SHA-256 of `ProgramData`) before execution.
- Add end-to-end tests (localnet) for propose→approve→timelock→execute flow.

---

## Troubleshooting

- If `anchor build` fails on Windows:
  - Ensure `llvm` and `clang` are installed (Anchor bundles build tools typically via npm cli; make sure Node is installed).
  - Use a recent Rust stable and Solana CLI 1.18.x.
- If deployment fails on localnet:
  - Confirm `solana-test-validator` is running.
  - Re-airdrop: `solana airdrop 10`.
  - Verify program id consistency between `Anchor.toml` and `declare_id!`.

---

## Security Checklist (before mainnet)

- Multisig membership and threshold finalized.
- Minimum 48h timelock enforced in `init_config`.
- Buffer hash verified pre-execution via backend and governance flow.
- Audit logs and DB persistence enabled.
- Program and migration routines audited.


