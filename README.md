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

This is a single-file Anchor program intended for Solana Playground.

Included:

* `lib.rs`
* Playground-compatible `client.ts`
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

This is only a simplified placeholder. A real system would need defined day-count conventions, compounding rules, payment waterfalls, fees, and reporting logic.

### `make_repayment`

Allows the borrower to make a repayment.

Tracks:

* Total repayment amount
* Principal portion
* Interest portion
* Whether scheduled payment was satisfied

Updates:

* Total repaid
* Outstanding principal
* Accrued interest
* Available liquidity
* Payment status
* Facility status
* Repayment schedule

If principal and accrued interest are fully repaid, the facility can be marked as `Matured`.

### `record_missed_payment`

Allows the operator to mark a scheduled payment as missed.

Updates:

* Missed payment count
* Payment status
* Repayment schedule

If missed payments exceed the threshold, the facility moves to `Defaulted`.

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

If a covenant is breached, the facility moves to `Breached`.

### `toggle_pik`

Allows the operator to enable or disable PIK mode.

PIK means **payment-in-kind**, where interest can accrue instead of being paid in cash.

Tracks:

* Whether PIK is enabled
* Total PIK accrued
* Last PIK timestamp

PIK cannot be toggled after default or close.

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

Redemption does not instantly withdraw funds. It enters a queue and must be processed by the operator.

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
* Post-redemption liquidity does not fall below the minimum liquidity reserve

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

This models compliance-style controls, transfer restrictions, investor eligibility issues, legal freezes, or risk controls.

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

The facility can be closed if:

* Outstanding principal is zero, or
* The facility is marked fully resolved

If not fully resolved, pending redemptions must be zero.

### `append_event_log`

Allows the operator to append a manual event log record.

This is useful for demo and audit trail testing.

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
```

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

A production version would replace simulated accounting with SPL Token vault transfers.

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

### 2. Replace `lib.rs`

Paste the program code into:

```text
programs/<your-program-name>/src/lib.rs
```

### 3. Build

Click:

```text
Build
```

### 4. Deploy

Click:

```text
Deploy
```

Wait until deployment finishes.

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

The Playground client demonstrates this sequence:

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
* No multi-investor waterfall allocation
* No production admin controls

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
* Add compliance allowlist
* Add redemption gates by epoch
* Add default recovery workflow
* Add liquidation workflow
* Add TypeScript tests
* Add dashboard indexer
* Add event indexer
* Add multi-facility operator dashboard

## Repository Structure

For Solana Playground, the project can stay minimal:

```text
.
├── programs/
│   └── tokenized-private-credit-vault/
│       └── src/
│           └── lib.rs
├── client.ts
└── README.md
```

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
