//! `sol!` bindings for the calldata a check decodes.
//!
//! Unlike [`crate::types`], these are pure ABI: `sol!` is exactly the right
//! tool, since the encoding is the contract's to define and not ours.

pub mod erc20 {
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

pub mod multi_send {
    alloy::sol! {
        function multiSend(bytes transactions);
    }
}
