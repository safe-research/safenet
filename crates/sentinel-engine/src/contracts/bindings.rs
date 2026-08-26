//! Alloy bindings for contracts whose calls and events the engine inspects.

pub mod safe {
    alloy::sol! {
        function addOwnerWithThreshold(address owner, uint256 threshold);
        function removeOwner(address prevOwner, address owner, uint256 threshold);
        function swapOwner(address prevOwner, address oldOwner, address newOwner);
        function changeThreshold(uint256 threshold);
        function setFallbackHandler(address handler);
        function setGuard(address guard);
        function enableModule(address module);
        function disableModule(address prevModule, address module);
        function setModuleGuard(address guard);
        function migrateSingleton();
        function migrateWithFallbackHandler();
        function migrateL2Singleton();
        function migrateL2WithFallbackHandler();
        function signMessage(bytes message);
        function multiSend(bytes transactions);
        function performCreate(uint256 value, bytes deploymentData);
        function performCreate2(uint256 value, bytes deploymentData, bytes32 salt);
    }
}

pub mod safenet_guard {
    alloy::sol! {
        /// Mirrors `TransactionAnnouncement.AnnouncedTransaction`. Only the
        /// function selector matters to the escape-hatch check, so the
        /// struct is declared purely to get that selector right.
        struct AnnouncedTransaction {
            address to;
            uint256 value;
            bytes data;
            uint8 operation;
            uint256 safeTxGas;
            uint256 baseGas;
            uint256 gasPrice;
            address gasToken;
            address refundReceiver;
        }

        function announceTransaction(AnnouncedTransaction announcement);
        function cancelAnnouncement(bytes32 announcementHash);
    }
}

pub mod erc20 {
    alloy::sol! {
        function transfer(address to, uint256 amount);
        function transferFrom(address from, address to, uint256 amount);
        function approve(address spender, uint256 amount);

        event Transfer(address indexed from, address indexed to, uint256 amount);
        event Approval(address indexed owner, address indexed spender, uint256 amount);
    }
}

pub mod erc721 {
    // `safeTransferFrom` is overloaded (3- and 4-arg); `sol!` disambiguates
    // same-named items by appending `_{index}` in declaration order, so
    // these generate `safeTransferFrom_0Call`/`safeTransferFrom_1Call`.
    alloy::sol! {
        function setApprovalForAll(address operator, bool approved);
        function safeTransferFrom(address from, address to, uint256 tokenId);
        function safeTransferFrom(address from, address to, uint256 tokenId, bytes data);
    }
}

pub mod erc1155 {
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

pub mod cow {
    alloy::sol! {
        function setPreSignature(bytes orderUid, bool signed);

        struct ConditionalOrderParams {
            address handler;
            bytes32 salt;
            bytes staticInput;
        }
        /// The Safe app's actual order-creation call — unlike bare `create`,
        /// this resolves `factory`'s value at creation time and substitutes
        /// it into `params.staticInput` in place of a context-dependent
        /// field (for the TWAP handler, `t0`). This is how a TWAP order
        /// created "starting now" is expressed on-chain: `staticInput`'s own
        /// `t0` is a placeholder, and `factory` — CoW's
        /// `CurrentBlockTimestampFactory` for a genuine one — supplies the
        /// real start time instead.
        function createWithContext(ConditionalOrderParams params, address factory, bytes data, bool dispatch);

        /// ComposableCoW's TWAP handler's own order shape, ABI-encoded as
        /// `create`'s `staticInput`. `sellToken` must match the token the
        /// batched `approve` is actually for — an approval on an unrelated
        /// token authorizes an allowance the order doesn't need at all, which is
        /// itself excessive regardless of amount. `receiver` must be the Safe
        /// itself (or the zero address, CoW's convention for "defaults to the
        /// order owner") — anything else would route the order's proceeds to an
        /// unrelated address. `partSellAmount * n` is the order's total sell
        /// amount, the ceiling the approval must not exceed. The remaining
        /// fields aren't needed by this check.
        struct TwapData {
            address sellToken;
            address buyToken;
            address receiver;
            uint256 partSellAmount;
            uint256 minPartLimit;
            uint256 t0;
            uint256 n;
            uint256 t;
            uint256 span;
            bytes32 appData;
        }

        /// GPv2Settlement's own order struct (`GPv2Order.Data`), declared here
        /// only for its EIP-712 hash — `kind`/`sellTokenBalance`/
        /// `buyTokenBalance` are `string` (not `bytes32`) in the *type
        /// signature* on purpose: GPv2Order.sol stores them pre-hashed as
        /// `bytes32` markers (`keccak256("sell")` etc.), but EIP-712 hashes a
        /// dynamic `string` field's *content* the same way, so passing the
        /// literal strings here and letting this type's derived hashing do that
        /// produces the identical digest.
        struct Order {
            address sellToken;
            address buyToken;
            address receiver;
            uint256 sellAmount;
            uint256 buyAmount;
            uint32 validTo;
            bytes32 appData;
            uint256 feeAmount;
            string kind;
            bool partiallyFillable;
            string sellTokenBalance;
            string buyTokenBalance;
        }
    }
}
