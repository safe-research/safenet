//! Transaction queue configuration.

use alloy::primitives::Address;
use serde::Deserialize;

/// The default in-flight limit when submitting transactions directly.
const DEFAULT_MAX_IN_FLIGHT_TRANSACTIONS: usize = 16;

/// The default gas limit of a single batch.
const DEFAULT_MAX_BATCH_GAS: u64 = 2_000_000;

/// Transaction queue configuration.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(try_from = "RawConfig")]
pub struct Config {
    /// How queued transactions are submitted onchain.
    pub mode: SubmissionMode,
    /// How many blocks a submitted transaction may go unexecuted before it is
    /// resubmitted with a bumped fee.
    pub blocks_before_resubmit: u64,
    /// Caps the priority fee of estimated fees to at most this percentage of the
    /// total max fee per gas, lowering the priority fee (and max fee) when an
    /// estimate exceeds it. `None` applies no cap.
    pub priority_fee_cap_percentage: Option<f64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: SubmissionMode::Direct {
                max_in_flight_transactions: DEFAULT_MAX_IN_FLIGHT_TRANSACTIONS,
            },
            blocks_before_resubmit: 2,
            priority_fee_cap_percentage: None,
        }
    }
}

/// How queued transactions are submitted onchain.
#[derive(Clone, Debug, PartialEq)]
pub enum SubmissionMode {
    /// Each queued transaction is submitted as its own transaction.
    Direct {
        /// The maximum number of transactions that may be in flight (submitted
        /// onchain but not yet executed) at any one time. The queue only
        /// submits new transactions while it is below this limit.
        max_in_flight_transactions: usize,
    },
    /// Queued transactions are batched into self-calls to an
    /// `ISafenet7702Executor` that the signer account delegates to via
    /// EIP-7702, with one transaction in flight at a time.
    Batched {
        /// The `ISafenet7702Executor` the signer account delegates to.
        executor: Address,
        /// The maximum gas a single batch may consume. A transaction that does
        /// not fit on its own gets a one-call batch, so `0` sends every
        /// transaction through the executor without batching.
        max_batch_gas: u64,
    },
}

impl SubmissionMode {
    /// The maximum number of transactions that may be in flight at any one
    /// time.
    pub fn max_in_flight_transactions(&self) -> usize {
        match self {
            Self::Direct {
                max_in_flight_transactions,
            } => *max_in_flight_transactions,
            // EIP-7702 mempools accept only one pending transaction from a
            // delegated account.
            Self::Batched { .. } => 1,
        }
    }
}

/// The flat `[transactions]` table, before grouping the submission mode.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    max_in_flight_transactions: Option<usize>,
    executor: Option<Address>,
    max_batch_gas: Option<u64>,
    blocks_before_resubmit: u64,
    priority_fee_cap_percentage: Option<f64>,
}

impl Default for RawConfig {
    fn default() -> Self {
        let config = Config::default();
        Self {
            max_in_flight_transactions: None,
            executor: None,
            max_batch_gas: None,
            blocks_before_resubmit: config.blocks_before_resubmit,
            priority_fee_cap_percentage: config.priority_fee_cap_percentage,
        }
    }
}

impl TryFrom<RawConfig> for Config {
    type Error = &'static str;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        let mode = match (
            raw.max_in_flight_transactions,
            raw.executor,
            raw.max_batch_gas,
        ) {
            (max_in_flight_transactions, None, None) => SubmissionMode::Direct {
                max_in_flight_transactions: max_in_flight_transactions
                    .unwrap_or(DEFAULT_MAX_IN_FLIGHT_TRANSACTIONS),
            },
            (_, None, Some(_)) => return Err("`max_batch_gas` requires `executor`"),
            (Some(_), Some(_), _) => {
                return Err("`max_in_flight_transactions` cannot be combined with `executor`");
            }
            (None, Some(executor), max_batch_gas) => SubmissionMode::Batched {
                executor,
                max_batch_gas: max_batch_gas.unwrap_or(DEFAULT_MAX_BATCH_GAS),
            },
        };
        Ok(Self {
            mode,
            blocks_before_resubmit: raw.blocks_before_resubmit,
            priority_fee_cap_percentage: raw.priority_fee_cap_percentage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
    use serde_json::json;

    #[test]
    fn deserializes_submission_mode() {
        let executor = address!("0x0404040404040404040404040404040404040404");
        for (table, mode) in [
            (
                json!({}),
                SubmissionMode::Direct {
                    max_in_flight_transactions: 16,
                },
            ),
            (
                json!({ "max_in_flight_transactions": 4 }),
                SubmissionMode::Direct {
                    max_in_flight_transactions: 4,
                },
            ),
            (
                json!({ "executor": executor }),
                SubmissionMode::Batched {
                    executor,
                    max_batch_gas: 2_000_000,
                },
            ),
            (
                json!({ "executor": executor, "max_batch_gas": 1_000_000 }),
                SubmissionMode::Batched {
                    executor,
                    max_batch_gas: 1_000_000,
                },
            ),
        ] {
            let config = serde_json::from_value::<Config>(table).unwrap();
            assert_eq!(config.mode, mode);
        }
    }

    #[test]
    fn rejects_invalid_submission_mode() {
        let executor = address!("0x0404040404040404040404040404040404040404");
        for table in [
            json!({ "max_batch_gas": 1_000_000 }),
            json!({ "max_in_flight_transactions": 4, "executor": executor }),
            json!({ "executor": "0x04" }),
            json!({ "unknown": 1 }),
        ] {
            assert!(
                serde_json::from_value::<Config>(table.clone()).is_err(),
                "{table}"
            );
        }
    }
}
