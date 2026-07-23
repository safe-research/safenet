//! `sol!`-generated Safe transaction types, shared between `validator` and
//! `sentinel`.
//
// TODO: rename this module to `bindings.rs` on a later cleanup pass — it now
// holds all `sol!` bindings, not just the `SafeTransaction`/`Operation`
// types the current name implies.

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

pub(crate) mod erc20 {
    alloy::sol! {
        function transfer(address to, uint256 amount);
        function transferFrom(address from, address to, uint256 amount);
        function approve(address spender, uint256 amount);
    }
}

pub(crate) mod erc721 {
    // `safeTransferFrom` is overloaded (3- and 4-arg); `sol!` disambiguates
    // same-named items by appending `_{index}` in declaration order, so
    // these generate `safeTransferFrom_0Call`/`safeTransferFrom_1Call`.
    alloy::sol! {
        function setApprovalForAll(address operator, bool approved);
        function safeTransferFrom(address from, address to, uint256 tokenId);
        function safeTransferFrom(address from, address to, uint256 tokenId, bytes data);
    }
}

pub(crate) mod erc1155 {
    alloy::sol! {
        function safeTransferFrom(address from, address to, uint256 id, uint256 amount, bytes data);
        function safeBatchTransferFrom(address from, address to, uint256[] ids, uint256[] amounts, bytes data);
    }
}

pub(crate) mod multi_send_bindings {
    alloy::sol! {
        function multiSend(bytes transactions);
    }
}
