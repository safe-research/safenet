//! Sending the sentinel's transactions through a Zodiac Roles modifier, so that
//! a Safe can be the registered sentinel while the service only holds a
//! restricted role key.
//!
//! `SentinelOracle` identifies a sentinel by `msg.sender`: it pulls the bond
//! from it on `commit`, checks its commitment on `reveal`, and pays it on
//! `claim`. When the sentinel is a Safe, every one of these calls must come
//! from the Safe. The service's key is then a member of a Roles role scoped to
//! the sentinel's calls, and each transaction is sent to the modifier as
//! `execTransactionWithRole`, which checks it against the role and executes it
//! on the Safe.

use crate::bindings::{
    oracle::{ERC20, SentinelOracle},
    roles::Roles,
};
use alloy::{
    primitives::{Address, B256, U256},
    providers::Provider as _,
    sol_types::SolCall,
};
use safenet_core::{provider::Provider, tx::Transaction};
use serde::Deserialize;

/// Zodiac `Enum.Operation.Call`.
const CALL: u8 = 0;

/// The Zodiac Roles modifier the sentinel's transactions are sent through, and
/// the role the service's key acts with. The sentinel is the modifier's
/// `target`, read onchain at startup by [`RolesModule::sentinel_safe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RolesModule {
    /// The Roles modifier attached to the sentinel Safe.
    pub module: Address,
    /// The role the service's key is a member of.
    pub role_key: B256,
    /// Gas added to each wrapped transaction's own limit for the modifier's
    /// role check and the Safe's module execution, including any guard.
    #[serde(default = "RolesModule::default_gas_overhead")]
    pub gas_overhead: u64,
}

/// A Roles setup the sentinel can't run with.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the Roles modifier {0} has no code")]
    NoModule(Address),
    #[error("the Roles modifier {0} has no target Safe")]
    NoTarget(Address),
    #[error("the Safe {0} is not a registered sentinel on the oracle")]
    NotRegistered(Address),
    #[error(
        "the role {role_key} doesn't let {signer} make the Safe {safe} approve the oracle: {source}"
    )]
    RoleDenied {
        role_key: B256,
        signer: Address,
        safe: Address,
        source: Box<alloy::contract::Error>,
    },
    #[error(transparent)]
    Rpc(#[from] alloy::transports::TransportError),
    #[error(transparent)]
    Call(Box<alloy::contract::Error>),
}

impl From<alloy::contract::Error> for Error {
    fn from(err: alloy::contract::Error) -> Self {
        Self::Call(Box::new(err))
    }
}

impl RolesModule {
    /// About 2.5 times the largest overhead measured for a role scoped to the
    /// sentinel's calls on a Safe v1.5.0 (~58k, for the bond `approve`).
    const fn default_gas_overhead() -> u64 {
        150_000
    }

    /// Returns the Safe this role acts for, after checking that the sentinel
    /// can run as it: the modifier exists, it targets a Safe registered on the
    /// oracle, and `signer`'s role lets it make that Safe send a bond
    /// `approve` (simulated with a zero amount). A misconfiguration then fails
    /// here at startup, instead of later as a revert of every transaction.
    pub async fn sentinel_safe(
        &self,
        provider: &Provider,
        signer: Address,
        oracle: Address,
        fee_token: Address,
    ) -> Result<Address, Error> {
        if provider.get_code_at(self.module).await?.is_empty() {
            return Err(Error::NoModule(self.module));
        }
        let roles = Roles::new(self.module, provider);
        let safe = roles.target().call().await?;
        if safe.is_zero() {
            return Err(Error::NoTarget(self.module));
        }
        let active_at = SentinelOracle::new(oracle, provider)
            .sentinelActiveAt(safe)
            .call()
            .await?;
        if active_at.is_zero() {
            return Err(Error::NotRegistered(safe));
        }
        let approve = ERC20::approveCall {
            spender: oracle,
            amount: U256::ZERO,
        };
        roles
            .execTransactionWithRole(
                fee_token,
                U256::ZERO,
                approve.abi_encode().into(),
                CALL,
                self.role_key,
                true,
            )
            .from(signer)
            .call()
            .await
            .map_err(|source| Error::RoleDenied {
                role_key: self.role_key,
                signer,
                safe,
                source: Box::new(source),
            })?;
        Ok(safe)
    }

    /// Wraps `transaction` into the `execTransactionWithRole` call that makes
    /// the Safe send it. `shouldRevert` is set, so a call the role denies or
    /// that fails reverts the whole transaction, as the unwrapped call would.
    pub fn wrap(&self, transaction: Transaction) -> Transaction {
        let Transaction {
            to,
            value,
            data,
            gas,
        } = transaction;
        Transaction {
            to: self.module,
            value: U256::ZERO,
            data: Roles::execTransactionWithRoleCall {
                to,
                value,
                data,
                operation: CALL,
                roleKey: self.role_key,
                shouldRevert: true,
            }
            .abi_encode()
            .into(),
            gas: gas.saturating_add(self.gas_overhead),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{Bytes, address, b256, bytes},
        sol_types::SolValue,
        transports::mock::Asserter,
    };

    const MODULE: Address = address!("6666666666666666666666666666666666666666");
    const ROLE_KEY: B256 =
        b256!("0x7777777777777777777777777777777777777777777777777777777777777777");
    const SAFE: Address = address!("9999999999999999999999999999999999999999");
    const SIGNER: Address = address!("1111111111111111111111111111111111111111");
    const ORACLE: Address = address!("2222222222222222222222222222222222222222");
    const FEE_TOKEN: Address = address!("3333333333333333333333333333333333333333");

    const ROLES: RolesModule = RolesModule {
        module: MODULE,
        role_key: ROLE_KEY,
        gas_overhead: 150_000,
    };

    /// Queues the RPC answers of a setup that passes every check up to the
    /// one under test: the modifier's code, its `target`, and the Safe's
    /// `sentinelActiveAt`, in the order `sentinel_safe` asks for them.
    fn answer(asserter: &Asserter, target: Address, active_at: u64) {
        asserter.push_success(&bytes!("0x6080"));
        asserter.push_success(&Bytes::from(target.abi_encode()));
        asserter.push_success(&Bytes::from(U256::from(active_at).abi_encode()));
    }

    async fn sentinel_safe(asserter: &Asserter) -> Result<Address, Error> {
        ROLES
            .sentinel_safe(&Provider::mocked(asserter), SIGNER, ORACLE, FEE_TOKEN)
            .await
    }

    #[tokio::test]
    async fn resolves_the_safe_when_the_role_can_act_for_it() {
        let asserter = Asserter::new();
        answer(&asserter, SAFE, 1);
        asserter.push_success(&Bytes::from(true.abi_encode()));

        assert_eq!(sentinel_safe(&asserter).await.unwrap(), SAFE);
    }

    #[tokio::test]
    async fn rejects_a_module_without_code() {
        let asserter = Asserter::new();
        asserter.push_success(&Bytes::new());

        assert!(matches!(
            sentinel_safe(&asserter).await,
            Err(Error::NoModule(MODULE))
        ));
    }

    #[tokio::test]
    async fn rejects_a_module_without_a_target() {
        let asserter = Asserter::new();
        answer(&asserter, Address::ZERO, 1);

        assert!(matches!(
            sentinel_safe(&asserter).await,
            Err(Error::NoTarget(MODULE))
        ));
    }

    #[tokio::test]
    async fn rejects_a_safe_that_is_not_a_registered_sentinel() {
        let asserter = Asserter::new();
        answer(&asserter, SAFE, 0);

        assert!(matches!(
            sentinel_safe(&asserter).await,
            Err(Error::NotRegistered(SAFE))
        ));
    }

    #[tokio::test]
    async fn rejects_a_role_that_denies_the_sentinel_calls() {
        let asserter = Asserter::new();
        answer(&asserter, SAFE, 1);
        asserter.push_failure_msg("execution reverted: NoMembership()");

        assert!(matches!(
            sentinel_safe(&asserter).await,
            Err(Error::RoleDenied { safe: SAFE, .. })
        ));
    }

    #[test]
    fn wraps_a_transaction_as_an_exec_transaction_with_role_call() {
        let inner = Transaction {
            to: address!("1111111111111111111111111111111111111111"),
            value: U256::ZERO,
            data: bytes!("0xdeadbeef"),
            gas: 250_000,
        };

        let wrapped = ROLES.wrap(inner.clone());

        assert_eq!(wrapped.to, MODULE);
        assert_eq!(wrapped.value, U256::ZERO);
        assert_eq!(wrapped.gas, 400_000);
        // `execTransactionWithRole(address,uint256,bytes,uint8,bytes32,bool)`.
        assert_eq!(wrapped.data[..4], [0xc6, 0xfe, 0x87, 0x47]);
        let call = Roles::execTransactionWithRoleCall::abi_decode(&wrapped.data).unwrap();
        assert_eq!(call.to, inner.to);
        assert_eq!(call.value, inner.value);
        assert_eq!(call.data, inner.data);
        assert_eq!(call.operation, CALL);
        assert_eq!(call.roleKey, ROLE_KEY);
        assert!(call.shouldRevert);
    }
}
