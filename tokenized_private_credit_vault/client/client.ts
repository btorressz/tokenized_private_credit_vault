// Client for Solana Playground
// Tokenized Private Credit Vault + Covenant Monitor
//
// Simulated-accounting demo for the modular Anchor MVP.
// All 20 Rust files must be built together; advanced research helpers are not public instructions.
//
// It has two modes:
//
// 1. Live mode:
//    If the program is deployed and executable, it sends transactions.
//
// 2. Dry-run mode:
//    If the program is not deployed, it still runs, derives all PDAs,
//    prints the full planned instruction flow, and exits cleanly.
//
// This prevents:
// "Attempt to load a program that does not exist"

async function main() {
  console.log("My address:", pg.wallet.publicKey.toString());

  const balance = await pg.connection.getBalance(pg.wallet.publicKey);
  console.log(`My balance: ${balance / web3.LAMPORTS_PER_SOL} SOL`);

  const program = pg.program;
  const programId = program.programId;
  const wallet = pg.wallet.publicKey;
  const BN = anchor.BN;

  console.log("Program ID:", programId.toString());

  function u64Le(value) {
    const bn = BN.isBN(value) ? value : new BN(value);
    return bn.toArrayLike(Buffer, "le", 8);
  }

  function findPda(seeds) {
    const [pda] = web3.PublicKey.findProgramAddressSync(seeds, programId);
    return pda;
  }

  function methodExists(methodName) {
    return typeof program.methods[methodName] === "function";
  }

  async function getRunMode() {
    const programAccount = await pg.connection.getAccountInfo(programId);

    if (!programAccount) {
      console.log("");
      console.log("Program account not found on this cluster.");
      console.log("Running in DRY-RUN mode.");
      console.log("The client will derive PDAs and print the planned instruction flow.");
      console.log("");
      console.log("To run live transactions:");
      console.log("1. Build the program in Solana Playground.");
      console.log("2. Deploy the program.");
      console.log("3. Run this client again.");
      console.log("");
      return "dry-run";
    }

    if (!programAccount.executable) {
      console.log("");
      console.log("Program account exists, but it is not executable.");
      console.log("Running in DRY-RUN mode.");
      console.log("This usually means the deploy did not complete correctly.");
      console.log("");
      return "dry-run";
    }

    console.log("Program deployed and executable: true");
    return "live";
  }

  async function sendOrDry(label, methodName, eventIndex, buildTx) {
    const eventPda = eventLogPda(eventIndex);

    if (runMode === "dry-run") {
      console.log("");
      console.log(`[dry-run] ${label}`);
      console.log(`  method: ${methodName}`);
      console.log(`  eventIndex: ${eventIndex}`);
      console.log(`  eventLog PDA: ${eventPda.toString()}`);
      return true;
    }

    if (!methodExists(methodName)) {
      console.log("");
      console.log(`${label} cannot run because method "${methodName}" is missing from the IDL.`);
      console.log("This usually means the client and lib.rs are out of sync.");
      console.log("Build the latest lib.rs in Solana Playground, then run the client again.");
      return false;
    }

    try {
      const sig = await buildTx();
      console.log(`${label}: ${sig}`);
      return true;
    } catch (err) {
      console.log("");
      console.log(`${label} failed.`);

      if (err?.message) {
        console.log("Error message:", err.message);
      } else {
        console.log("Error:", err);
      }

      if (
        err?.message &&
        String(err.message).includes("Attempt to load a program that does not exist")
      ) {
        console.log("");
        console.log("The program is not deployed on this cluster.");
        console.log("Build + Deploy the program, then run the client again.");
        return false;
      }

      try {
        if (typeof err?.getLogs === "function") {
          const logs = await err.getLogs(pg.connection);
          console.log("Transaction logs:");
          console.log(logs);
        } else if (err?.logs) {
          console.log("Transaction logs:");
          console.log(err.logs);
        }
      } catch (logErr) {
        console.log("Could not fetch transaction logs:", logErr);
      }

      return false;
    }
  }

  // ---------------------------------------------------------------------------
  // Demo identities
  // ---------------------------------------------------------------------------
  // For Playground simplicity, this uses the same wallet as:
  // operator, borrower, and investor.
  //
  // In a real test, these could be separate Keypairs.
  const operator = wallet;
  const borrower = wallet;
  const investor = wallet;

  // Placeholder stablecoin mint.
  // This program uses simulated accounting only.
  const stablecoinMint = web3.Keypair.generate().publicKey;

  // Keep facility ID under 32 bytes because lib.rs uses #[max_len(32)].
  const facilityId = `pcv-${Date.now()}`;

  console.log("Facility ID:", facilityId);

  // ---------------------------------------------------------------------------
  // PDA derivation
  // ---------------------------------------------------------------------------
  const facilityPda = findPda([
    Buffer.from("facility"),
    Buffer.from(facilityId),
  ]);

  const repaymentSchedulePda = findPda([
    Buffer.from("repayment"),
    facilityPda.toBuffer(),
  ]);

  const covenantStatePda = findPda([
    Buffer.from("covenant"),
    facilityPda.toBuffer(),
  ]);

  const investorPositionPda = findPda([
    Buffer.from("investor"),
    facilityPda.toBuffer(),
    investor.toBuffer(),
  ]);

  const redemptionRequestPda = findPda([
    Buffer.from("redemption"),
    facilityPda.toBuffer(),
    investor.toBuffer(),
  ]);

  function eventLogPda(eventIndex) {
    return findPda([
      Buffer.from("event"),
      facilityPda.toBuffer(),
      u64Le(eventIndex),
    ]);
  }

  console.log("Facility PDA:", facilityPda.toString());
  console.log("Repayment Schedule PDA:", repaymentSchedulePda.toString());
  console.log("Covenant State PDA:", covenantStatePda.toString());
  console.log("Investor Position PDA:", investorPositionPda.toString());
  console.log("Redemption Request PDA:", redemptionRequestPda.toString());

  // ---------------------------------------------------------------------------
  // Determine run mode
  // ---------------------------------------------------------------------------
  const runMode = await getRunMode();
  console.log("SIMULATED ledger only: no real SPL assets or verified investor eligibility.");
  if (runMode === "live") {
    const requiredMethods = ["initializeFacility", "activateFacility", "updateFacilityMetadata", "depositCapital", "borrowerDrawdown", "accrueInterest", "updateCovenantState", "togglePik", "capitalizePik", "updateCollateralStatus", "makeRepayment", "requestRedemption", "cancelRedemption", "processRedemption", "freezeInvestorPosition", "markMatured", "closeFacility", "appendEventLog"];
    const missing = requiredMethods.filter((name) => !methodExists(name));
    if (missing.length) throw new Error(`Stale IDL; rebuild the modular program. Missing: ${missing.join(", ")}`);
  }

  // ---------------------------------------------------------------------------
  // Demo values
  // ---------------------------------------------------------------------------
  const UNIT = new BN(1_000_000); // pretend stablecoin has 6 decimals

  const facilitySize = new BN(5_000).mul(UNIT);
  const maxDrawdownAmount = new BN(3_000).mul(UNIT);
  const depositAmount = new BN(2_000).mul(UNIT);
  const drawdownAmount = new BN(1_000).mul(UNIT);
  const pikAmount = new BN(25).mul(UNIT);
  const repaymentAmount = new BN(1_025).mul(UNIT);
  const principalPortion = new BN(1_025).mul(UNIT);
  const interestPortion = new BN(0);
  const firstRedemptionAmount = new BN(500).mul(UNIT);
  const secondRedemptionAmount = new BN(400).mul(UNIT);

  const interestRateBps = 1_200; // 12.00%
  const now = Math.floor(Date.now() / 1000);
  const maturityTimestamp = new BN(now + 60 * 60 * 24 * 180); // 180 days

  const collateralDescriptionHash = Array(32).fill(7);
  const updatedCollateralDescriptionHash = Array(32).fill(9);

  const minimumCollateralCoverageBps = new BN(12_500); // 125%
  const minimumLiquidityReserve = new BN(100).mul(UNIT);

  let eventIndex = 0;

  // ---------------------------------------------------------------------------
  // 0. initialize_facility
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("initializeFacility", "initializeFacility", eventIndex, () =>
      program.methods
        .initializeFacility(
          facilityId,
          new BN(eventIndex),
          stablecoinMint,
          facilitySize,
          interestRateBps,
          maturityTimestamp,
          maxDrawdownAmount,
          collateralDescriptionHash,
          minimumCollateralCoverageBps,
          minimumLiquidityReserve
        )
        .accounts({
          operator,
          borrower,
          facility: facilityPda,
          repaymentSchedule: repaymentSchedulePda,
          covenantState: covenantStatePda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 1. activate_facility
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("activateFacility", "activateFacility", eventIndex, () =>
      program.methods
        .activateFacility(new BN(eventIndex))
        .accounts({
          operator,
          facility: facilityPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 2. update_facility_metadata
  // This is done before drawdown so maturity/max drawdown are still editable.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("updateFacilityMetadata", "updateFacilityMetadata", eventIndex, () =>
      program.methods
        .updateFacilityMetadata(
          updatedCollateralDescriptionHash,
          new BN(now + 60 * 60 * 24 * 181),
          new BN(3_000).mul(UNIT),
          null,
          new BN(12_000),
          minimumLiquidityReserve,
          new BN(eventIndex)
        )
        .accounts({
          operator,
          facility: facilityPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 3. deposit_capital
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("depositCapital", "depositCapital", eventIndex, () =>
      program.methods
        .depositCapital(depositAmount, new BN(eventIndex))
        .accounts({
          investor,
          facility: facilityPda,
          investorPosition: investorPositionPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 4. borrower_drawdown
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("borrowerDrawdown", "borrowerDrawdown", eventIndex, () =>
      program.methods
        .borrowerDrawdown(drawdownAmount, new BN(eventIndex))
        .accounts({
          actor: borrower,
          facility: facilityPda,
          repaymentSchedule: repaymentSchedulePda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 5. accrue_interest
  // This may accrue 0 if run immediately, but it proves the instruction works.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("accrueInterest", "accrueInterest", eventIndex, () =>
      program.methods
        .accrueInterest(new BN(eventIndex))
        .accounts({
          operator,
          facility: facilityPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 6. update_covenant_state
  // Non-breached update to show covenant health tracking.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("updateCovenantState", "updateCovenantState", eventIndex, () =>
      program.methods
        .updateCovenantState(
          new BN(15000), // DSCR bps: 150%
          new BN(5000),  // LTV bps: 50%
          new BN(1000),  // minimum liquidity bps: 10%
          true,          // reporting current
          false,         // breached
          "",            // breach reason
          new BN(eventIndex)
        )
        .accounts({
          operator,
          facility: facilityPda,
          covenantState: covenantStatePda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 7. toggle_pik
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("togglePik", "togglePik", eventIndex, () =>
      program.methods
        .togglePik(true, pikAmount, new BN(eventIndex))
        .accounts({
          operator,
          facility: facilityPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 8. capitalize_pik
  // Moves accrued PIK into outstanding principal.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("capitalizePik", "capitalizePik", eventIndex, () =>
      program.methods
        .capitalizePik(new BN(eventIndex))
        .accounts({
          operator,
          facility: facilityPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 9. update_collateral_status
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("updateCollateralStatus", "updateCollateralStatus", eventIndex, () =>
      program.methods
        .updateCollateralStatus(
          new BN(2_000).mul(UNIT),
          { healthy: {} },
          false,
          new BN(eventIndex)
        )
        .accounts({
          operator,
          facility: facilityPda,
          covenantState: covenantStatePda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 10. make_repayment
  // Repay principal first; the program accrues interest against the old principal.
  // Once principal reaches zero, read and settle the actual remaining interest.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("makeRepayment", "makeRepayment", eventIndex, () =>
      program.methods
        .makeRepayment(
          repaymentAmount,
          principalPortion,
          interestPortion,
          false, // no verified contractual schedule satisfaction
          new BN(eventIndex)
        )
        .accounts({
          borrower,
          facility: facilityPda,
          repaymentSchedule: repaymentSchedulePda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // Pay the remaining interest from on-chain evidence, not a guessed amount.
  // In dry-run mode this is a conditional step; no account fetch or transaction.
  if (runMode === "dry-run") {
    console.log("[dry-run] Conditional interest-only repayment if accruedInterest > 0 after principal repayment.");
    console.log("[dry-run] Subsequent event indexes assume this optional step is omitted.");
  } else {
    const debt = await program.account.facility.fetch(facilityPda);
    if (!debt.outstandingPrincipal.isZero()) throw new Error("Principal settlement incomplete");
    if (!debt.totalPikAccrued.isZero()) throw new Error("Uncapitalized PIK remains");
    if (!debt.accruedInterest.isZero()) {
      const interest = debt.accruedInterest;
      if (!(await sendOrDry("settleAccruedInterest", "makeRepayment", eventIndex, () =>
        program.methods.makeRepayment(interest, new BN(0), interest, false, new BN(eventIndex))
          .accounts({ borrower, facility: facilityPda, repaymentSchedule: repaymentSchedulePda,
            eventLog: eventLogPda(eventIndex), systemProgram: web3.SystemProgram.programId }).rpc()
      ))) return;
      eventIndex++;
    }
    const settled = await program.account.facility.fetch(facilityPda);
    if (!settled.outstandingPrincipal.isZero() || !settled.accruedInterest.isZero() || !settled.totalPikAccrued.isZero()) {
      throw new Error("Recorded obligations remain; refusing to close");
    }
  }

  // ---------------------------------------------------------------------------
  // 11. request_redemption
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("requestRedemption", "requestRedemption", eventIndex, () =>
      program.methods
        .requestRedemption(firstRedemptionAmount, new BN(eventIndex))
        .accounts({
          investor,
          facility: facilityPda,
          investorPosition: investorPositionPda,
          redemptionRequest: redemptionRequestPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 12. cancel_redemption
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("cancelRedemption", "cancelRedemption", eventIndex, () =>
      program.methods
        .cancelRedemption(new BN(eventIndex))
        .accounts({
          investor,
          facility: facilityPda,
          investorPosition: investorPositionPda,
          redemptionRequest: redemptionRequestPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 13. request_redemption again
  // The upgraded lib.rs allows a processed/cancelled request account to be reused.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("requestRedemptionAgain", "requestRedemption", eventIndex, () =>
      program.methods
        .requestRedemption(secondRedemptionAmount, new BN(eventIndex))
        .accounts({
          investor,
          facility: facilityPda,
          investorPosition: investorPositionPda,
          redemptionRequest: redemptionRequestPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 14. process_redemption
  // Uses the minimum liquidity reserve gate.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("processRedemption", "processRedemption", eventIndex, () =>
      program.methods
        .processRedemption(secondRedemptionAmount, new BN(eventIndex))
        .accounts({
          operator,
          investor,
          facility: facilityPda,
          investorPosition: investorPositionPda,
          redemptionRequest: redemptionRequestPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // Settle remaining fixed-price shares before demonstrating a freeze/closure.
  // This avoids leaving known investor claims outstanding in the happy-path demo.
  const remainingShares = depositAmount.sub(secondRedemptionAmount);
  if (runMode === "live") {
    const position = await program.account.investorPosition.fetch(investorPositionPda);
    if (!position.shareBalance.eq(remainingShares)) throw new Error("Unexpected share balance");
  }
  if (!(await sendOrDry("requestFinalRedemption", "requestRedemption", eventIndex, () =>
    program.methods.requestRedemption(remainingShares, new BN(eventIndex))
      .accounts({ investor, facility: facilityPda, investorPosition: investorPositionPda,
        redemptionRequest: redemptionRequestPda, eventLog: eventLogPda(eventIndex),
        systemProgram: web3.SystemProgram.programId }).rpc()
  ))) return;
  eventIndex++;
  if (!(await sendOrDry("processFinalRedemption", "processRedemption", eventIndex, () =>
    program.methods.processRedemption(remainingShares, new BN(eventIndex))
      .accounts({ operator, investor, facility: facilityPda, investorPosition: investorPositionPda,
        redemptionRequest: redemptionRequestPda, eventLog: eventLogPda(eventIndex),
        systemProgram: web3.SystemProgram.programId }).rpc()
  ))) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 15. freeze_investor_position
  // Demonstrates simulated position status controls after all shares are redeemed.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("freezeInvestorPosition", "freezeInvestorPosition", eventIndex, () =>
      program.methods
        .freezeInvestorPosition(new BN(eventIndex))
        .accounts({
          operator,
          investor,
          facility: facilityPda,
          investorPosition: investorPositionPda,
          redemptionRequest: redemptionRequestPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // 16. mark_matured
  // Allowed because outstanding principal is now 0.
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("markMatured", "markMatured", eventIndex, () =>
      program.methods
        .markMatured(new BN(eventIndex))
        .accounts({
          operator,
          facility: facilityPda,
          repaymentSchedule: repaymentSchedulePda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  if (!(await sendOrDry("appendAdministrativeNote", "appendEventLog", eventIndex, () =>
    program.methods.appendEventLog(new BN(eventIndex), { administrativeNote: {} }, new BN(0), 1)
      .accounts({ operator, facility: facilityPda, eventLog: eventLogPda(eventIndex),
        systemProgram: web3.SystemProgram.programId }).rpc()
  ))) return;
  eventIndex++;

  if (runMode === "live") {
    const f = await program.account.facility.fetch(facilityPda);
    const pos = await program.account.investorPosition.fetch(investorPositionPda);
    if (!f.outstandingPrincipal.isZero() || !f.accruedInterest.isZero() ||
        !f.totalPikAccrued.isZero() || !f.totalPendingRedemptions.isZero() || !pos.shareBalance.isZero()) {
      throw new Error("Known investor or borrower obligations remain; closure refused");
    }
  }

  // ---------------------------------------------------------------------------
  // 17. close_facility
  // ---------------------------------------------------------------------------
  if (
    !(await sendOrDry("closeFacility", "closeFacility", eventIndex, () =>
      program.methods
        .closeFacility(false, new BN(eventIndex))
        .accounts({
          operator,
          facility: facilityPda,
          eventLog: eventLogPda(eventIndex),
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc()
    ))
  ) return;
  eventIndex++;

  // ---------------------------------------------------------------------------
  // Fetch final state only in live mode
  // ---------------------------------------------------------------------------
  if (runMode === "dry-run") {
    console.log("");
    console.log("Dry-run complete.");
    console.log("No transactions were sent because the program is not deployed.");
    console.log("After Build + Deploy, run this same client again for live execution.");
    return;
  }

  try {
    const facilityAccount = await program.account.facility.fetch(facilityPda);
    const investorPositionAccount = await program.account.investorPosition.fetch(
      investorPositionPda
    );
    const redemptionRequestAccount = await program.account.redemptionRequest.fetch(
      redemptionRequestPda
    );

    console.log("");
    console.log("Final facility state:");
    console.log(facilityAccount);
    if (!Object.prototype.hasOwnProperty.call(facilityAccount.status, "closed") ||
        !facilityAccount.eventIndex.eq(new BN(eventIndex)) || !investorPositionAccount.shareBalance.isZero() ||
        !facilityAccount.outstandingPrincipal.isZero() || !facilityAccount.accruedInterest.isZero() ||
        !facilityAccount.totalPikAccrued.isZero() || !facilityAccount.totalPendingRedemptions.isZero()) {
      throw new Error("Final state invariant failed");
    }

    console.log("");
    console.log("Final investor position:");
    console.log(investorPositionAccount);

    console.log("");
    console.log("Final redemption request:");
    console.log(redemptionRequestAccount);
  } catch (err) {
    throw new Error(`Demo final state could not be verified: ${String(err)}`);
  }

  console.log("Demo complete.");
}

await main();
