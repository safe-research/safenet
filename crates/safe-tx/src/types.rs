//! `sol!`-generated Safe transaction types, shared between `validator` and
//! `sentinel`.

use alloy::sol;
use serde::{Deserialize, Serialize};

sol! {
    /// Safe transaction operation type; mirrors `Enum.Operation` onchain.
    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    enum Operation {
        #[default]
        CALL,
        DELEGATECALL,
    }

    /// A full Safe transaction as carried by the `(Oracle)TransactionProposed`
    /// events (the 12-field `SafeTransaction.T` tuple).
    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct SafeTransaction {
        uint256 chainId;
        address safe;
        address to;
        uint256 value;
        bytes data;
        Operation operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }
}
