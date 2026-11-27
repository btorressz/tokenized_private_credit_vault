// Solana Playground integration tests for the modular Anchor MVP.
// No imports needed: web3, anchor, pg, BN, assert are globally available.
// Requires a freshly built/deployed program and matching IDL. No real SPL transfers.

describe("Tokenized Private Credit Vault + Covenant Monitor", () => {
  const program = pg.program;
  const programId = pg.program.programId;

  const operator = pg.wallet.publicKey;

  // For Playground simplicity, this test uses the same wallet as:
  // operator, borrower, and investor.
  //
  // This avoids airdrop/funding issues before your program is deployed.
  // In a deeper test, you can replace borrower/investor with generated
  // Keypairs and airdrop SOL to them.
  const borrower = pg.wallet.publicKey;
  const investor = pg.wallet.publicKey;

  const facilityId = `pcv-${Date.now()}-${web3.Keypair.generate().publicKey.toBase58().slice(0, 6)}`;
  const stablecoinMint = web3.Keypair.generate().publicKey;

  let eventIndex = 0;

  const bn = (value: number | string) => new BN(value);

  const u64LeBytes = (value: number) => {
    return new BN(value).toArrayLike(Buffer, "le", 8);
  };

  const hash32 = (value: number) => {
    return Array(32).fill(value);
  };

  const findPda = (seeds: Buffer[]) => {
    return web3.PublicKey.findProgramAddressSync(seeds, programId)[0];
  };

  const isVariant = (enumValue: any, variant: string) => {
    return Object.prototype.hasOwnProperty.call(enumValue, variant);
  };

  const assertVariant = (enumValue: any, variant: string) => {
    assert(
      isVariant(enumValue, variant),
      `Expected enum variant ${variant}, got ${JSON.stringify(enumValue)}`
    );
  };

  const assertBnLikeEq = (actual: any, expected: number | string, label: string) => {
    const expectedBn = bn(expected);

    if (actual && typeof actual.eq === "function") {
      assert(
        actual.eq(expectedBn),
        `${label} expected ${expectedBn.toString()}, got ${actual.toString()}`
      );
      return;
    }

    assert(
      actual === Number(expected),
      `${label} expected ${expected.toString()}, got ${actual}`
    );
  };

  const confirm = async (txHash: string) => {
    console.log(`Transaction: ${txHash}`);
    const confirmation = await pg.connection.confirmTransaction(txHash, "confirmed");
    assert(confirmation.value.err === null, `Transaction failed: ${JSON.stringify(confirmation.value.err)}`);
  };

  const ensureProgramDeployed = async () => {
    const programAccount = await pg.connection.getAccountInfo(programId);
    assert(programAccount && programAccount.executable,
      "Integration tests require a deployed modular program; build and deploy on a test cluster first.");
  };

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

  const nextEventPda = () => {
    const currentIndex = eventIndex;

    const eventLog = findPda([
      Buffer.from("event"),
      facilityPda.toBuffer(),
      u64LeBytes(currentIndex),
    ]);

    eventIndex += 1;

    return {
      eventIndex: bn(currentIndex),
      eventLog,
    };
  };

  // Negative attempts use the next event PDA without advancing the sequence.
  // A rejected transaction must roll back both the financial changes and event creation.
  const expectRejected = async (label: string, expected: string, send: (ix: any) => Promise<string>) => {
    const before = await program.account.facility.fetch(facilityPda);
    assert(before.eventIndex.eq(bn(eventIndex)), label + ": precondition event index");
    const eventLog = findPda([Buffer.from("event"), facilityPda.toBuffer(), u64LeBytes(eventIndex)]);
    let failure: any;
    try {
      await send({ eventIndex: bn(eventIndex), eventLog });
    } catch (err) {
      failure = err;
    }
    assert(failure, label + ": expected a rejected transaction");
    const detail = [failure?.error?.errorCode?.code, failure?.message, ...(failure?.logs ?? [])].join(" ");
    assert(detail.includes(expected), label + ": unexpected error: " + detail);
    const after = await program.account.facility.fetch(facilityPda);
    for (const field of ["eventIndex", "availableLiquidity", "outstandingPrincipal", "accruedInterest",
      "totalDeposited", "totalDrawn", "totalRepaid", "totalPendingRedemptions", "totalPikAccrued"]) {
      assert((after as any)[field].eq((before as any)[field]), label + ": changed " + field);
    }
    assert(JSON.stringify(after.status) === JSON.stringify(before.status), label + ": changed status");
    assert((await pg.connection.getAccountInfo(eventLog)) === null, label + ": event account was created");
  };

  it("runs the modular private credit lifecycle and rejects invalid transitions", async () => {
    await ensureProgramDeployed();

    {
      console.log("Program ID:", programId.toBase58());
      console.log("Facility ID:", facilityId);
      console.log("Facility PDA:", facilityPda.toBase58());
      console.log("Investor Position PDA:", investorPositionPda.toBase58());
      console.log("Redemption Request PDA:", redemptionRequestPda.toBase58());

      const now = Math.floor(Date.now() / 1000);
      const maturityTimestamp = now + 60 * 60 * 24 * 365;

      // -----------------------------------------------------------------------
      // 1. initialize_facility
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .initializeFacility(
            facilityId,
            ixEventIndex,
            stablecoinMint,
            bn(1_000_000), // facility_size
            1200, // interest_rate_bps = 12.00%
            bn(maturityTimestamp),
            bn(800_000), // max_drawdown_amount
            hash32(1), // collateral_description_hash
            bn(12_000), // minimum_collateral_coverage_bps = 120%
            bn(50_000) // minimum_liquidity_reserve
          )
          .accounts({
            operator,
            borrower,
            facility: facilityPda,
            repaymentSchedule: repaymentSchedulePda,
            covenantState: covenantStatePda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assert(facility.facilityId === facilityId);
        assert(facility.operator.equals(operator));
        assert(facility.borrower.equals(borrower));
        assert(facility.facilitySize.eq(bn(1_000_000)));
        assert(facility.maxDrawdownAmount.eq(bn(800_000)));
        assert(facility.minimumCollateralCoverageBps.eq(bn(12_000)));
        assert(facility.minimumLiquidityReserve.eq(bn(50_000)));
        assertVariant(facility.status, "initialized");
        assertVariant(facility.paymentStatus, "current");

        console.log("Initialized facility");
      }

      // -----------------------------------------------------------------------
      // 2. update_facility_metadata before drawdown
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .updateFacilityMetadata(
            hash32(2), // new collateral hash
            null, // new maturity timestamp
            bn(900_000), // new max drawdown amount
            null, // new borrower
            bn(12_500), // new minimum collateral coverage bps = 125%
            bn(50_000), // new minimum liquidity reserve
            ixEventIndex
          )
          .accounts({
            operator,
            facility: facilityPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assert(facility.maxDrawdownAmount.eq(bn(900_000)));
        assert(facility.minimumCollateralCoverageBps.eq(bn(12_500)));
        assert(facility.minimumLiquidityReserve.eq(bn(50_000)));

        console.log("Updated facility metadata");
      }

      // -----------------------------------------------------------------------
      // 3. activate_facility
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .activateFacility(ixEventIndex)
          .accounts({
            operator,
            facility: facilityPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assertVariant(facility.status, "active");

        console.log("Activated facility");
      }

      // -----------------------------------------------------------------------
      // 4. deposit_capital
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .depositCapital(bn(700_000), ixEventIndex)
          .accounts({
            investor,
            facility: facilityPda,
            investorPosition: investorPositionPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);
        const position = await program.account.investorPosition.fetch(
          investorPositionPda
        );

        assert(facility.totalDeposited.eq(bn(700_000)));
        assert(facility.availableLiquidity.eq(bn(700_000)));
        assert(position.principal.eq(bn(700_000)));
        assert(position.shareBalance.eq(bn(700_000)));
        assert(position.depositedAmount.eq(bn(700_000)));
        assertVariant(position.status, "active");

        console.log("Deposited capital");
      }

      // A draw within cash and facility limits can still violate the reserve.
      await expectRejected("reserve breach", "InsufficientLiquidityReserve", ({ eventIndex, eventLog }) =>
        program.methods.borrowerDrawdown(bn(675_000), eventIndex)
          .accounts({ actor: borrower, facility: facilityPda, repaymentSchedule: repaymentSchedulePda,
            eventLog, systemProgram: web3.SystemProgram.programId }).rpc());

      // -----------------------------------------------------------------------
      // 5. borrower_drawdown
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .borrowerDrawdown(bn(400_000), ixEventIndex)
          .accounts({
            actor: borrower,
            facility: facilityPda,
            repaymentSchedule: repaymentSchedulePda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assert(facility.totalDrawn.eq(bn(400_000)));
        assert(facility.outstandingPrincipal.eq(bn(400_000)));
        assert(facility.availableLiquidity.eq(bn(300_000)));
        assertVariant(facility.status, "drawn");
        assertVariant(facility.paymentStatus, "due");

        console.log("Borrower drew down capital");
      }

      await expectRejected("premature missed payment", "PaymentNotYetDue", ({ eventIndex, eventLog }) =>
        program.methods.recordMissedPayment(eventIndex)
          .accounts({ operator, facility: facilityPda, repaymentSchedule: repaymentSchedulePda,
            eventLog, systemProgram: web3.SystemProgram.programId }).rpc());
      await expectRejected("fully-resolved flag cannot waive debt", "CannotCloseFacility", ({ eventIndex, eventLog }) =>
        program.methods.closeFacility(true, eventIndex)
          .accounts({ operator, facility: facilityPda, eventLog, systemProgram: web3.SystemProgram.programId }).rpc());
      await expectRejected("manual financial event", "InvalidEventTypeForAdminLog", ({ eventIndex, eventLog }) =>
        program.methods.appendEventLog(eventIndex, { capitalDeposited: {} }, bn(123), 0)
          .accounts({ operator, facility: facilityPda, eventLog, systemProgram: web3.SystemProgram.programId }).rpc());
      await expectRejected("excess interest payment", "InvalidRepaymentAmount", ({ eventIndex, eventLog }) =>
        program.methods.makeRepayment(bn("18446744073709551615"), bn(0), bn("18446744073709551615"), false, eventIndex)
          .accounts({ borrower, facility: facilityPda, repaymentSchedule: repaymentSchedulePda,
            eventLog, systemProgram: web3.SystemProgram.programId }).rpc());

      // -----------------------------------------------------------------------
      // 6. accrue_interest
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .accrueInterest(ixEventIndex)
          .accounts({
            operator,
            facility: facilityPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assert(facility.lastInterestAccrualTimestamp.gte(bn(0)));

        console.log("Accrued interest:", facility.accruedInterest.toString());
      }

      // -----------------------------------------------------------------------
      // 7. toggle_pik
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .togglePik(true, bn(10_000), ixEventIndex)
          .accounts({
            operator,
            facility: facilityPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assert(facility.pikEnabled === true);
        assert(facility.totalPikAccrued.eq(bn(10_000)));

        console.log("Toggled PIK on and accrued simulated PIK");
      }

      // -----------------------------------------------------------------------
      // 8. capitalize_pik
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .capitalizePik(ixEventIndex)
          .accounts({
            operator,
            facility: facilityPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assert(facility.totalPikAccrued.eq(bn(0)));
        assert(facility.outstandingPrincipal.eq(bn(410_000)));

        console.log("Capitalized PIK into principal");
      }

      // -----------------------------------------------------------------------
      // 9. update_collateral_status
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .updateCollateralStatus(
            bn(900_000), // collateral_value
            { healthy: {} }, // CollateralStatus::Healthy
            false, // collateral_impaired
            ixEventIndex
          )
          .accounts({
            operator,
            facility: facilityPda,
            covenantState: covenantStatePda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);
        const covenant = await program.account.covenantState.fetch(
          covenantStatePda
        );

        assertVariant(facility.collateralStatus, "healthy");
        assertVariant(covenant.collateralStatus, "healthy");
        assert(covenant.collateralValue.eq(bn(900_000)));
        assert(covenant.collateralImpaired === false);

        console.log("Updated collateral status");
      }

      // -----------------------------------------------------------------------
      // 10. make_repayment partial
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .makeRepayment(
            bn(200_000), // amount
            bn(200_000), // principal_portion
            bn(0), // interest is settled separately using actual accrued debt
            false, // no verified scheduled payment satisfaction
            ixEventIndex
          )
          .accounts({
            borrower,
            facility: facilityPda,
            repaymentSchedule: repaymentSchedulePda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);
        const schedule = await program.account.repaymentSchedule.fetch(
          repaymentSchedulePda
        );

        assert(facility.outstandingPrincipal.eq(bn(210_000)));
        assert(facility.availableLiquidity.eq(bn(500_000)));

        // Fixed: no .eg typo. This works whether Anchor returns u32 as number or BN.
        assertBnLikeEq(schedule.paymentsMade, 1, "schedule.paymentsMade");

        console.log("Made partial repayment");
      }

      // -----------------------------------------------------------------------
      // 11. request_redemption
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .requestRedemption(bn(100_000), ixEventIndex)
          .accounts({
            investor,
            facility: facilityPda,
            investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const position = await program.account.investorPosition.fetch(
          investorPositionPda
        );
        const request = await program.account.redemptionRequest.fetch(
          redemptionRequestPda
        );

        assert(position.pendingRedemptionAmount.eq(bn(100_000)));
        assert(request.requestedAmount.eq(bn(100_000)));
        assertVariant(request.status, "queued");

        console.log("Requested redemption");
      }

      // -----------------------------------------------------------------------
      // 12. cancel_redemption
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .cancelRedemption(ixEventIndex)
          .accounts({
            investor,
            facility: facilityPda,
            investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);
        const position = await program.account.investorPosition.fetch(
          investorPositionPda
        );
        const request = await program.account.redemptionRequest.fetch(
          redemptionRequestPda
        );

        assert(facility.totalPendingRedemptions.eq(bn(0)));
        assert(position.pendingRedemptionAmount.eq(bn(0)));
        assertVariant(request.status, "cancelled");

        console.log("Cancelled redemption");
      }

      // -----------------------------------------------------------------------
      // 13. request_redemption again after cancellation
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .requestRedemption(bn(150_000), ixEventIndex)
          .accounts({
            investor,
            facility: facilityPda,
            investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);
        const position = await program.account.investorPosition.fetch(
          investorPositionPda
        );
        const request = await program.account.redemptionRequest.fetch(
          redemptionRequestPda
        );

        assert(facility.totalPendingRedemptions.eq(bn(150_000)));
        assert(position.pendingRedemptionAmount.eq(bn(150_000)));
        assert(request.requestedAmount.eq(bn(150_000)));
        assertVariant(request.status, "queued");

        console.log("Requested redemption again");
      }

      // -----------------------------------------------------------------------
      // 14. process_redemption partial
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .processRedemption(bn(100_000), ixEventIndex)
          .accounts({
            operator,
            investor,
            facility: facilityPda,
            investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);
        const position = await program.account.investorPosition.fetch(
          investorPositionPda
        );
        const request = await program.account.redemptionRequest.fetch(
          redemptionRequestPda
        );

        assert(facility.availableLiquidity.eq(bn(400_000)));
        assert(facility.totalPendingRedemptions.eq(bn(50_000)));
        assert(position.shareBalance.eq(bn(600_000)));
        assert(position.pendingRedemptionAmount.eq(bn(50_000)));
        assert(request.processedAmount.eq(bn(100_000)));
        assertVariant(request.status, "partiallyProcessed");

        console.log("Processed partial redemption");
      }

      // -----------------------------------------------------------------------
      // 15. process_redemption remaining
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .processRedemption(bn(50_000), ixEventIndex)
          .accounts({
            operator,
            investor,
            facility: facilityPda,
            investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);
        const position = await program.account.investorPosition.fetch(
          investorPositionPda
        );
        const request = await program.account.redemptionRequest.fetch(
          redemptionRequestPda
        );

        assert(facility.availableLiquidity.eq(bn(350_000)));
        assert(facility.totalPendingRedemptions.eq(bn(0)));
        assert(position.shareBalance.eq(bn(550_000)));
        assert(position.pendingRedemptionAmount.eq(bn(0)));
        assert(request.processedAmount.eq(bn(150_000)));
        assertVariant(request.status, "processed");

        console.log("Processed remaining redemption");
      }

      // -----------------------------------------------------------------------
      // 17. make_repayment final principal
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .makeRepayment(
            bn(210_000), // amount
            bn(210_000), // principal_portion
            bn(0), // interest_portion
            false,
            ixEventIndex
          )
          .accounts({
            borrower,
            facility: facilityPda,
            repaymentSchedule: repaymentSchedulePda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assert(facility.outstandingPrincipal.eq(bn(0)));
        assert(facility.availableLiquidity.eq(bn(560_000)));
        if (facility.accruedInterest.isZero()) assertVariant(facility.status, "matured");

        console.log("Repaid remaining principal");
      }

      // Principal is zero, so no new interest can accrue during the sweep.
      {
        const debt = await program.account.facility.fetch(facilityPda);
        if (!debt.accruedInterest.isZero()) {
          const interest = debt.accruedInterest;
          const { eventIndex: ixEventIndex, eventLog } = nextEventPda();
          const txHash = await program.methods.makeRepayment(interest, bn(0), interest, false, ixEventIndex)
            .accounts({ borrower, facility: facilityPda, repaymentSchedule: repaymentSchedulePda,
              eventLog, systemProgram: web3.SystemProgram.programId }).rpc();
          await confirm(txHash);
          const after = await program.account.facility.fetch(facilityPda);
          assert(after.accruedInterest.isZero());
          assert(after.availableLiquidity.eq(debt.availableLiquidity.add(interest)));
        }
        const settled = await program.account.facility.fetch(facilityPda);
        assert(settled.outstandingPrincipal.isZero());
        assert(settled.accruedInterest.isZero());
        assert(settled.totalPikAccrued.isZero());
        assertVariant(settled.status, "matured");
      }

      // -----------------------------------------------------------------------
      // 18. mark_matured
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .markMatured(ixEventIndex)
          .accounts({
            operator,
            facility: facilityPda,
            repaymentSchedule: repaymentSchedulePda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assertVariant(facility.status, "matured");

        console.log("Marked facility matured");
      }

      // Settle remaining known shares using the debt-free matured reserve exception.
      {
        const position = await program.account.investorPosition.fetch(investorPositionPda);
        assert(position.shareBalance.eq(bn(550_000)));
        const amount = position.shareBalance;
        const requestIx = nextEventPda();
        await confirm(await program.methods.requestRedemption(amount, requestIx.eventIndex)
          .accounts({ investor, facility: facilityPda, investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda, eventLog: requestIx.eventLog,
            systemProgram: web3.SystemProgram.programId }).rpc());
        const before = await program.account.facility.fetch(facilityPda);
        const processIx = nextEventPda();
        await confirm(await program.methods.processRedemption(amount, processIx.eventIndex)
          .accounts({ operator, investor, facility: facilityPda, investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda, eventLog: processIx.eventLog,
            systemProgram: web3.SystemProgram.programId }).rpc());
        const after = await program.account.facility.fetch(facilityPda);
        const finalPosition = await program.account.investorPosition.fetch(investorPositionPda);
        assert(after.availableLiquidity.eq(before.availableLiquidity.sub(amount)));
        // This settlement is permitted even if it consumes the reserve.
        assert(after.outstandingPrincipal.isZero() && after.accruedInterest.isZero());
        assert(finalPosition.shareBalance.isZero());
        assert(finalPosition.pendingRedemptionAmount.isZero());
        assert(after.totalPendingRedemptions.isZero());
      }

      // -----------------------------------------------------------------------
      // Freeze only after settling the known investor shares
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .freezeInvestorPosition(ixEventIndex)
          .accounts({
            operator,
            investor,
            facility: facilityPda,
            investorPosition: investorPositionPda,
            redemptionRequest: redemptionRequestPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const position = await program.account.investorPosition.fetch(
          investorPositionPda
        );

        assertVariant(position.status, "frozen");

        console.log("Froze investor position");
      }


      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();
        await confirm(await program.methods.appendEventLog(ixEventIndex, { administrativeNote: {} }, bn(0), 1)
          .accounts({ operator, facility: facilityPda, eventLog, systemProgram: web3.SystemProgram.programId }).rpc());
        const note = await program.account.eventLog.fetch(eventLog);
        assertVariant(note.eventType, "administrativeNote");
        assert(note.eventIndex.eq(ixEventIndex));
        assert(note.facility.equals(facilityPda));
      }

      // -----------------------------------------------------------------------
      // 19. close_facility
      // -----------------------------------------------------------------------

      {
        const { eventIndex: ixEventIndex, eventLog } = nextEventPda();

        const txHash = await program.methods
          .closeFacility(false, ixEventIndex)
          .accounts({
            operator,
            facility: facilityPda,
            eventLog,
            systemProgram: web3.SystemProgram.programId,
          })
          .rpc();

        await confirm(txHash);

        const facility = await program.account.facility.fetch(facilityPda);

        assertVariant(facility.status, "closed");

        console.log("Closed facility");
      }

      // -----------------------------------------------------------------------
      // Final checks
      // -----------------------------------------------------------------------

      const finalFacility = await program.account.facility.fetch(facilityPda);
      const finalPosition = await program.account.investorPosition.fetch(
        investorPositionPda
      );
      const finalRequest = await program.account.redemptionRequest.fetch(
        redemptionRequestPda
      );

      console.log("Final facility state:", {
        facilityId: finalFacility.facilityId,
        status: finalFacility.status,
        paymentStatus: finalFacility.paymentStatus,
        totalDeposited: finalFacility.totalDeposited.toString(),
        totalDrawn: finalFacility.totalDrawn.toString(),
        totalRepaid: finalFacility.totalRepaid.toString(),
        availableLiquidity: finalFacility.availableLiquidity.toString(),
        outstandingPrincipal: finalFacility.outstandingPrincipal.toString(),
        accruedInterest: finalFacility.accruedInterest.toString(),
        totalPendingRedemptions:
          finalFacility.totalPendingRedemptions.toString(),
        eventIndex: finalFacility.eventIndex.toString(),
      });

      console.log("Final investor position:", {
        principal: finalPosition.principal.toString(),
        shareBalance: finalPosition.shareBalance.toString(),
        depositedAmount: finalPosition.depositedAmount.toString(),
        redeemedAmount: finalPosition.redeemedAmount.toString(),
        pendingRedemptionAmount:
          finalPosition.pendingRedemptionAmount.toString(),
        status: finalPosition.status,
      });

      console.log("Final redemption request:", {
        requestedAmount: finalRequest.requestedAmount.toString(),
        processedAmount: finalRequest.processedAmount.toString(),
        status: finalRequest.status,
        queueIndex: finalRequest.queueIndex.toString(),
      });

      assertVariant(finalFacility.status, "closed");
      assert(finalFacility.outstandingPrincipal.eq(bn(0)));
      assert(finalFacility.totalPendingRedemptions.eq(bn(0)));
      assert(finalPosition.redeemedAmount.eq(bn(700_000)));
      assert(finalPosition.shareBalance.isZero());
      assert(finalFacility.accruedInterest.isZero());
      assert(finalFacility.totalPikAccrued.isZero());
      assert(finalFacility.eventIndex.eq(bn(eventIndex)));
      // Simulated cash conservation across deposits, draws, repayments and redemptions.
      assert(finalFacility.availableLiquidity.eq(finalFacility.totalDeposited.sub(finalFacility.totalDrawn).add(finalFacility.totalRepaid).sub(finalPosition.redeemedAmount)));
      for (let i = 0; i < eventIndex; i++) {
        const key = findPda([Buffer.from("event"), facilityPda.toBuffer(), u64LeBytes(i)]);
        const event = await program.account.eventLog.fetch(key);
        assert(event.facility.equals(facilityPda));
        assert(event.eventIndex.eq(bn(i)));
      }
      assertVariant(finalPosition.status, "frozen");
      assertVariant(finalRequest.status, "processed");
    }
  });
});
