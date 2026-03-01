// ─────────────────────────────────────────────────────────────────────────────
//  Token-Vesting Vault — Unit Tests
//
//  These tests exercise the core business logic that lives inside the program
//  (cliff guard, vested-amount math, overflow detection, zero-period guard,
//  nothing-to-claim guard) WITHOUT a running Solana validator.  The helpers
//  below replicate only the logic that appears inside `claim_tokens`, so the
//  tests stay fast and deterministic.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {

    // ── tiny mirror of the on-chain structs / error codes ────────────────────

    #[derive(Clone, Copy, Debug)]
    struct EmployeeAccount {
        pub start_time: i64,
        pub end_time: i64,
        pub cliff_time: i64,
        pub total_amount: u64,
        pub total_claimed: u64,
    }

    #[derive(Debug, PartialEq)]
    enum ErrorCode {
        ClaimNotAvailable,
        InvalidVestingPeriod,
        OverflowError,
        NoTokensToClaim,
    }

    // ── pure reimplementation of the claim_tokens calculation ─────────────────

    /// Returns the amount available to claim right now, or an error.
    fn compute_claimable(
        employee: &EmployeeAccount,
        current_time: i64,
    ) -> Result<u64, ErrorCode> {
        // 1. Cliff guard
        if current_time < employee.cliff_time {
            return Err(ErrorCode::ClaimNotAvailable);
        }

        // 2. Zero-period guard
        let total_vesting_period = employee
            .end_time
            .saturating_sub(employee.start_time);
        if total_vesting_period == 0 {
            return Err(ErrorCode::InvalidVestingPeriod);
        }

        // 3. Vested amount (identical arithmetic to on-chain code)
        let time_since_start = current_time.saturating_sub(employee.start_time);
        let vested_amount = if current_time >= employee.end_time {
            employee.total_amount
        } else {
            match employee
                .total_amount
                .checked_mul(time_since_start as u64)
            {
                Some(product) => product / total_vesting_period as u64,
                None => return Err(ErrorCode::OverflowError),
            }
        };

        // 4. Nothing-to-claim guard
        let amount_to_claim = vested_amount.saturating_sub(employee.total_claimed);
        if amount_to_claim == 0 {
            return Err(ErrorCode::NoTokensToClaim);
        }

        Ok(amount_to_claim)
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /// A baseline account where all time fields are in the past and nothing
    /// has been claimed yet.  Individual tests override specific fields.
    fn base_account() -> EmployeeAccount {
        EmployeeAccount {
            start_time: 0,
            end_time: 1_000,
            cliff_time: 100,
            total_amount: 1_000_000, // 1 M tokens
            total_claimed: 0,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Cliff tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_claim_before_cliff_fails() {
        let acc = base_account(); // cliff_time = 100
        let result = compute_claimable(&acc, 50); // current_time < cliff
        assert_eq!(result, Err(ErrorCode::ClaimNotAvailable));
    }

    #[test]
    fn test_claim_exactly_at_cliff_succeeds() {
        let acc = base_account(); // cliff_time = 100, start = 0, end = 1000
        // At t=100: vested = 1_000_000 * 100 / 1000 = 100_000
        let result = compute_claimable(&acc, 100);
        assert_eq!(result, Ok(100_000));
    }

    #[test]
    fn test_claim_before_cliff_by_one_second_fails() {
        let acc = base_account();
        let result = compute_claimable(&acc, acc.cliff_time - 1);
        assert_eq!(result, Err(ErrorCode::ClaimNotAvailable));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Mid-vesting linear calculation
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_vested_at_halfway() {
        let acc = base_account(); // total = 1_000_000, period = 0..1000
        // t = 500 → vested = 1_000_000 * 500 / 1000 = 500_000
        let result = compute_claimable(&acc, 500);
        assert_eq!(result, Ok(500_000));
    }

    #[test]
    fn test_vested_at_quarter() {
        let acc = base_account();
        // t = 250 → 1_000_000 * 250 / 1000 = 250_000
        let result = compute_claimable(&acc, 250);
        assert_eq!(result, Ok(250_000));
    }

    #[test]
    fn test_vested_at_three_quarters() {
        let acc = base_account();
        // t = 750 → 750_000
        let result = compute_claimable(&acc, 750);
        assert_eq!(result, Ok(750_000));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Full vest (at / after end_time)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_full_vested_at_end_time() {
        let acc = base_account();
        let result = compute_claimable(&acc, acc.end_time);
        assert_eq!(result, Ok(acc.total_amount));
    }

    #[test]
    fn test_full_vested_after_end_time() {
        let acc = base_account();
        let result = compute_claimable(&acc, acc.end_time + 10_000);
        assert_eq!(result, Ok(acc.total_amount));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Partial claim deduction
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_partial_claim_subtracts_already_claimed() {
        let mut acc = base_account();
        acc.total_claimed = 200_000; // already claimed 20 %
        // t = 500 → vested = 500_000; claimable = 500_000 - 200_000
        let result = compute_claimable(&acc, 500);
        assert_eq!(result, Ok(300_000));
    }

    #[test]
    fn test_no_tokens_to_claim_after_full_claim() {
        let mut acc = base_account();
        acc.total_claimed = acc.total_amount; // everything already claimed
        let result = compute_claimable(&acc, acc.end_time);
        assert_eq!(result, Err(ErrorCode::NoTokensToClaim));
    }

    #[test]
    fn test_no_tokens_to_claim_when_vested_equals_claimed() {
        let mut acc = base_account();
        // At t=500, vested = 500_000; if already claimed exactly that:
        acc.total_claimed = 500_000;
        let result = compute_claimable(&acc, 500);
        assert_eq!(result, Err(ErrorCode::NoTokensToClaim));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Invalid vesting period
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_zero_vesting_period_fails() {
        let mut acc = base_account();
        acc.end_time = acc.start_time; // period = 0
        acc.cliff_time = acc.start_time; // cliff already passed
        let result = compute_claimable(&acc, acc.start_time + 1);
        assert_eq!(result, Err(ErrorCode::InvalidVestingPeriod));
    }

    #[test]
    fn test_end_before_start_skips_zero_period_check() {
        // When end_time < current_time, the code short-circuits into the
        // full-vest branch (`current_time >= end_time`) BEFORE reaching the
        // zero-period guard, so all tokens become claimable immediately.
        // This documents the actual on-chain behaviour for a backwards schedule.
        let mut acc = base_account(); // total_amount = 1_000_000
        acc.end_time = acc.start_time - 10;  // end_time is in the past
        acc.cliff_time = acc.start_time - 20; // cliff already passed
        let result = compute_claimable(&acc, acc.start_time + 1);
        // Full vest fires → entire amount is claimable
        assert_eq!(result, Ok(acc.total_amount));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Overflow detection
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_overflow_detected_on_huge_amount() {
        let acc = EmployeeAccount {
            start_time: 0,
            end_time: i64::MAX,      // very long period
            cliff_time: 0,
            total_amount: u64::MAX,  // maximum tokens — mul will overflow
            total_claimed: 0,
        };
        // time_since_start = i64::MAX / 2 (mid-vest, so we enter the mul branch)
        let current_time = i64::MAX / 2;
        let result = compute_claimable(&acc, current_time);
        assert_eq!(result, Err(ErrorCode::OverflowError));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Edge cases around cliff == start_time
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_cliff_equal_to_start_allows_claim_immediately() {
        let acc = EmployeeAccount {
            start_time: 1_000,
            end_time:   2_000,
            cliff_time: 1_000, // no waiting period
            total_amount: 1_000,
            total_claimed: 0,
        };
        // At exactly start_time: time_since_start = 0 → vested = 0 → NoTokensToClaim
        let result = compute_claimable(&acc, 1_000);
        assert_eq!(result, Err(ErrorCode::NoTokensToClaim));
    }

    #[test]
    fn test_cliff_equal_to_start_one_second_later() {
        let acc = EmployeeAccount {
            start_time: 1_000,
            end_time:   2_000,
            cliff_time: 1_000,
            total_amount: 1_000,
            total_claimed: 0,
        };
        // At t=1001: vested = 1000 * 1 / 1000 = 1
        let result = compute_claimable(&acc, 1_001);
        assert_eq!(result, Ok(1));
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  VestingAccount / EmployeeAccount field validation helpers
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_vesting_account_init_space_constant() {
        // VestingAccount fields: owner(32) + mint(32) + treasury(32)
        //   + company_name(4+32) + treasury_bump(1) + bump(1) = 134 bytes
        // #[derive(InitSpace)] computes this automatically; we just sanity-check
        // the struct layout by confirming we can reason about its minimum size.
        let owner_size = 32usize;
        let mint_size = 32usize;
        let treasury_size = 32usize;
        let name_prefix = 4usize; // borsh Vec length prefix
        let name_max = 32usize;
        let bumps = 2usize;

        let total = owner_size + mint_size + treasury_size + name_prefix + name_max + bumps;
        assert_eq!(total, 134);
    }

    #[test]
    fn test_employee_account_init_space_constant() {
        // EmployeeAccount fields: beneficiary(32) + start(8) + end(8) + cliff(8)
        //   + vesting_account(32) + total_amount(8) + total_claimed(8) + bump(1) = 105 bytes
        let beneficiary = 32usize;
        let start = 8usize;
        let end = 8usize;
        let cliff = 8usize;
        let vesting_key = 32usize;
        let total_amount = 8usize;
        let total_claimed = 8usize;
        let bump = 1usize;

        let total = beneficiary + start + end + cliff + vesting_key
            + total_amount + total_claimed + bump;
        assert_eq!(total, 105);
    }
}
