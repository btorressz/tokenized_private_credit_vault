# Tokenized Private Credit Vault + Covenant Monitor

A Solana Anchor proof-of-concept program for modeling private credit facility workflows, covenant monitoring, repayment tracking, PIK accounting, collateral health, and redemption queue mechanics on-chain.

This project is designed for **Solana Playground**, **devnet experimentation**, and **research/demo purposes only**.

## Important Disclaimer

This is **not financial advice**, **not a securities product**, **not a real lending protocol**, and **not production-ready**.

This project does not implement real credit underwriting, real investor onboarding, real yield generation, real token custody, real KYC/AML, or real securities compliance. It is a simplified proof-of-concept for exploring how private credit facility state, investor positions, borrower repayments, covenant health, collateral status, and redemption queue accounting could be represented using Solana program accounts.

A production system would require legal review, securities/lending compliance, audited smart contracts, KYC/AML controls, custody infrastructure, oracle integrations, off-chain reporting, investor agreements, governance controls, risk management, and extensive security reviews.

## Project Overview

Private credit facilities often involve many moving parts:

* Capital commitments from investors
* Borrower drawdowns
* Scheduled repayments
* Interest accrual
* Covenant monitoring
* Collateral valuation
* Missed payment tracking
* PIK interest behavior
* Redemption requests
* Liquidity gates
* Facility maturity and default handling

This program models those workflows as Solana PDA accounts.

The goal is to show how a private credit facility could be represented as an auditable on-chain state machine, while keeping the implementation simple enough for Solana Playground.

## Core Concept

The program models a private credit facility where:

1. An operator initializes a credit facility.
2. The operator activates the facility.
3. Investors deposit stablecoin-like capital into the facility.
4. A borrower draws down capital.
5. The program tracks outstanding principal, available liquidity, repayments, accrued interest, PIK, and facility status.
6. The operator updates covenant and collateral state.
7. Investors request redemptions.
8. Redemptions can be queued, cancelled, partially processed, or processed.
9. The operator can freeze investor positions.
10. The facility can mature, default, or close.

For simplicity, this proof of concept uses **simulated stablecoin accounting**. No real SPL token transfers are performed.

## Tech Stack

* Solana
* Anchor
* Rust
* TypeScript client for Solana Playground
* PDA-based account architecture
* Simulated stablecoin accounting

## Program Scope

This is a modular Anchor/Rust MVP intended for Solana Playground. Following [PR #1](https://github.com/btorressz/tokenized_private_credit_vault/pull/1), the program has 20 Rust source files (including `lib.rs`), 19 declared source modules, and retains the original 20 public instruction signatures. The program entry point is approximately 482 lines, below the project's 2,300-line editor constraint.

The repository now separates the original instruction execution paths from reusable financial research engines. Source availability and module registration do not imply that every helper is exposed as an on-chain instruction or that the current revision has passed compilation.

Included:

* Modular Rust source tree and Anchor program entry point (`lib.rs`)
* Playground-compatible `client/client.ts` and TypeScript lifecycle test (`tests/anchor.test.ts`)
* Deterministic risk, NAV, waterfall, recovery, permission and simulated settlement helpers
* PDA state accounts
* Lifecycle state machine
* Event log accounts
* Simulated vault accounting
* Redemption queue logic
* Covenant and collateral monitoring

Not included:

* Frontend
* Backend
* Database
* Real SPL token transfers
* Real stablecoin vault
* Oracle integration
* KYC/AML system
* Securities compliance logic
* Production deployment setup

## Modular Architecture and Implementation Status

| File | Responsibility | Current scope |
| --- | --- | --- |
| `lib.rs` | Anchor entry point, instruction dispatch, deposit and drawdown orchestration | Original 20 instructions retained |
| `account.rs` | Anchor account contexts, signers, PDA constraints | Original contexts; extension contexts prepared |
| `state.rs` | Facility, investor, repayment, covenant, redemption and event state | Original layouts plus extension account types |
| `constants.rs` | Protocol bounds, time periods and fixed-point constants | Shared foundation |
| `errors.rs` | Custom Anchor errors | Shared foundation |
| `math.rs` | Checked arithmetic, interest, ratios, valuation and allocation helpers | Shared foundation |
| `validation.rs` | Authorization and lifecycle guards | Shared foundation |
| `pda.rs` | Manual PDA creation, ownership checks, serialization | Core instruction support |
| `events.rs` | Sequential event snapshots and full-width redemption queue evidence | Core instruction support |
| `facility.rs` | Initialization, activation, maturity, default, metadata and closure | Delegated instruction logic |
| `accounting.rs` | Interest, repayment, missed payments and PIK | Delegated instruction logic |
| `covenant.rs` | Operator-reported covenant monitoring | Delegated instruction logic |
| `collateral.rs` | Reported collateral value, coverage and impairment | Delegated instruction logic |
| `redemption.rs` | Redemption requests, cancellation, processing and freezes | Delegated instruction logic; strict FIFO deferred |
| `risk.rs` | Explainable heuristic credit risk, freshness and haircut calculations | Research helpers; not an enforced risk engine |
| `valuation.rs` | GAV, NAV, share-price and conversion calculations | Research helpers; NAV execution disabled |
| `waterfall.rs` | Fee-bounded repayment allocation and reconciliation | Research helpers; not wired into repayment execution |
| `recovery.rs` | Recovery metrics, write-down arithmetic, allocation and transition guards | Research helpers; recovery administration not exposed |
| `compliance.rs` | Eligibility checks and position status restoration | Research helpers; permissioned mode and unfreeze instructions not exposed |
| `token_vault.rs` | Pure simulated settlement arithmetic and denomination validation | Used for core cash updates; real SPL mode rejected |

### Current financial execution boundaries

The original public instructions use fixed 1:1 simulated share accounting. The new modules are registered in the source tree, but advanced NAV execution, configurable payment waterfalls, recovery administration, permissioned facility activation, strict FIFO enforcement and extension management are not yet enabled as complete on-chain workflows. The permission module requires canonical authenticated records before it can safely be integrated; missing evidence must not bypass permission checks. Risk scores are deterministic heuristics, not calibrated probabilities of default. Collateral and covenant inputs remain operator-reported.

The modular integration adds source-level safeguards for accrual before principal changes, excess-interest rejection, missed-payment timing, liquidity reserves, prefunded PDAs, restricted administrative events and full-width queue evidence. These changes are not a substitute for executed regression tests or a security audit. See [MVP integration notes](MVP_INTEGRATION_NOTES.md) for remaining boundaries.

## PDA Accounts

The program uses deterministic PDA accounts for facility state, investor state, repayment tracking, covenant monitoring, redemption requests, and event logs.

| Account              | Seeds                                            | Purpose                                  |
| -------------------- | ------------------------------------------------ | ---------------------------------------- |
| `facility`           | `["facility", facility_id]`                      | Main facility state account              |
| `investor_position`  | `["investor", facility.key(), investor.key()]`   | Tracks each investor’s position          |
| `repayment_schedule` | `["repayment", facility.key()]`                  | Tracks repayment state                   |
| `covenant_state`     | `["covenant", facility.key()]`                   | Tracks covenant and collateral health    |
| `redemption_request` | `["redemption", facility.key(), investor.key()]` | Tracks investor redemption queue request |
| `event_log`          | `["event", facility.key(), event_index]`         | Stores auditable event snapshots         |

The six original account layouts remain the compatibility baseline. Two additional account types are defined for future instruction integration:

| Extension account | Seeds | Purpose |
| --- | --- | --- |
| `VaultExtension` | `["extension", facility.key()]` | Proposed permission mode, fees, FIFO pointer, grace period, valuation and recovery state |
| `InvestorEligibility` | `["eligibility", facility.key(), investor.key()]` | Proposed eligibility flag, expiry and administrative evidence |

These extension types and preparation contexts do not establish an active permissioned or NAV-based facility mode. The current program does not expose their initialization or administration as public instructions.

## Main Accounts

### Facility

The `Facility` account stores the primary state of the credit facility.

It tracks:

* Facility ID
* Operator
* Borrower
* Stablecoin mint placeholder
* Facility size
* Interest rate
* Maturity timestamp
* Max drawdown amount
* Minimum collateral coverage
* Minimum liquidity reserve
* Collateral description hash
* Current lifecycle status
* Payment status
* Collateral status
* Total deposited
* Total drawn
* Total repaid
* Available liquidity
* Outstanding principal
* Accrued interest
* Missed payment count
* Pending redemptions
* Redemption queue index
* PIK status
* Default reason code
* Event index
* Timestamps

### InvestorPosition

The `InvestorPosition` account tracks a single investor’s exposure to a facility.

It tracks:

* Investor wallet
* Facility
* Principal
* Share balance
* Deposited amount
* Redeemed amount
* Pending redemption amount
* Investor position status
* Timestamps

### RepaymentSchedule

The `RepaymentSchedule` account tracks repayment state for the facility.

It tracks:

* Expected repayment amount
* Next due timestamp
* Scheduled payment amount
* Last payment timestamp
* Last repayment amount
* Principal portion
* Interest portion
* Number of payments made
* Number of scheduled payments satisfied
* Payment status

### CovenantState

The `CovenantState` account tracks covenant and collateral health.

It tracks:

* Debt service coverage ratio
* Loan-to-value ratio
* Minimum liquidity ratio
* Reporting status
* Last report timestamp
* Breach status
* Breach reason
* Collateral value
* Collateral status
* Last valuation timestamp
* Collateral impairment flag
* Last updater

### RedemptionRequest

The `RedemptionRequest` account tracks an investor’s redemption request.

It tracks:

* Facility
* Investor
* Requested amount
* Processed amount
* Request timestamp
* Processed timestamp
* Redemption status
* Queue index

### EventLog

The `EventLog` account stores an immutable-style event snapshot for important actions.

It tracks:

* Facility
* Event index
* Event type
* Actor
* Timestamp
* Amount
* Facility status snapshot
* Payment status snapshot
* Note code

## Facility Lifecycle

The facility can move through these states:

```text
Initialized
Active
Drawn
Performing
Breached
Defaulted
Matured
Closed
```

### Example Flow

```text
initialize_facility
        ↓
activate_facility
        ↓
deposit_capital
        ↓
borrower_drawdown
        ↓
accrue_interest
        ↓
toggle_pik / capitalize_pik
        ↓
make_repayment
        ↓
request_redemption
        ↓
cancel_redemption or process_redemption
        ↓
mark_matured
        ↓
close_facility
```

## Program Instructions

### `initialize_facility`

Creates a new private credit facility.

Initializes:

* Facility terms
* Operator
* Borrower
* Stablecoin mint placeholder
* Facility size
* Interest rate
* Maturity timestamp
* Max drawdown
* Collateral hash
* Minimum collateral coverage
* Minimum liquidity reserve
* Repayment schedule
* Covenant state
* First event log

### `activate_facility`

Explicitly moves the facility from `Initialized` to `Active`.

Checks:

* Operator authorization
* Facility is currently initialized
* Maturity timestamp is still in the future

### `deposit_capital`

Allows an investor to deposit simulated stablecoin-like capital.

Updates:

* Investor principal
* Investor share balance
* Investor deposited amount
* Facility total deposited
* Facility available liquidity

Production note: this is where a real implementation would use SPL Token CPI transfers into a vault token account.

### `borrower_drawdown`

Allows the borrower or operator to draw down capital.

Checks:

* Actor is borrower or operator
* Facility allows drawdowns
* Drawdown is before maturity
* Amount does not exceed available liquidity
* Total drawn does not exceed max drawdown
* Total drawn does not exceed facility size
* Post-drawdown simulated cash respects the minimum liquidity reserve
* Interest is accrued against the old principal before increasing principal

Updates:

* Total drawn
* Outstanding principal
* Available liquidity
* Facility status
* Payment status
* Repayment schedule

Production note: this is where a real implementation would transfer stablecoins to the borrower.

### `accrue_interest`

Accrues simple simulated interest based on:

```text
outstanding_principal * interest_rate_bps * elapsed_seconds / year_seconds / 10_000
```

Updates:

* Accrued interest
* Last interest accrual timestamp

This uses checked integer arithmetic and a simplified 365-day denominator. Accrual is also applied before principal-changing drawdowns, non-defaulted repayments and PIK capitalization. Fractional interest carry and comprehensive day-count and compounding rules remain follow-up work.

### `make_repayment`

Allows the borrower to make a repayment.

Tracks:

* Total repayment amount
* Principal portion
* Interest portion
* Caller-reported scheduled payment satisfaction (legacy MVP behavior; deterministic contractual schedule validation remains pending)
* Principal and interest portions must reconcile with the accepted amount and not exceed their recorded obligations

Updates:

* Total repaid
* Outstanding principal
* Accrued interest
* Available liquidity
* Payment status
* Facility status
* Repayment schedule

If principal, accrued interest and uncapitalized PIK are fully repaid, a non-defaulted facility can be marked as `Matured`. Ordinary repayment does not clear default status. The configurable waterfall helper is not yet wired into this instruction.

### `record_missed_payment`

Allows the operator to mark a scheduled payment as missed.

Updates:

* Missed payment count
* Payment status
* Repayment schedule

Missed-payment recording requires outstanding principal and a due date past the configured default grace period. The next due date advances by one payment period. At the configured threshold of two missed payments, the facility moves to `Defaulted`. A complete contractual schedule and payment-period reconciliation are still required.

### `update_covenant_state`

Allows the operator to update covenant metrics.

Tracks:

* Debt service coverage ratio
* Loan-to-value ratio
* Minimum liquidity ratio
* Reporting status
* Last report timestamp
* Breach status
* Breach reason

A reported breach can move an eligible facility to `Breached`; the update cannot reactivate a matured or defaulted facility. Collateral impairment is retained as a breach condition. Full contractual threshold evaluation and independent reporting verification remain future work.

### `toggle_pik`

Allows the operator to enable or disable PIK mode.

PIK means **payment-in-kind**, where interest can accrue instead of being paid in cash.

Tracks:

* Whether PIK is enabled
* Total PIK accrued
* Last PIK timestamp

PIK cannot be toggled after default or close. The current PIK accrual amount is still operator-supplied; deterministic contractual PIK accrual and reconciliation are not yet implemented.

### `capitalize_pik`

Moves accrued PIK into outstanding principal.

Example:

```text
outstanding_principal += total_pik_accrued
total_pik_accrued = 0
```

This models a major private credit risk behavior where unpaid interest increases the borrower’s principal balance.

### `update_collateral_status`

Allows the operator to update collateral health.

Tracks:

* Collateral value
* Collateral status
* Last valuation timestamp
* Collateral impairment flag

The program also calculates collateral coverage:

```text
collateral_coverage_bps = collateral_value * 10_000 / outstanding_principal
```

If collateral is impaired or coverage falls below the minimum threshold, the facility can move to `Breached`.

### `request_redemption`

Allows an investor to request redemption.

Updates:

* Redemption request amount
* Redemption request status
* Queue index
* Investor pending redemption amount
* Facility total pending redemptions

Redemption does not instantly withdraw funds. It records a queue index and must be processed by the operator. Strict FIFO priority enforcement and protection against new amounts inheriting an older queue position remain future integration work.

### `cancel_redemption`

Allows an investor to cancel a pending redemption request.

Supported statuses:

* `Requested`
* `Queued`
* `PartiallyProcessed`

Updates:

* Remaining pending redemption amount
* Investor pending redemption amount
* Facility total pending redemptions
* Redemption status

This makes the redemption queue more realistic.

### `process_redemption`

Allows the operator to process an investor redemption.

Checks:

* Operator authorization
* Redemption is not already processed
* Redemption is not frozen
* Investor position is not frozen
* Amount does not exceed pending request
* Amount does not exceed investor balance
* Amount does not exceed available liquidity
* Post-redemption liquidity does not fall below the minimum liquidity reserve, except for the implemented debt-free matured-facility settlement case

Updates:

* Investor share balance
* Investor principal
* Investor redeemed amount
* Investor pending redemption amount
* Redemption processed amount
* Redemption status
* Facility available liquidity
* Facility pending redemptions

Production note: this is where a real implementation would transfer stablecoins back to the investor.

### `freeze_investor_position`

Allows the operator to freeze an investor position.

Updates:

* Investor position status to `Frozen`
* Pending redemption request status to `Frozen`, if one exists

This models a simulated position restriction, not legal compliance. A status restoration helper now exists in `compliance.rs`, but an authorized public unfreeze instruction is not yet exposed.

### `mark_matured`

Allows the operator to mark a facility as matured.

Allowed when:

* Maturity timestamp has passed, or
* Outstanding principal is zero

Updates:

* Facility status
* Payment status
* Repayment schedule

### `raise_facility_default`

Allows the operator to mark the facility as defaulted.

Updates:

* Facility status to `Defaulted`
* Payment status to `Defaulted`
* Default reason code
* Repayment schedule

Once defaulted, normal deposits, drawdowns, and redemptions are blocked by status checks.

### `update_facility_metadata`

Allows the operator to update safe metadata and limited terms.

Allowed updates:

* Collateral description hash
* Maturity timestamp before drawdown
* Max drawdown amount before drawdown
* Borrower before activation
* Minimum collateral coverage
* Minimum liquidity reserve

This demonstrates governance and lifecycle-aware term update logic.

### `close_facility`

Allows the operator to close the facility.

Closure now requires zero outstanding principal, accrued interest, uncapitalized PIK and pending redemptions. The legacy `fully_resolved` argument remains in the public signature for compatibility but cannot waive these checks. Aggregate unredeemed share supply is not yet tracked; complete investor settlement invariants remain a required follow-up.

### `append_event_log`

Allows the operator to append an `AdministrativeNote` event only. Arbitrary manual financial event types are rejected so an administrative annotation cannot impersonate a deposit, repayment or other financial transaction.

## Enums

### FacilityStatus

```text
Initialized
Active
Drawn
Performing
Breached
Defaulted
Matured
Closed
```

### PaymentStatus

```text
Current
Due
Late
Missed
Defaulted
Repaid
```

### InvestorPositionStatus

```text
Active
RedemptionPending
PartiallyRedeemed
Redeemed
Frozen
```

### RedemptionStatus

```text
Requested
Queued
PartiallyProcessed
Processed
Cancelled
Frozen
```

### CollateralStatus

```text
Unknown
Healthy
Watchlist
Impaired
LiquidationPending
Liquidated
```

### EventType

```text
FacilityInitialized
FacilityActivated
CapitalDeposited
BorrowerDrawdown
InterestAccrued
RepaymentMade
PaymentMissed
CovenantUpdated
CovenantBreached
PikToggled
PikCapitalized
CollateralUpdated
RedemptionRequested
RedemptionCancelled
RedemptionProcessed
InvestorFrozen
FacilityMatured
FacilityDefaulted
TermsUpdated
FacilityClosed
InvestorUnfrozen
RiskAssessed
ValuationUpdated
RecoveryProceedsRecorded
DebtWrittenDown
FacilityRestructured
RecoveryCompleted
EligibilityUpdated
WaterfallApplied
AdministrativeNote
```

The additional event variants reserve audit vocabulary for future integrated workflows; their presence does not mean those workflows are executable. `RedemptionQueueEvidence` is a supplementary Anchor event carrying the full `u64` queue index, alongside the legacy `u16` note field.

## Simulated Accounting Model

This program does not move real tokens.

Instead, it updates accounting fields such as:

* `total_deposited`
* `available_liquidity`
* `total_drawn`
* `outstanding_principal`
* `accrued_interest`
* `total_pik_accrued`
* `total_repaid`
* `total_pending_redemptions`
* `redeemed_amount`

This keeps the project simple and suitable for Solana Playground.

The `token_vault.rs` module computes checked cash deltas without maintaining a second ledger. Existing instruction paths remain responsible for the authoritative balance update. `SimulatedAccounting` is the only supported settlement mode; `FutureSplTokenSettlement` is explicitly rejected by the settlement preview helper. Mint keys identify simulated denominations, not verified token accounts. A production version would require a separately implemented, reviewed SPL Token custody and settlement architecture.

## Event Logging

Every important action creates an `EventLog` PDA.

Event logs use this seed pattern:

```text
["event", facility.key(), event_index]
```

The facility stores the current `event_index`, and each action must pass the correct next index.

Each event stores:

* Event type
* Actor
* Timestamp
* Amount
* Facility status snapshot
* Payment status snapshot
* Note code

This creates a simple on-chain audit trail.

## Solana Playground Usage

### 1. Open Solana Playground

Go to Solana Playground and create a new Anchor project.

### 2. Import the complete modular source tree

Import or synchronize the repository's [`tokenized_private_credit_vault`](tokenized_private_credit_vault/) project directory into an Anchor workspace. If manually copying files, copy **all 20 Rust files** from `tokenized_private_credit_vault/src/` into the workspace's program source directory, preserving their names. Copying `lib.rs` alone will leave unresolved modules.

Back up local Playground changes before refreshing or reimporting from GitHub. The exported source contains a public placeholder in `declare_id!`; replace it with the actual program public key assigned by your Playground workspace. The placeholder is not a deployed program or usable keypair.

### 3. Build

Click:

```text
Build
```

### 4. Verify before any devnet experiment

The current modular revision has not been verified with an executed Anchor build or regression suite. Resolve compilation errors, inspect the generated IDL, and adapt and run the client and integration tests against the stricter accounting checks first. Only after successful verification should you consider a devnet-only deployment using test data. Do not deploy to mainnet or use real assets.

### 5. Run the Client

Paste the provided `client.ts` into the Playground client tab.

The client supports two modes:

### Live Mode

If the program is deployed, it sends transactions.

### Dry-Run Mode

If the program is not deployed, it still runs and prints:

* Wallet address
* SOL balance
* Program ID
* Facility PDA
* Repayment schedule PDA
* Covenant state PDA
* Investor position PDA
* Redemption request PDA
* Planned instruction flow

This prevents the common error:

```text
Attempt to load a program that does not exist
```

## Example Demo Flow

The existing Playground client describes this demonstration sequence. It predates the stricter modular accounting and may need fixture changes before every step succeeds:

```text
initializeFacility
activateFacility
updateFacilityMetadata
depositCapital
borrowerDrawdown
accrueInterest
updateCovenantState
togglePik
capitalizePik
updateCollateralStatus
makeRepayment
requestRedemption
cancelRedemption
requestRedemption again
processRedemption
freezeInvestorPosition
markMatured
closeFacility
```

## Why This Project Exists

Private credit markets are large, complex, and often opaque. A research-oriented on-chain model can help demonstrate how credit facility workflows could become more transparent and auditable.

This proof of concept explores how Solana can be used to track:

* Facility lifecycle state
* Investor capital positions
* Borrower drawdowns
* Repayments
* Interest accrual
* PIK behavior
* Covenant health
* Collateral impairment
* Redemption queues
* Liquidity gates
* Defaults
* Maturity
* Facility closure
* Event history

The project is useful as a portfolio demo, research prototype, or starting point for learning how structured credit workflows can be modeled with Solana PDAs.

## Production Considerations

A real-world private credit system would require many additional components.

### Legal and Compliance

* Securities law review
* Lending law review
* Investor accreditation checks
* KYC/AML
* Sanctions screening
* Jurisdiction restrictions
* Offering documents
* Transfer restrictions

### Technical Infrastructure

* SPL Token vaults
* Token account ownership controls
* Custody architecture
* Oracle integrations
* Off-chain reporting
* Database indexing
* Monitoring
* Admin dashboards
* Incident response
* Governance controls

### Risk Management

* Borrower underwriting
* Collateral verification
* Independent valuations
* Payment waterfall rules
* Default resolution
* Recovery workflows
* Liquidity stress tests
* Redemption gates
* Concentration limits

### Security

* Full smart contract audit
* Economic attack review
* Oracle manipulation review
* Access control review
* PDA validation review
* Fuzz testing
* Integration testing
* Upgrade authority controls

## Current Limitations

This program intentionally simplifies many things:

* No real SPL token transfers
* No real stablecoin vault
* No investor accreditation
* No KYC/AML
* No oracle feeds
* No real collateral verification
* No secondary transfer logic
* No tokenized share mint
* No legal enforcement
* No real repayment schedule calendar
* No compounding interest model
* No integrated multi-investor waterfall or dynamic NAV-based execution
* No active permissioned facility or public unfreeze/recovery administration
* No strict FIFO enforcement or aggregate unredeemed share supply invariant
* No independently verified covenant or collateral data
* No deterministic contractual PIK calculation or complete repayment-period reconciliation
* No production admin controls
* Modular build and regression testing remain unverified

These limitations are intentional because the goal is a clean proof of concept.

## Possible Future Extensions

Potential next steps:

* Add SPL Token vault transfers
* Add tokenized investor share mint
* Add per-investor pro-rata repayment accounting
* Add repayment schedule installments
* Add oracle-based collateral valuations
* Add Chainlink or Pyth-style price feed integration
* Add off-chain report hash submissions
* Add document hash verification
* Add role-based permissions
* Integrate the existing eligibility and permission helpers with authenticated canonical accounts
* Add redemption gates by epoch
* Integrate the existing default recovery helpers into authorized on-chain workflows
* Add liquidation workflow
* Update the existing TypeScript lifecycle test and add regression, negative-path and invariant tests
* Add dashboard indexer
* Add event indexer
* Add multi-facility operator dashboard

## Repository Structure

The current GitHub export is organized as follows:

```text
.
├── LICENSE
├── README.md
├── MVP_INTEGRATION_NOTES.md
└── tokenized_private_credit_vault/
    ├── client/
    │   └── client.ts
    ├── src/
    │   ├── account.rs
    │   ├── accounting.rs
    │   ├── collateral.rs
    │   ├── compliance.rs
    │   ├── constants.rs
    │   ├── covenant.rs
    │   ├── errors.rs
    │   ├── events.rs
    │   ├── facility.rs
    │   ├── lib.rs
    │   ├── math.rs
    │   ├── pda.rs
    │   ├── recovery.rs
    │   ├── redemption.rs
    │   ├── risk.rs
    │   ├── state.rs
    │   ├── token_vault.rs
    │   ├── validation.rs
    │   ├── valuation.rs
    │   └── waterfall.rs
    └── tests/
        └── anchor.test.ts
```

## Verification Status and Next Milestones

| Milestone | Status |
| --- | --- |
| 20 Rust source files and module declarations | Present on main |
| Original 20 public instruction signatures | Retained in source |
| Simulated-only settlement | Implemented in source |
| Rust unit tests | Defined, execution unverified |
| TypeScript client and lifecycle tests | Present; updated behavior requires regression testing |
| Anchor build and generated IDL verification | Pending |
| Advanced financial and permission execution paths | Partial; integration pending |
| Security and economic audit | Not performed |

The GitHub export does not currently include a Cargo.toml or Anchor.toml. Use an appropriately configured Playground Anchor workspace for compilation, or establish a reproducible local build configuration before claiming a passing build. The next milestone is a verified build, instruction/account compatibility review, and full financial regression testing; then integrate extension-based features incrementally. See [MVP_INTEGRATION_NOTES.md](MVP_INTEGRATION_NOTES.md).

## Summary

Tokenized Private Credit Vault + Covenant Monitor is a Solana Anchor proof-of-concept for modeling private credit facility operations on-chain.

It demonstrates how Solana PDAs can be used to represent:

* Credit facility state
* Investor positions
* Borrower drawdowns
* Repayment tracking
* Interest accrual
* PIK capitalization
* Covenant monitoring
* Collateral health
* Redemption queue accounting
* Liquidity reserve gates
* Investor freezes
* Defaults
* Maturity
* Closure
* Event logs

This is a research and demo project only, not a real-world financial product.
