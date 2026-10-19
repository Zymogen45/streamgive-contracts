# Contract events

Every event published by the contracts in this repo, with its topics and
data shape, so an indexer can be written directly against this repo without
going to `streamgisve-docs` first.

Soroban events have two parts:

- **Topics** — a tuple, always starting with a `Symbol` naming the event.
  Topics are indexed/filterable.
- **Data** — the event payload. Shown below as the Rust type(s) passed to
  `env.events().publish((topics...), data)`. A single value publishes as
  itself; a tuple of values publishes as an XDR array in that order.

The contracts define migrated event declarations with Soroban SDK's
`#[contractevent]` macro. Explicit topic names and data formats keep the wire
layout stable. Event type names include `RegisterEvent`, `RenamedEvent`,
`ApprovedEvent`, `RevokedEvent`, `UnregisteredEvent`, `RemovedEvent`,
`ProposedAdminEvent`, `AcceptedAdminEvent`, `CancelledAdminEvent`,
`PausedEvent`, `UnpausedEvent`, `TreasurySetEvent`, `TreasuryClearedEvent`,
`FeeBpsSetEvent`, `TokenFeeBpsSetEvent`, `TokenFeeBpsClearedEvent`,
`MinDepositSetEvent`, `CancelGraceLedgersSetEvent`, `RegistrySetEvent`,
`MaxStreamsPerDonorSetEvent`, `InitializedEvent`, `UpgradedEvent`,
`CreatedEvent`, `WithdrawnEvent`, `CancelledStreamEvent`, `RescuedStreamEvent`,
`ToppedUpEvent`, and `RateModifiedEvent`.

`Address`, `i128`, `u64`, `u32`, `bool`, and `String` are the standard
Soroban SDK/XDR types (`Address` is the SDK's account/contract address
type, `String` is `soroban_sdk::String`).

## `ngo-registry`

### `register`

Emitted by `register` when an NGO submits an application.

| | |
|---|---|
| Topics | `("register", owner: Address)` |
| Data | `name: String` |

The entry starts unverified (`verified: false`); look for a matching
`approved` event for the same `owner` to know when it's live.

### `renamed`

Emitted by `update_name` when an NGO changes the name on its application
before it has been approved.

| | |
|---|---|
| Topics | `("renamed", owner: Address)` |
| Data | `name: String` (the new name) |

Once an `approved` event has been seen for an `owner`, no further
`renamed` events can follow for it while it stays verified.

### `approved`

Emitted by `approve_ngo` when an admin marks a registered NGO as verified.

| | |
|---|---|
| Topics | `("approved", ngo_owner: Address)` |
| Data | `()` (no payload) |

### `revoked`

Emitted by `revoke_ngo` when an admin reverses a prior approval, marking a
verified NGO as unverified again. This is the direct counterpart to
`approved` for the same `ngo_owner` — an indexer should treat a `revoked`
event as cancelling the most recent `approved` event for that address.

| | |
|---|---|
| Topics | `("revoked", ngo_owner: Address)` |
| Data | `()` (no payload) |

### `removed`

Emitted by `remove_ngo` when an admin deletes an unverified application
outright, without waiting for its owner to `unregister`. Like `unregist` this
is the owner-initiated counterpart, but admin-initiated and carrying the same
payload. A verified NGO cannot be removed directly: it must be revoked first,
so a `removed` event never follows an un-revoked `approved` for the same
`owner`.

| | |
|---|---|
| Topics | `("removed", owner: Address)` |
| Data | `()` (no payload) |

### `init`

Emitted by the registry constructor and by `init` when the admin is first set,
so an indexer can learn the contract's admin from deployment events alone.

| | |
|---|---|
| Topics | `("init",)` |
| Data | `admin: Address` |

### `upgrade`

Emitted by `upgrade` when an admin replaces the registry's Wasm bytecode.

| | |
|---|---|
| Topics | `("upgrade",)` |
| Data | `new_wasm_hash: BytesN<32>` |

## `donation-vault`

### `init`

Emitted by the vault constructor and by `init` when the admin is first set, so
an indexer can learn the contract's admin from deployment events alone.

| | |
|---|---|
| Topics | `("init",)` |
| Data | `admin: Address` |

### `propadmin`

Emitted by `propose_admin` when the current admin nominates a new admin.
The transfer is not complete until the nominated address calls
`accept_admin` and an `acptadmin` event is emitted.

| | |
|---|---|
| Topics | `("propadmin",)` |
| Data | `new_admin: Address` |

### `acptadmin`

Emitted by `accept_admin` when the nominated admin accepts the transfer.
After this event the address in `data` is the active admin; the previous
admin has no further authority.

| | |
|---|---|
| Topics | `("acptadmin",)` |
| Data | `new_admin: Address` |

### `pause`

Emitted by `pause` when an admin halts stream creation, withdrawal, top-up,
and rate changes.

| | |
|---|---|
| Topics | `("pause",)` |
| Data | `()` (no payload) |

### `unpause`

Emitted by `unpause` when an admin lifts a pause.

| | |
|---|---|
| Topics | `("unpause",)` |
| Data | `()` (no payload) |

### `feeset`

Emitted by `set_fee_bps` when an admin changes the protocol fee.

| | |
|---|---|
| Topics | `("feeset",)` |
| Data | `fee_bps: u32` (the new fee, in basis points) |

The fee is capped at `MAX_FEE_BPS` (1,000 / 10%); calls above the cap fail
with `FeeTooHigh` and emit nothing. The event carries the full new value
(not a delta), so an indexer can track the fee without polling `fee_bps`.
"Accepted" here means stored, not effective: the fee only affects payouts
once a treasury is set, so pair this with `set_treasury`/`treasury()` when
deriving an actual split.

### `tokfeeset`

Emitted by `set_token_fee_bps` when an admin configures a per-token fee override.

| | |
|---|---|
| Topics | `("tokfeeset", token: Address)` |
| Data | `fee_bps: u32` (the token fee override, in basis points) |

The override takes precedence over the global `fee_bps` for streams using this token.
An explicit override of `0` means zero fees for this token even when a global fee is set.

### `tokfeeclr`

Emitted by `clear_token_fee_bps` when an admin removes a per-token fee override.

| | |
|---|---|
| Topics | `("tokfeeclr", token: Address)` |
| Data | `()` (no payload) |

After clearing, streams using this token fall back to the global `fee_bps` setting.
If no override existed for the token, the call is a clean no-op and no event is emitted.

### `treasset`

Emitted by `set_treasury` when an admin changes where protocol fees are paid.

| | |
|---|---|
| Topics | `("treasset",)` |
| Data | `treasury: Address` (the new treasury address) |

The event carries the new treasury address so an off-chain indexer can track
where protocol fees will flow without polling `treasury()`. Protocol fees only
affect payouts once a treasury is configured, so pair this with `feeset`/`fee_bps()`
when deriving an actual split.

### `treclr`

Emitted by `clear_treasury` when an admin removes the configured treasury.

| | |
|---|---|
| Topics | `("treclr",)` |
| Data | `()` (no payload) |

Clearing the treasury is rejected while a non-zero `fee_bps` is set, so a
`treclr` event never follows a `feeset` with a non-zero value without an
intervening `feeset` of `0`. After a `treclr` event, protocol fees stop
accruing until a new `treasset` event appears.

### `mindep`

Emitted by `set_min_deposit` when an admin changes the minimum stream deposit.

| | |
|---|---|
| Topics | `("mindep",)` |
| Data | `min_deposit: i128` (the new minimum) |

The event carries the full new value so an indexer can track the minimum
without polling `min_deposit()`. A negative value fails with `InvalidAmount`
and emits nothing.

### `grceset`

Emitted by `set_cancel_grace_ledgers` when an admin changes the extra ledger
retention window applied to cancelled stream records.

| | |
|---|---|
| Topics | `("grceset",)` |
| Data | `grace_ledgers: u32` (the new window, in ledgers) |

### `regset`

Emitted by `set_registry` when an admin points the vault at an NGO registry
contract.

| | |
|---|---|
| Topics | `("regset",)` |
| Data | `registry: Address` (the new registry address) |

Once set, `create_stream` verifies the NGO against this registry. To clear the
registry pointer back to `None`, the admin calls `clear_registry` (emitting `regclr`).

### `regclr`

Emitted by `clear_registry` when an admin clears the configured NGO registry contract.

| | |
|---|---|
| Topics | `("regclr",)` |
| Data | `()` |

Clearing the registry removes the stored registry address (`registry()` returns `None`),
so subsequent `create_stream` calls accept any recipient address without registry
verification.


### `upgrade`

Emitted by `upgrade` when an admin replaces the vault's Wasm bytecode.

| | |
|---|---|
| Topics | `("upgrade",)` |
| Data | `new_wasm_hash: BytesN<32>` |

### `created`

Emitted by `create_stream` when a donor opens a new stream.

| | |
|---|---|
| Topics | `("created", stream_id: u64)` |
| Data | `(donor: Address, ngo: Address, token: Address, deposit: i128, rate: i128)` |

`deposit` is the amount pulled from the donor into the vault; `rate` is how
much of it accrues to the NGO per second (see [`math::accrued`](../contracts/donation-vault/src/math.rs)).

### `withdraw`

Emitted by `withdraw` when an NGO claims everything accrued on a stream
since the last checkpoint.

| | |
|---|---|
| Topics | `("withdraw", stream_id: u64)` |
| Data | `(accrued: i128, fee: i128, net: i128)` |

`accrued` is the gross amount released from the stream's balance. `fee` is the
protocol fee paid to the treasury (or `0` if no treasury is set or the effective
fee is 0%). `net` is the net amount actually transferred to the NGO. The
invariant `accrued = fee + net` holds for every payout.

The `withdraw` entry point returns that same `net` amount, so callers displaying
the payout receive the exact token units transferred to the NGO.

### `cancel`

Emitted by `cancel_stream` when a donor stops a stream for good.

| | |
|---|---|
| Topics | `("cancel", stream_id: u64)` |
| Data | `(accrued: i128, refund: i128)` |

`accrued` is whatever had accrued to the NGO and was paid out (subject to
the same protocol-fee split as `withdraw`) as part of settling the stream;
`refund` is the untouched remainder returned to the donor. The stream
record is kept with `balance` and `rate` zeroed, not deleted.

### `topup`

Emitted by `top_up` when a donor adds more funds to an existing stream.

| | |
|---|---|
| Topics | `("topup", stream_id: u64)` |
| Data | `(amount: i128, settled: i128)` |

`amount` is the newly added deposit. Any balance already accrued at the time of the
top-up is settled to the NGO first (as its own implicit payout, without emitting a
separate `withdraw` event) before the deposit is added; `settled` reports that gross
settled amount.

### `ratemod`

Emitted by `modify_rate` when a donor changes a stream's per-second accrual
rate.

| | |
|---|---|
| Topics | `("ratemod", stream_id: u64)` |
| Data | `(old_rate: i128, new_rate: i128, settled: i128)` |

As with `top_up`, any balance already accrued at the old rate is settled to
the NGO first, so the new rate only ever applies going forward. `settled` reports
the gross amount settled to the NGO at the old rate. The event records both rates
so an indexer can calculate the change without another state query.

### `transfer`

Emitted by `transfer_stream` when a donor reassigns an ongoing stream to a
different NGO.

| | |
|---|---|
| Topics | `("transfer", stream_id: u64)` |
| Data | `(old_ngo: Address, new_ngo: Address, settled: i128)` |

Anything accrued to the old NGO before the transfer is settled to it as
part of the call (subject to the same protocol-fee split as `withdraw`),
without emitting a separate `withdraw` event; `settled` reports that gross
amount settled to `old_ngo`. After this event, future accrual goes to the
`new_ngo` in `data`. The donor and the stream's balance and rate are unchanged.

### `unregist`

Emitted by `ngo-registry` when an unverified NGO removes its own application.

| | |
|---|---|
| Topics | `("unregist", owner: Address)` |
| Data | `()` (no payload) |
