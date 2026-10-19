# Changelog

All notable changes to this repository's contracts will be documented in
this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project does not yet follow a formal versioning scheme — each contract's
`Cargo.toml` still reads `0.1.0` — so entries are grouped under
[Unreleased] until the first tagged release.

## [Unreleased]

### Added

- `donation-vault`: contract skeleton with stream storage schema.
- `donation-vault`: `create_stream`, `withdraw`, `cancel_stream`, `top_up`,
  and `modify_rate` for the streaming donation lifecycle.
- `donation-vault`: admin-gated `pause` / `unpause` for fund-moving actions.
- `donation-vault`: optional protocol fee (`set_fee_bps` / `fee_bps`) with
  treasury payout (`set_treasury` / `treasury`).
- `donation-vault`: `clear_token_fee_bps` to remove a per-token fee override,
  restoring the global fee. Emits `tokfeeclr`.
- `donation-vault`, `ngo-registry`: unit test coverage for `upgrade` entry
  points across both contracts, verifying admin authorization requirements,
  non-admin rejection, and state preservation.
- `donation-vault`: emit `treasset` event on `set_treasury` so off-chain
  indexers can track protocol fee destination changes.
- `donation-vault`: admin-configurable `cancel_grace_ledgers` retention for
  cancelled streams, with TTL coverage for indexing after cancellation.
- `donation-vault`: read-only `pending_accrual` view.
- `donation-vault`: regression coverage for one-stroop-per-second streams and
  their zero-rounded protocol fee.
- `DataKey` enums now derive `Debug` in both contracts for clearer storage-key
  diagnostics.
- `donation-vault`: read-only `allowed_tokens` getter for frontend token pickers.
- `donation-vault`: two-step admin transfer via `propose_admin` /
  `accept_admin`.
- `donation-vault`: reject self-admin proposals with `Error::InvalidAdmin`.
- `ngo-registry`: contract skeleton with storage types and `init`.
- `ngo-registry`: NGO application/registration via `register`.
- `ngo-registry`: admin-gated `approve_ngo`.
- `ngo-registry`: admin-gated `revoke_ngo`.
- `ngo-registry`: two-step admin transfer via `propose_admin` /
  `accept_admin`.
- `ngo-registry`: owner-gated `update_name` for fixing an application's
  name before approval; rejected with `Error::AlreadyVerified` after.
- `ngo-registry`: read-only `ngo_count` getter for total registered NGOs.
- Contract events for registry and vault state changes (see
  [`docs/EVENTS.md`](docs/EVENTS.md)).
- Document storage TTL policy, inactivity expiry risks, and keep-alive entry
  points (`extend_stream` and `touch_ngo`) in `README.md` and `docs/STORAGE.md`.
- `scripts/deploy-testnet.sh` for deploying both contracts to testnet.
- `scripts/deploy-mainnet.sh` for deploying to mainnet. It requires
  `--confirm`, pins the Public network passphrase, requires an explicit
  admin address, refuses to overwrite an existing entry, and records the
  contract IDs under a separate `mainnet` key in `deployments.json`.
  `deploy-testnet.sh` now preserves that key when it rewrites the file.
- CI workflow running `cargo fmt --check`, `cargo clippy`, a
  `wasm32v1-none` release build, and `cargo test --workspace`.
- README FAQ covering the license, release overflow checks, `no_std`, storage
  TTLs, and cancelled-stream retention.
- `docs/THREAT_MODEL.md`: assets, actors and trust boundaries, security
  invariants, and per-contract threat tables (authorization and accounting,
  availability and storage economics, registry identity, privilege/deployment
  and off-chain), plus a residual-risk register of open items.

### Changed

- **BREAKING** `donation-vault`: updated event data payloads to include net, fee,
  and settled amounts. `withdraw` now emits `(accrued: i128, fee: i128, net: i128)`
  instead of single-value `accrued`; `topup` now emits `(amount: i128, settled: i128)`
  instead of single-value `amount`; `ratemod` now emits `(old_rate: i128, new_rate: i128, settled: i128)`
  instead of `(old_rate, new_rate)`; and `transfer` emits `(old_ngo: Address, new_ngo: Address, settled: i128)`
  instead of `(old_ngo, new_ngo)`. Off-chain indexers and subscribers must update
  their event decoders for these new data shapes.
- `donation-vault`: `get_config` returns an extended `Config` struct that includes
  `min_deposit`, `max_streams_per_donor`, `registry`, `cancel_grace_ledgers`,
  `total_donated`, `pending_admin`, and `renounced`. Clients decoding the old
  struct shape will need updating.
- `donation-vault`: stream `balance`/`withdrawn` updates and the stream-id
  counter use checked arithmetic, returning `Error::ArithmeticOverflow`
  instead of panicking on overflow.
- `donation-vault`: `create_stream` rejects a stream whose `donor` and `ngo`
  are the same address with `Error::SelfStream`, so a deposit can't be
  counted as a committed donation while streaming straight back to its donor.
- Both contracts migrated event declarations from deprecated
  `Events::publish` to explicit `#[contractevent]` types. Topic names and
  positional payload formats are declared in code and documented in
  `docs/EVENTS.md`; consumers should update to the generated event types
  before the next contract release.

### Fixed

- `donation-vault`: `set_treasury` rejects the vault's own address with
  `Error::InvalidTreasury`, so protocol fees can't be locked in the
  contract.

[Unreleased]: https://github.com/StreamGive/streamgive-contracts/compare/main...HEAD
