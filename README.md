# StreamGive — Contracts

Soroban smart contracts powering StreamGive, a recurring/streaming donation
platform for verified NGOs on Stellar.

For how these contracts fit with the backend and frontend — and how a
donation flows end to end — see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
For who these contracts defend against, what the admin can and cannot do, and
which risks are knowingly accepted, see
[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).

## Contracts

- `ngo-registry` — on-chain NGO application, verification, and registry
- `donation-vault` — streaming donation vault (create / withdraw / cancel / modify streams)

### Admin transfer (donation-vault and ngo-registry)

Both contracts use the same two-step handshake. The registry has no pause
switch, but its admin transfer is otherwise identical.

| Step | Entry point | Who may call it | Effect |
| --- | --- | --- | --- |
| 1 | `propose_admin(new_admin)` | The **current admin** (admin auth) | Records `new_admin` as pending. The active admin does not change. |
| 2 | `accept_admin()` | The **pending admin only** (pending address auth) | Promotes the pending address to active admin and clears the proposal. |
| 3 | `cancel_admin_proposal()` | The **current admin only** (admin auth) | Clears the pending proposal. The active admin does not change. |

Only the current admin can propose or cancel. The pending address cannot
cancel; it can only accept. Until `accept_admin()` succeeds, the current admin
continues to control every admin-gated operation. In `donation-vault`, if the
current admin renounces, both the active and pending slots are cleared; the
registry has no renounce entry point.

## Release profile

The workspace `Cargo.toml`'s `[profile.release]` sets several non-default
flags. Soroban's resource-fee model charges per byte of the deployed wasm
and per CPU instruction executed, so a smaller, more predictable binary
isn't just nice-to-have — it directly lowers what every invocation of
these contracts costs:

| Setting             | Value       | Why                                                                                                   |
| -------------------- | ----------- | ------------------------------------------------------------------------------------------------------ |
| `opt-level`          | `"z"`       | Optimizes for binary size over speed — wasm size drives upload and storage fees.                       |
| `lto`                | `true`      | Whole-program link-time optimization, trimming dead code and shrinking the binary further.             |
| `codegen-units`      | `1`         | A single codegen unit gives the optimizer the whole crate to work with, trading build time for smaller output. |
| `panic`              | `"abort"`   | Drops unwinding tables and landing pads; Soroban traps on panic and can't unwind across the host boundary anyway. |
| `strip`              | `"symbols"` | Strips symbol/debug info from the deployed artifact — of no use on-chain, pure size cost otherwise.     |
| `debug`              | `0`         | No debug info emitted for release builds, same rationale as `strip`.                                    |
| `debug-assertions`   | `false`     | Standard release behavior — keeps hot paths free of debug-only checks.                                  |
| `overflow-checks`    | `true`      | Kept **on** in release, contrary to the Rust default — these contracts move token balances, and a silently wrapped `i128` is far worse than the small extra cost of a checked op. |

Change these with care: relaxing `opt-level`, `lto`, or `strip` grows the
deployed wasm and raises fees, while turning `overflow-checks` off would
let balance arithmetic wrap silently.

## Pausing

`donation-vault` has an admin-gated `pause` / `unpause` pair — an emergency
brake for when something is wrong. `pause` only flips the `Paused` flag in
instance storage: no funds are moved by the call itself, so balances stay where
they were and there is nothing to unwind when the pause is lifted.

While paused, the vault is not fully frozen. The state-changing entry points
that move funds or mutate stream state reject with `Error::ContractPaused`;
donor cancellation and the exceptional admin rescue path stay available. Every
other entry point, including all read-only views and the permissionless
keep-alive, continues to work. The table below lists every public entry point.

| Entry point | While paused |
| --- | --- |
| `create_stream` | **Rejected** — `ContractPaused` |
| `withdraw` | **Rejected** — `ContractPaused` |
| `withdraw_batch` | **Rejected** — `ContractPaused` |
| `top_up` | **Rejected** — `ContractPaused` |
| `modify_rate` | **Rejected** — `ContractPaused` |
| `transfer_stream` | **Rejected** — `ContractPaused` |
| `cancel_stream` | **Works** — donor auth; settles accrued payouts and refunds the unspent balance |
| `rescue_stream` | **Only works while paused** — admin-only emergency path; returns `NotPaused` when the vault is active |
| `pause` | **Rejected** when already paused — `AlreadyPaused` |
| `unpause` | **Works** — admin auth; lifts the pause |
| `renounce_admin` | **Works** — admin auth; no pause gate |
| `extend_stream` | **Works** — permissionless TTL bump; no pause gate |
| `init`, `__constructor` | **Works** during setup; neither has a pause gate (the vault cannot be paused before it is initialised) |
| `propose_admin`, `accept_admin`, `cancel_admin_proposal` | **Work** — admin handshake is not paused |
| `set_treasury`, `clear_treasury`, `set_fee_bps`, `set_token_fee_bps`, `clear_token_fee_bps` | **Work** — admin-gated configuration; no pause gate |
| `set_min_deposit`, `set_cancel_grace_ledgers`, `set_max_streams_per_donor`, `set_registry`, `upgrade` | **Work** — admin-gated configuration; no pause gate |
| `total_donated`, `admin`, `pending_admin`, `get_stream`, `streams_by_donor`, `stream_count`, `pending_accrual`, `depletion_time`, `pending_payout`, `paused`, `get_config`, `treasury`, `fee_bps`, `max_fee_bps`, `token_fee_bps`, `allowed_tokens`, `min_deposit`, `cancel_grace_ledgers`, `max_streams_per_donor`, `registry` | **Work** — read-only views are never paused |

Pausing does **not** stop time-based accrual. A stream's `pending_accrual`
keeps growing while the vault is paused, so a stream paused for a week still
owes a week of accrual once the pause is lifted. That accrual is claimable via
`withdraw` as soon as the vault is unpaused.

`withdraw` and `withdraw_batch` being rejected is the point of the brake: they
are the paths that pay tokens straight out of the vault, so a pause triggered
by a suspected vulnerability has to close them or an attacker could drain funds
while the rest of the contract is frozen.

`cancel_stream` is deliberately left open. It is the one ordinary path that
returns money to a donor, so keeping it available means a pause can never trap
a donor's unspent deposit. `rescue_stream` is the opposite: it exists only for
incident response, is admin-only, and deliberately refuses to run while the
vault is active.

## Storage TTL and keeping entries alive

Soroban contracts use bounded Time-To-Live (TTL) for on-chain state retention:

- **Instance storage** (contract admins, configuration, pause flags) is bumped
  to 30 days (`518,400` ledgers) on every state-changing call.
- **Persistent storage** (each individual `Stream` record in `donation-vault` and
  each `Ngo` record in `ngo-registry`) has an independent 90-day TTL (`1,555,200`
  ledgers) that must be maintained per entry.

### Expiry risk for idle entries

If a stream has no activity (withdrawals, top-ups, rate modifications) or an NGO
entry receives no updates for 90 consecutive days, its TTL expires and the network
**archives** the entry.

Archived entries cannot be read or modified by normal contract calls (`get_stream`,
`withdraw`, verification lookups, etc. will fail) until a Soroban state restoration
transaction is submitted and network restoration fees are paid. Long-running streams
with low drip rates or infrequent withdrawals are especially at risk if left untouched.

### Keep-alive entry points (`extend_stream` & `touch_ngo`)

To protect idle entries from archival without moving funds, modifying balances, or
requiring admin credentials, both contracts provide permissionless keep-alive entry
points that anyone (donors, NGOs, keeper bots, or indexers) can invoke:

- **`donation-vault::extend_stream(stream_id)`**: Extends the persistent storage
  TTL of `DataKey::Stream(stream_id)` back to 90 days. Can be called at any time,
  requires no authorization, and remains accessible even while the vault is paused.
- **`ngo-registry::touch_ngo(owner)`**: Refreshes both the registry instance TTL
  (to 30 days) and the NGO's persistent storage TTL (`DataKey::Ngo(owner)`) back
  to 90 days without altering registration status. Requires no authorization.

For full key specifications, bump thresholds, and archival lifecycle details, see
[docs/STORAGE.md](docs/STORAGE.md).

## Related repositories

- [streamgive-backend](https://github.com/streamgive/streamgive-backend) — indexer & API
- [streamgive-frontend](https://github.com/streamgive/streamgive-frontend) — donor & NGO web app
- [streamgive-docs](https://github.com/streamgive/streamgive-docs) — documentation

## Testing

Run the full test suite for all contracts from the workspace root:

```sh
cargo test --workspace
```

To run tests for a single contract:

```sh
cargo test -p donation-vault
cargo test -p ngo-registry
```

Notable coverage:

- `donation-vault`'s `math` module unit-tests the streaming accrual
  calculation (`accrued`) directly: zero/negative rate, zero balance,
  zero elapsed time, capping at the remaining balance, and saturating
  instead of overflowing/panicking near `i128::MAX`.
- It also includes a deterministic grid-based invariant sweep
  (`invariants_hold_across_a_grid_of_inputs`) that checks, across a
  matrix of rates, balances, and elapsed durations, that accrual is
  always non-negative, never exceeds the remaining balance, and is
  monotonically non-decreasing as elapsed time (or rate) grows — a
  stand-in for property-based testing over the streaming math's edge
  cases.

CI (see [`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs
`cargo fmt --check`, `cargo clippy`, a `wasm32v1-none` release
build, a wasm binary size check (see
[`scripts/check-wasm-size.sh`](scripts/check-wasm-size.sh)), and
`cargo test --workspace` on every push and pull request.

## FAQ

### Why is this project licensed under Apache-2.0?

Apache-2.0 permits reuse and modification while providing an explicit patent
license and clear contributor protections. That makes it a practical default
for contracts intended to be integrated by wallets, applications, and other
open-source projects.

### Why are release overflow checks enabled?

The contracts move token balances and calculate payouts with `i128`. A wrapped
balance could silently corrupt funds, so release builds keep `overflow-checks`
enabled and return explicit arithmetic errors where the contract can handle
the failure.

### Why are the contracts `no_std`?

Soroban contracts run in a constrained WebAssembly environment. `no_std`
keeps the deployed artifact small and avoids bringing operating-system
facilities that are unavailable on-chain.

### Why does each stream have its own TTL?

Persistent storage is retained per key. A stream that is never touched can
expire independently of the vault instance, so state-changing calls and the
permissionless `extend_stream` entry point refresh the specific stream that
needs to remain available.

### What is the cancelled-stream grace period?

The admin can configure `cancel_grace_ledgers` so indexers have additional
time to observe and process a cancellation. Cancelling a stream retains its
record for the normal stream TTL plus that configured grace period; a value of
zero keeps the default retention period.

## Error codes

Each contract exposes its failures as a `#[contracterror] enum Error`,
returned as `Result<_, Error>` from every fallible entry point. Clients see
the numeric code below (e.g. a failed `try_withdraw` surfacing `Error(5)`).

### `donation-vault`

Codes 1–23 are the full `Error` enum. "Admin-gated" entry points first call
`require_admin`, so they can also return `NotInitialized` (no admin) or
`AdminRenounced`.

| Code | Error | Meaning | Entry points |
| --- | --- | --- | --- |
| 1 | `AlreadyInitialized` | `init` already created an admin. | `init` |
| 2 | `NotInitialized` | No admin is stored. | `admin`, `propose_admin`, `cancel_admin_proposal`, `pause`, `unpause`, `set_treasury`, `clear_treasury`, `set_fee_bps`, `set_token_fee_bps`, `clear_token_fee_bps`, `set_min_deposit`, `set_cancel_grace_ledgers`, `set_max_streams_per_donor`, `set_registry`, `renounce_admin`, `rescue_stream`, `upgrade` |
| 3 | `StreamNotFound` | No stream exists for the id. | `get_stream`, `pending_accrual`, `depletion_time`, `pending_payout`, `extend_stream`, `withdraw`, `withdraw_batch`, `cancel_stream`, `rescue_stream`, `top_up`, `modify_rate`, `transfer_stream` |
| 4 | `InvalidAmount` | A deposit, rate, top-up, rate change, or minimum deposit was zero/negative (or a fee would swallow the first payout). | `create_stream`, `top_up`, `modify_rate`, `set_min_deposit` |
| 5 | `NothingToWithdraw` | The stream accrued nothing since its last checkpoint. | `withdraw` |
| 6 | `ContractPaused` | The admin paused the vault; see [Pausing](#pausing). | `create_stream`, `withdraw`, `withdraw_batch`, `top_up`, `modify_rate`, `transfer_stream` |
| 7 | `FeeTooHigh` | Fee above the 1,000 bps (10%) cap. | `set_fee_bps`, `set_token_fee_bps` |
| 8 | `NoPendingAdmin` | No admin proposal is pending. | `accept_admin`, `cancel_admin_proposal` |
| 9 | `ArithmeticOverflow` | Checked balance, total, counter, or grace-ledger arithmetic left `i128`/`u64` range. | `create_stream`, `withdraw`, `withdraw_batch`, `cancel_stream`, `top_up`, `modify_rate`, `transfer_stream`, `set_cancel_grace_ledgers` |
| 10 | `SelfStream` | Donor and NGO are the same address. | `create_stream` |
| 11 | `InvalidTreasury` | Treasury address is the vault itself. | `set_treasury` |
| 12 | `AlreadyPaused` | `pause` called while already paused. | `pause` |
| 13 | `AlreadyUnpaused` | `unpause` called while already active. | `unpause` |
| 14 | `DepositTooLow` | Deposit is below the configured `min_deposit`. | `create_stream` |
| 15 | `StreamCancelled` | The stream was already closed out by `cancel_stream`/`rescue_stream`. | `top_up`, `modify_rate`, `transfer_stream`, `rescue_stream` |
| 16 | `InvalidAdmin` | The proposed admin equals the current admin. | `propose_admin` |
| 17 | `StreamLimitExceeded` | Donor reached `max_streams_per_donor`. | `create_stream` |
| 18 | `NgoNotVerified` | A configured registry has no verified entry for the NGO. | `create_stream` |
| 19 | `StreamCounterMissing` | The stream-id counter is absent from instance storage. | `create_stream` |
| 20 | `AdminRenounced` | Admin control was permanently renounced, or `init` ran after a renounce. | `init`, `admin`, `propose_admin`, `cancel_admin_proposal`, `pause`, `unpause`, `set_treasury`, `clear_treasury`, `set_fee_bps`, `set_token_fee_bps`, `clear_token_fee_bps`, `set_min_deposit`, `set_cancel_grace_ledgers`, `set_max_streams_per_donor`, `set_registry`, `renounce_admin`, `rescue_stream`, `upgrade` |
| 21 | `FeeRequiresTreasury` | A non-zero fee was set with no treasury, or the treasury was cleared while a fee is set. | `set_fee_bps`, `set_token_fee_bps`, `clear_treasury` |
| 22 | `NotPaused` | `rescue_stream` called while the vault is active. | `rescue_stream` |
| 23 | `MixedNgo` | A batch withdrawal mixed streams belonging to different NGOs. | `withdraw_batch` |

### `ngo-registry`

Codes 1–11 are the full `Error` enum. Admin-gated entry points first call
`require_admin`, so they can also return `NotInitialized`.

| Code | Error | Meaning | Entry points |
| --- | --- | --- | --- |
| 1 | `AlreadyInitialized` | `init` already created an admin. | `init` |
| 2 | `NotInitialized` | No admin is stored. | `admin`, `propose_admin`, `cancel_admin_proposal`, `approve_ngo`, `revoke_ngo`, `upgrade` |
| 3 | `AlreadyRegistered` | The owner already has a registry entry. | `register` |
| 4 | `NotRegistered` | No entry exists for the owner. | `unregister`, `update_name`, `get_ngo`, `approve_ngo`, `revoke_ngo`, `touch_ngo` |
| 5 | `AlreadyVerified` | The action is not allowed once an NGO is verified: unregister, rename, or approve again. | `unregister`, `update_name`, `approve_ngo` |
| 6 | `NameTooLong` | Name exceeds 200 bytes. | `register`, `update_name` |
| 7 | `NotVerified` | `revoke_ngo` called on an NGO that is not verified. | `revoke_ngo` |
| 8 | `NoPendingAdmin` | No admin proposal is pending. | `accept_admin`, `cancel_admin_proposal` |
| 9 | `InvalidAdmin` | The proposed admin equals the current admin. | `propose_admin` |
| 10 | `ArithmeticOverflow` | The owner index or total count left `u32`/`u64` range. | `register` |
| 11 | `InvalidName` | The name is empty or whitespace-only. | `register` |

## Status

Early development.

## License

Apache-2.0 — see [LICENSE](./LICENSE).
