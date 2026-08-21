//! `sol!`-generated bindings for the contract interfaces the checks decode
//! calldata against.

pub mod multi_send {
    alloy::sol! {
        function multiSend(bytes transactions);
    }
}
