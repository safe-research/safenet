//! EIP-1559 fee calculations for reliable transaction submission.

use alloy::{eips::eip1559::Eip1559Estimation, primitives::U256, uint};

/// Caps the priority fee of `fees` so it is at most `cap_percentage` percent of
/// the total max fee per gas, leaving the base-fee component unchanged.
///
/// Mirrors the validator's gas fee estimator: it solves for the largest priority
/// fee `p` with `p / (base_fee + p) <= cap_percentage / 100`, then lowers the
/// priority fee to it (never raising it). A cap of 0% or negative disables
/// priority fees, and a cap of 100% or more is a no-op.
pub fn cap_priority_fee(fees: Eip1559Estimation, cap_percentage: f64) -> Eip1559Estimation {
    // Scale the percentage into integer space for the fee math, allowing up to
    // six digits of precision in `cap_percentage`.
    const PRECISION: u128 = 1_000_000;
    let scaled_percent = ((cap_percentage.max(0.0) / 100.0) * PRECISION as f64).round() as u128;
    if scaled_percent >= PRECISION {
        return fees;
    }

    let base_fee = fees
        .max_fee_per_gas
        .saturating_sub(fees.max_priority_fee_per_gas);
    let capped = base_fee.saturating_mul(scaled_percent) / (PRECISION - scaled_percent);
    let max_priority_fee_per_gas = fees.max_priority_fee_per_gas.min(capped);

    Eip1559Estimation {
        max_priority_fee_per_gas,
        max_fee_per_gas: base_fee + max_priority_fee_per_gas,
    }
}

/// Returns the `fresh` fee estimate with each component raised to at least 10%
/// above the matching `previous` fee, when a previous submission exists, so a
/// resubmitted transaction replaces rather than duplicates the pending one;
/// nodes reject a replacement that does not raise the fee enough.
///
/// Note that fee bumps can cause priority fee caps to not be observed.
pub fn bump(fresh: Eip1559Estimation, previous: Option<Eip1559Estimation>) -> Eip1559Estimation {
    let Some(previous) = previous else {
        return fresh;
    };
    Eip1559Estimation {
        max_fee_per_gas: bump_fee(fresh.max_fee_per_gas, previous.max_fee_per_gas),
        max_priority_fee_per_gas: bump_fee(
            fresh.max_priority_fee_per_gas,
            previous.max_priority_fee_per_gas,
        ),
    }
}

/// Returns `fresh`, raised to at least 10% above `previous`.
fn bump_fee(fresh: u128, previous: u128) -> u128 {
    let bumped = U256::from(previous)
        .wrapping_mul(uint!(110_U256))
        .div_ceil(uint!(100_U256))
        .try_into()
        .unwrap_or(u128::MAX);
    fresh.max(bumped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fees(max_fee_per_gas: u128, max_priority_fee_per_gas: u128) -> Eip1559Estimation {
        Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        }
    }

    #[test]
    fn caps_the_priority_fee_at_a_percentage_of_the_max_fee() {
        // A priority fee already below the cap is unchanged.
        assert_eq!(cap_priority_fee(fees(100, 10), 50.0), fees(100, 10));

        // Above the cap it is lowered, preserving the base-fee component: the
        // base fee is 40, so the priority fee is capped to 40 * 25/75 = 13 and
        // the max fee to 40 + 13 = 53.
        assert_eq!(cap_priority_fee(fees(100, 60), 25.0), fees(53, 13));

        // A cap of 100% or more never reduces the fee.
        assert_eq!(cap_priority_fee(fees(100, 60), 100.0), fees(100, 60));
    }

    #[test]
    fn bumps_replacement_fees_by_at_least_ten_percent() {
        // A first submission (no previous fees) keeps the fresh estimate.
        assert_eq!(bump(fees(100, 10), None), fees(100, 10));

        // A resubmission raises each fee to 10% above the previous one.
        assert_eq!(bump(fees(100, 10), Some(fees(100, 10))), fees(110, 11));

        // ...unless the fresh estimate already exceeds that.
        assert_eq!(bump(fees(200, 50), Some(fees(100, 10))), fees(200, 50));
    }
}
