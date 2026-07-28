//! R-4.4 worked example (epic Phase 7a): CoW Protocol's own expected
//! transaction shape. A genuine CoW Swap interaction is never a standalone
//! ERC-20 `approve` — the Safe's own frontend always batches the `approve`
//! (to CoW's canonical `GPv2VaultRelayer`) together with the call that
//! actually commits the Safe to an order: a swap's `setPreSignature`
//! presignature call to `GPv2Settlement`, or a TWAP order's creation call
//! (`ComposableCoW.create`, `handler` set to the canonical TWAP contract).
//!
//! This registry (`GPv2VaultRelayer`/`GPv2Settlement`/`ComposableCoW`/TWAP
//! handler addresses) is deliberately sentinel-local, not part of
//! `safe-tx`: `safe-tx`'s own allow-lists are Safe-native protocol constants
//! shared by both `validator` and `sentinel`, whereas these are one
//! third-party dapp's addresses that only this one check needs today.
//!
//! **Denies, rather than exempts.** A standalone `approve` to
//! `GPv2VaultRelayer` — not co-batched with a recognized trigger call — is
//! denied under [`RuleId::R4_4AuthorizationTarget`], including when
//! [`crate::address_poisoning::AddressPoisoningChecker`] would otherwise have
//! approved the same target off a prior genuine interaction: this
//! protocol-specific rule runs ahead of, and overrides, that general
//! history-based bypass (`CowChecker` is ordered first in
//! `crate::effect::Handler`'s checker chain).
//!
//! **A batched TWAP order is checked against the order itself**
//! ([`CowChecker::check_twap_batch`], epic Phase 8b), deliberately scoped
//! narrow: only an exact 2-call batch — one `approve`, one TWAP `create`,
//! in either order — is recognized at all; anything else (different batch
//! size, two of the same kind) is `Unknown`. A TWAP order's sell token,
//! receiver, and total sell amount (`partSellAmount * n`) are decodable
//! directly from the `create` call's own `staticInput` — no RPC or offchain
//! call needed, since the order's terms are committed onchain at creation
//! time. Once recognized, the pair is denied under
//! [`RuleId::R4_4AuthorizationTarget`] if the order's receiver isn't the
//! Safe itself (the same address-poisoning-style target-manipulation
//! concern as a wrong `approve` spender), or under
//! [`RuleId::R4_5ExcessiveApproval`] if the approved token doesn't match the
//! order's sell token, or the approved amount *exceeds* the order's total —
//! the latter only guards against over-authorization (the security concern:
//! a compromised relayer could drain the excess); an approval too small to
//! fully fund the order is a trade-soundness concern, not a security one,
//! and doesn't affect the verdict. Otherwise, the batch is approved
//! outright.
//!
//! **A batched presignature order is checked the same way**
//! ([`CowChecker::check_presignature_batch`], epic Phase 8c): only an exact
//! 2-call batch — one `approve`, one `setPreSignature`, in either order — is
//! recognized. Unlike the TWAP order's own calldata, a presigned order's
//! terms are signed off-chain, so this fetches the referenced order from
//! CoW's public order-by-UID API and approves outright once its token/amount
//! exactly match what's approved *and* its proceeds go back to the Safe
//! itself. As with the TWAP check, a denial here splits by cause: a wrong
//! receiver is [`RuleId::R4_4AuthorizationTarget`] (target manipulation,
//! not an amount concern), while a token/amount mismatch is
//! [`RuleId::R4_5ExcessiveApproval`] — unlike the TWAP check, though, a Safe
//! `approve` sets an allowance rather than incrementing it, so both an
//! under-approval and an over-approval are denied here. An unreachable or
//! malformed API response is `Unknown`, not guessed at either way.

use crate::checker::{CheckOutcome, Checker};
use alloy::{
    primitives::{Address, B256, Bytes, U256, address},
    sol,
    sol_types::{Eip712Domain, SolCall, SolStruct, SolValue, eip712_domain},
};
use safe_tx::{
    multi_send::decode_multi_send_call,
    rule::RuleId,
    types::{Operation, SafeTransaction, erc20::approveCall},
};
use serde::Deserialize;

sol! {
    function setPreSignature(bytes orderUid, bool signed);

    struct ConditionalOrderParams {
        address handler;
        bytes32 salt;
        bytes staticInput;
    }
    function create(ConditionalOrderParams params, bool dispatch);

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

/// CoW Protocol's canonical contracts. Deployed at the same address on every
/// network CoW Swap supports (deterministic `CREATE2` deployment), including
/// all three networks the Charter scopes to (Article I).
const GP_V2_VAULT_RELAYER: Address = address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110");
const GP_V2_SETTLEMENT: Address = address!("9008D19f58AAbD9eD0D60971565AA8510560ab41");
const COMPOSABLE_COW: Address = address!("fdaFc9d1902f4e0b84f65F49f244b32b31013b74");
const TWAP_HANDLER: Address = address!("6cF1e9cA41f7611dEf408122793c358a3d11E5a5");

/// Chain IDs the above are recognized on: Ethereum mainnet, Arbitrum One,
/// Gnosis Chain.
const SUPPORTED_CHAIN_IDS: &[u64] = &[1, 42161, 100];

/// CoW's own public order-by-UID API host, one per network it's deployed on
/// — the same three chains recognized above, used only by
/// [`CowChecker::check_presignature_batch`] to resolve where to fetch an
/// order's details from.
fn order_api_base_url(chain_id: U256) -> Option<&'static str> {
    if chain_id == U256::from(1u64) {
        Some("https://api.cow.fi/mainnet")
    } else if chain_id == U256::from(100u64) {
        Some("https://api.cow.fi/xdai")
    } else if chain_id == U256::from(42161u64) {
        Some("https://api.cow.fi/arbitrum_one")
    } else {
        None
    }
}

/// GPv2Settlement's EIP-712 domain. `verifyingContract` is the same address
/// on every network it's deployed to (deterministic `CREATE2`), so only
/// `chain_id` varies. Chain ids of the networks CoW supports (see
/// `order_api_base_url`) are all well within `u64` range, which is all the
/// `eip712_domain!` macro accepts.
fn gp_v2_domain(chain_id: U256) -> Eip712Domain {
    let chain_id = u64::try_from(chain_id).expect("chain id expected to be in u64 range");
    eip712_domain! {
        name: "Gnosis Protocol",
        version: "v2",
        chain_id: chain_id,
        verifying_contract: GP_V2_SETTLEMENT,
    }
}

/// Recomputes `order`'s 56-byte `orderUid` (`orderDigest(32) ‖ owner(20) ‖
/// validTo(4)`, exactly how `GPv2Order.sol`'s `packOrderUidParams` and
/// EIP-712 `hash` combine) for `chain_id`'s domain — the cryptographic tie
/// between an order's terms and the specific UID a presignature commits to.
fn compute_order_uid(chain_id: U256, order: &CowOrder) -> [u8; 56] {
    let digest = Order {
        sellToken: order.sell_token,
        buyToken: order.buy_token,
        // The order's own on-chain `receiver` value — `address(0)` is
        // itself a real, meaningful field value here ("same as owner"),
        // not something to substitute away before hashing (unlike
        // `CowOrder::receiver()`, used for the *policy* check below).
        receiver: order.receiver.unwrap_or(Address::ZERO),
        sellAmount: order.sell_amount,
        buyAmount: order.buy_amount,
        validTo: order.valid_to,
        appData: order.app_data,
        feeAmount: order.fee_amount,
        kind: order.kind.clone(),
        partiallyFillable: order.partially_fillable,
        sellTokenBalance: order.sell_token_balance.clone(),
        buyTokenBalance: order.buy_token_balance.clone(),
    }
    .eip712_signing_hash(&gp_v2_domain(chain_id));

    let mut uid = [0u8; 56];
    uid[..32].copy_from_slice(digest.as_slice());
    uid[32..52].copy_from_slice(order.owner.as_slice());
    uid[52..].copy_from_slice(&order.valid_to.to_be_bytes());
    uid
}

/// Fetches a CoW order's details by UID. A thin seam between
/// [`CowChecker`]'s own logic and the actual HTTP call, so tests can supply
/// a fake instead of standing up a real server (see `FakeOrderApi` in this
/// module's tests).
#[async_trait::async_trait]
trait OrderApi: Send + Sync {
    async fn fetch_order(
        &self,
        base_url: &str,
        order_uid: &Bytes,
    ) -> Result<CowOrder, OrderLookupError>;
}

/// The real [`OrderApi`], backed by a `reqwest::Client`.
struct ReqwestOrderApi {
    client: reqwest::Client,
}

#[async_trait::async_trait]
impl OrderApi for ReqwestOrderApi {
    async fn fetch_order(
        &self,
        base_url: &str,
        order_uid: &Bytes,
    ) -> Result<CowOrder, OrderLookupError> {
        Ok(self
            .client
            .get(format!("{base_url}/api/v1/orders/{order_uid}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

/// Why an order lookup failed, flattened to a message rather than kept as a
/// `reqwest::Error` — that type has no public constructor, so a test fake
/// couldn't otherwise return one without a real request to produce it from.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct OrderLookupError(String);

impl From<reqwest::Error> for OrderLookupError {
    fn from(err: reqwest::Error) -> Self {
        Self(err.to_string())
    }
}

/// The built-in CoW Swap check (see module docs). Delegates the actual order
/// lookup ([`CowChecker::check_presignature_batch`]'s own need) to an
/// [`OrderApi`] — real HTTP via [`ReqwestOrderApi`] in production, a fake in
/// tests.
pub struct CowChecker {
    order_api: Box<dyn OrderApi>,
}

impl CowChecker {
    pub fn new() -> Self {
        Self::with_client(reqwest::Client::new())
    }

    /// Backed by `client` rather than a freshly-constructed one, so a caller
    /// can share a single `reqwest::Client` across checkers.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            order_api: Box::new(ReqwestOrderApi { client }),
        }
    }

    #[cfg(test)]
    fn with_order_api(order_api: impl OrderApi + 'static) -> Self {
        Self {
            order_api: Box::new(order_api),
        }
    }

    /// Recognizes a presignature-flow batch — *exactly* one ERC-20 `approve`
    /// to `GPv2VaultRelayer` paired with one presignature call for the same
    /// order, nothing else, the only shape a genuine CoW Swap presignature
    /// flow takes (a Safe `approve` sets an allowance rather than
    /// incrementing it, so anything looser isn't the expected pattern) —
    /// fetches that order from CoW's own API, and approves outright once its
    /// token/amount exactly match what's approved *and* its proceeds go back
    /// to the Safe itself: this narrow a match is confident enough evidence
    /// of a genuine CoW Swap to decide the vote, not merely defer to the
    /// next checker. If it denies, it denies under
    /// [`RuleId::R4_4AuthorizationTarget`] when the order's receiver isn't
    /// `safe` itself (an address-poisoning-style target-manipulation
    /// concern, the same split [`CowChecker::check_twap_batch`] makes), or
    /// under [`RuleId::R4_5ExcessiveApproval`] when the token/amount don't
    /// match instead.
    ///
    /// CoW's API isn't authenticated, so its response is never trusted at
    /// face value: [`compute_order_uid`] recomputes the order's own EIP-712
    /// digest from the fetched fields and checks it actually matches the
    /// `orderUid` we looked it up by — the same binding the presignature
    /// itself relies on — before any of its fields are used for a decision.
    ///
    /// Returns [`CheckOutcome::Unknown`] if `chain_id` isn't recognized, if
    /// `calls` isn't that exact shape at all, if the lookup fails or is
    /// malformed, or if the response's recomputed digest doesn't match the
    /// requested `orderUid`: none of these are treated as approval or
    /// denial, consistent with this epic's established pattern for external
    /// lookups.
    async fn check_presignature_batch(
        &self,
        safe: Address,
        chain_id: U256,
        calls: &[SafeTransaction],
    ) -> CheckOutcome {
        let Some(base_url) = order_api_base_url(chain_id) else {
            return CheckOutcome::Unknown;
        };
        let [first, second] = calls else {
            return CheckOutcome::Unknown;
        };
        let Some((token, approved_amount, order_uid)) =
            decode_approval_and_presignature(first, second)
                .or_else(|| decode_approval_and_presignature(second, first))
        else {
            return CheckOutcome::Unknown;
        };

        match self.order_api.fetch_order(base_url, &order_uid).await {
            Ok(order) if order_uid.as_ref() != &compute_order_uid(chain_id, &order)[..] => {
                tracing::error!(
                    %order_uid,
                    "CoW order response's recomputed digest didn't match the requested \
                     order UID; no opinion on the batched approval"
                );
                CheckOutcome::Unknown
            }
            // A wrong receiver is an address-poisoning-style
            // target-manipulation concern (R-4.4), distinct from the
            // excessive-approval-amount concern (R-4.5) the token/amount
            // check below guards against — see `check_twap_batch` for the
            // same split.
            Ok(order) if order.receiver() != safe => {
                CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
            }
            Ok(order) if token == order.sell_token && approved_amount == order.sell_amount => {
                CheckOutcome::Approved
            }
            Ok(_) => CheckOutcome::Denied(RuleId::R4_5ExcessiveApproval),
            Err(err) => {
                tracing::error!(
                    %err,
                    %order_uid,
                    "CoW order lookup failed; no opinion on the batched approval"
                );
                CheckOutcome::Unknown
            }
        }
    }

    /// Deliberately narrow: recognizes only an exact 2-call batch, one
    /// `approve` and one TWAP `create`, in either order — anything else
    /// (different batch size, two of the same kind, neither) is `Unknown`,
    /// not guessed at. A TWAP order's sell token, receiver, and total sell
    /// amount (`partSellAmount * n`) are decodable directly from its
    /// `create` calldata — no RPC or offchain call needed, since the
    /// order's terms are committed onchain at creation time (epic Phase
    /// 8b). Once the pair is recognized, this check reaches a conclusive
    /// verdict rather than falling through: it denies under
    /// [`RuleId::R4_4AuthorizationTarget`] if the order's `receiver` isn't
    /// `safe` itself (the same address-poisoning-style target-manipulation
    /// concern as a wrong `approve` spender — proceeds routed to an
    /// unrelated address), or under [`RuleId::R4_5ExcessiveApproval`] if the
    /// approved token doesn't match the order's `sellToken` (an allowance
    /// the order doesn't need at all, itself excessive) or the approved
    /// amount exceeds the order's total; otherwise it approves outright. An
    /// approval *smaller* than that total is a trade-soundness concern (the
    /// order may not fully fill), not a security one, so it doesn't affect
    /// this verdict either way.
    async fn check_twap_batch(&self, safe: Address, calls: &[SafeTransaction]) -> CheckOutcome {
        let [first, second] = calls else {
            return CheckOutcome::Unknown;
        };

        // Order terms undecodable (malformed `staticInput`) means no
        // opinion, not a guessed denial — consistent with this epic's other
        // external/undecodable-data lookups (e.g. `address_poisoning`'s
        // no-history case).
        let Some((approved_token, approved_amount, sell_token, receiver, total_sell_amount)) =
            decode_approval_and_twap(first, second)
                .or_else(|| decode_approval_and_twap(second, first))
        else {
            return CheckOutcome::Unknown;
        };

        // A wrong receiver is an address-poisoning-style target-manipulation
        // concern (R-4.4), distinct from the excessive-approval-amount
        // concern (R-4.5) the token/amount checks below guard against.
        if receiver != safe && !receiver.is_zero() {
            return CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget);
        }
        if approved_token != sell_token || approved_amount > total_sell_amount {
            return CheckOutcome::Denied(RuleId::R4_5ExcessiveApproval);
        }
        CheckOutcome::Approved
    }

    /// An `approve` to `GPv2VaultRelayer` with no co-batched presignature or
    /// TWAP order-creation call is not the pattern a genuine CoW Swap
    /// interaction takes.
    async fn check_dangling_approval(&self, calls: &[SafeTransaction]) -> CheckOutcome {
        if !calls.iter().any(approves_vault_relayer) {
            return CheckOutcome::Unknown;
        }
        if calls
            .iter()
            .any(|c| is_presignature(c) || is_twap_create(c))
        {
            CheckOutcome::Unknown
        } else {
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        }
    }
}

impl Default for CowChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Checker for CowChecker {
    /// Runs, in order, [`CowChecker::check_dangling_approval`],
    /// [`CowChecker::check_presignature_batch`] and
    /// [`CowChecker::check_twap_batch`] against `transaction`'s sub-calls,
    /// returning the first non-[`CheckOutcome::Unknown`] result.
    async fn check(&self, safe: Address, transaction: &SafeTransaction) -> CheckOutcome {
        if !SUPPORTED_CHAIN_IDS
            .iter()
            .any(|&id| transaction.chainId == U256::from(id))
        {
            return CheckOutcome::Unknown;
        }
        let calls = sub_transactions(transaction);

        let dangling_check = self.check_dangling_approval(&calls).await;
        if dangling_check != CheckOutcome::Unknown {
            return dangling_check;
        }

        let presig_check = self
            .check_presignature_batch(safe, transaction.chainId, &calls)
            .await;
        if presig_check != CheckOutcome::Unknown {
            return presig_check;
        }

        let twap_check = self.check_twap_batch(safe, &calls).await;
        if twap_check != CheckOutcome::Unknown {
            return twap_check;
        }

        CheckOutcome::Unknown
    }
}

/// `tx` itself, or, if it's a MultiSend batch, each of its sub-calls.
fn sub_transactions(tx: &SafeTransaction) -> Vec<SafeTransaction> {
    decode_multi_send_call(tx)
        .map(|(sub_txs, _)| sub_txs)
        .unwrap_or_else(|| vec![tx.clone()])
}

/// Only a plain, valueless `CALL` is recognized — a `DELEGATECALL` executes
/// `tx.to`'s code in the Safe's own storage context rather than a real
/// ERC-20 `approve`, so calldata that merely matches the selector must not
/// be treated as one; and a genuine `approve` never carries native value, so
/// a nonzero `tx.value` is itself a sign this isn't the expected call.
/// Returns the approved token (`tx.to`, the contract `approve` is called
/// on) and amount, needed by [`CowChecker::check_twap_batch`]'s
/// amount-overlap check.
fn vault_relayer_approval_amount(tx: &SafeTransaction) -> Option<(Address, U256)> {
    if tx.operation != Operation::CALL || !tx.value.is_zero() {
        return None;
    }
    let call = approveCall::abi_decode(&tx.data).ok()?;
    (call.spender == GP_V2_VAULT_RELAYER).then_some((tx.to, call.amount))
}

/// Whether `tx` is an ERC-20 `approve` to `GPv2VaultRelayer`, for
/// [`CowChecker::check_dangling_approval`] — see
/// [`vault_relayer_approval_amount`] for the exact recognition rules.
fn approves_vault_relayer(tx: &SafeTransaction) -> bool {
    vault_relayer_approval_amount(tx).is_some()
}

/// Only a plain, valueless `CALL` to `GPv2Settlement` is recognized — see
/// [`vault_relayer_approval_amount`] for why `DELEGATECALL` and nonzero `tx.value`
/// are excluded even to a legitimate address.
fn is_presignature(tx: &SafeTransaction) -> bool {
    tx.operation == Operation::CALL
        && tx.value.is_zero()
        && tx.to == GP_V2_SETTLEMENT
        && setPreSignatureCall::abi_decode(&tx.data).is_ok()
}

/// Only a plain, valueless `CALL` to `ComposableCoW` is recognized — see
/// [`vault_relayer_approval_amount`] for why `DELEGATECALL` and nonzero `tx.value`
/// are excluded even to a legitimate address.
fn is_twap_create(tx: &SafeTransaction) -> bool {
    tx.operation == Operation::CALL
        && tx.value.is_zero()
        && tx.to == COMPOSABLE_COW
        && createCall::abi_decode(&tx.data).is_ok_and(|call| call.params.handler == TWAP_HANDLER)
}

/// Recognizes `approval_candidate` as a `GPv2VaultRelayer` approval and
/// `twap_candidate` as a TWAP `create` call, returning the approved token,
/// approved amount, and the TWAP order's own sell token, receiver, and total
/// sell amount. `None` if either candidate doesn't match its expected role —
/// including a matching pair given in the wrong order, which the caller
/// (`CowChecker::check_twap_batch`) handles by trying both orderings.
fn decode_approval_and_twap<'a>(
    approval_candidate: &'a SafeTransaction,
    twap_candidate: &'a SafeTransaction,
) -> Option<(Address, U256, Address, Address, U256)> {
    let (approved_token, approved_amount) = vault_relayer_approval_amount(approval_candidate)?;
    let (sell_token, receiver, total_sell_amount) = twap_order_terms(twap_candidate)?;
    Some((
        approved_token,
        approved_amount,
        sell_token,
        receiver,
        total_sell_amount,
    ))
}

/// Recognizes `tx` as a TWAP `create` call (same rules as [`is_twap_create`])
/// and, if so, decodes its sell token, receiver, and total sell amount
/// (`partSellAmount * n`) from `staticInput`. `None` if `tx` isn't a
/// recognized TWAP `create` call, `staticInput` isn't shaped as expected, or
/// the multiplication overflows (implausible in practice, but left
/// unguessed rather than wrapping) — either way the caller treats that as
/// inconclusive, not denied.
fn twap_order_terms(tx: &SafeTransaction) -> Option<(Address, Address, U256)> {
    if tx.operation != Operation::CALL || !tx.value.is_zero() || tx.to != COMPOSABLE_COW {
        return None;
    };
    let create = createCall::abi_decode(&tx.data).ok()?;
    if create.params.handler != TWAP_HANDLER {
        return None;
    };
    let order = TwapData::abi_decode(&create.params.staticInput).ok()?;
    let total = order.partSellAmount.checked_mul(order.n)?;
    Some((order.sellToken, order.receiver, total))
}

/// The referenced order's UID, if `tx` is a plain, valueless `CALL` to
/// `GPv2Settlement` presigning it (`signed: true` — an *unset* presignature
/// doesn't commit the Safe to the order). See [`approves_vault_relayer`] for
/// why `DELEGATECALL` and nonzero `tx.value` are excluded even to a
/// legitimate address.
fn presignature_order_uid(tx: &SafeTransaction) -> Option<Bytes> {
    if tx.operation != Operation::CALL || !tx.value.is_zero() || tx.to != GP_V2_SETTLEMENT {
        return None;
    }
    setPreSignatureCall::abi_decode(&tx.data)
        .ok()
        .filter(|call| call.signed)
        .map(|call| call.orderUid)
}

/// `approval`'s approved token/amount and `presignature`'s referenced order
/// UID, if both decode as expected — `None` if either doesn't. Used to try
/// both orderings of a two-call batch without repeating the pairing logic
/// (see [`CowChecker::check_presignature_batch`]).
fn decode_approval_and_presignature(
    approval: &SafeTransaction,
    presignature: &SafeTransaction,
) -> Option<(Address, U256, Bytes)> {
    let (token, approved_amount) = vault_relayer_approval_amount(approval)?;
    let order_uid = presignature_order_uid(presignature)?;
    Some((token, approved_amount, order_uid))
}

/// The full set of `GPv2Order.Data` fields — every one of them feeds
/// [`compute_order_uid`], so a response missing or misdecoding any of them
/// would (safely) just fail to reproduce the real digest, not silently skip
/// verification. `owner` is who placed the order — the Safe itself, for a
/// genuine presignature order — and `receiver` is who the bought tokens go
/// to, falling back to `owner` when unset (`null`) or zero, exactly as
/// CoW's own contracts define it (see [`CowOrder::receiver`]).
#[derive(Deserialize, Clone)]
struct CowOrder {
    owner: Address,
    receiver: Option<Address>,
    #[serde(rename = "sellToken")]
    sell_token: Address,
    #[serde(rename = "buyToken")]
    buy_token: Address,
    #[serde(rename = "sellAmount", deserialize_with = "deserialize_decimal_u256")]
    sell_amount: U256,
    #[serde(rename = "buyAmount", deserialize_with = "deserialize_decimal_u256")]
    buy_amount: U256,
    #[serde(rename = "validTo")]
    valid_to: u32,
    #[serde(rename = "appData")]
    app_data: B256,
    #[serde(rename = "feeAmount", deserialize_with = "deserialize_decimal_u256")]
    fee_amount: U256,
    kind: String,
    #[serde(rename = "partiallyFillable")]
    partially_fillable: bool,
    #[serde(rename = "sellTokenBalance")]
    sell_token_balance: String,
    #[serde(rename = "buyTokenBalance")]
    buy_token_balance: String,
}

impl CowOrder {
    /// The effective receiver of this order's proceeds. CoW treats *both*
    /// `null` and the zero address as "same as `owner`" — orders are
    /// commonly created with `receiver: 0x0` to mean exactly that, so
    /// `Some(Address::ZERO)` must fall back to `owner` too, not be taken
    /// literally as a real (zero) recipient.
    fn receiver(&self) -> Address {
        match self.receiver {
            Some(receiver) if !receiver.is_zero() => receiver,
            _ => self.owner,
        }
    }
}

/// CoW's API returns amounts as plain decimal strings, not the hex format
/// alloy's own `U256` (de)serialization expects.
fn deserialize_decimal_u256<'de, D: serde::Deserializer<'de>>(de: D) -> Result<U256, D::Error> {
    String::deserialize(de)?
        .parse()
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use safe_tx::types::multi_send_bindings;

    const SAFE: Address = Address::new([1u8; 20]);
    const TOKEN: Address = Address::new([2u8; 20]);
    const MULTI_SEND: Address = address!("218543288004CD07832472D464648173c77D7eB7");
    const ORDER_UID: [u8; 56] = [0xab; 56];

    /// A stand-in [`OrderApi`] returning a canned result, so these tests
    /// don't need a real HTTP server.
    #[allow(clippy::large_enum_variant)]
    enum FakeOrderApi {
        Found(CowOrder),
        NotFound,
    }

    #[async_trait::async_trait]
    impl OrderApi for FakeOrderApi {
        async fn fetch_order(
            &self,
            _base_url: &str,
            _order_uid: &Bytes,
        ) -> Result<CowOrder, OrderLookupError> {
            match self {
                Self::Found(order) => Ok(order.clone()),
                Self::NotFound => Err(OrderLookupError("order not found".into())),
            }
        }
    }

    fn order(sell_amount: u64) -> CowOrder {
        CowOrder {
            owner: SAFE,
            receiver: None,
            sell_token: TOKEN,
            buy_token: Address::new([3u8; 20]),
            sell_amount: U256::from(sell_amount),
            buy_amount: U256::from(1u64),
            valid_to: 1_700_000_000,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: "sell".to_string(),
            partially_fillable: false,
            sell_token_balance: "erc20".to_string(),
            buy_token_balance: "erc20".to_string(),
        }
    }

    /// `order`'s real `orderUid` on mainnet (chain id 1), computed the same
    /// way [`compute_order_uid`] does — so tests that supply a
    /// [`FakeOrderApi::Found`] order use a genuinely consistent
    /// (order, orderUid) pair, the same binding
    /// [`CowChecker::check_presignature_batch`] itself now verifies, rather
    /// than an arbitrary unrelated UID.
    fn order_uid_for(order: &CowOrder) -> Bytes {
        Bytes::from(compute_order_uid(U256::from(1u64), order).to_vec())
    }

    /// Runs the full [`Checker::check`] pipeline against `transaction`,
    /// backed by an [`OrderApi`] fake that never resolves a real order —
    /// exercising [`CowChecker::check_dangling_approval`]/
    /// [`CowChecker::check_twap_batch`] never needs a real lookup, so this
    /// keeps those tests network-free.
    async fn check(transaction: &SafeTransaction) -> CheckOutcome {
        CowChecker::with_order_api(FakeOrderApi::NotFound)
            .check(SAFE, transaction)
            .await
    }

    fn tx(to: Address, data: Vec<u8>, operation: Operation) -> SafeTransaction {
        SafeTransaction {
            chainId: U256::from(1u64),
            safe: SAFE,
            to,
            data: data.into(),
            operation,
            ..Default::default()
        }
    }

    fn approve_data(spender: Address) -> Vec<u8> {
        approve_data_with_amount(spender, U256::from(1u64))
    }

    fn approve_data_with_amount(spender: Address, amount: U256) -> Vec<u8> {
        approveCall { spender, amount }.abi_encode()
    }

    fn approve_amount_data(spender: Address, amount: U256) -> Vec<u8> {
        approveCall { spender, amount }.abi_encode()
    }

    fn twap_create_data(part_sell_amount: U256, n: U256) -> Vec<u8> {
        twap_create_data_full(TOKEN, SAFE, part_sell_amount, n)
    }

    fn twap_create_data_for_token(sell_token: Address, part_sell_amount: U256, n: U256) -> Vec<u8> {
        twap_create_data_full(sell_token, SAFE, part_sell_amount, n)
    }

    fn twap_create_data_for_receiver(
        receiver: Address,
        part_sell_amount: U256,
        n: U256,
    ) -> Vec<u8> {
        twap_create_data_full(TOKEN, receiver, part_sell_amount, n)
    }

    fn twap_create_data_full(
        sell_token: Address,
        receiver: Address,
        part_sell_amount: U256,
        n: U256,
    ) -> Vec<u8> {
        createCall {
            params: ConditionalOrderParams {
                handler: TWAP_HANDLER,
                salt: B256::ZERO,
                staticInput: Bytes::from(
                    TwapData {
                        sellToken: sell_token,
                        buyToken: Address::new([3u8; 20]),
                        receiver,
                        partSellAmount: part_sell_amount,
                        minPartLimit: U256::ZERO,
                        t0: U256::ZERO,
                        n,
                        t: U256::ZERO,
                        span: U256::ZERO,
                        appData: B256::ZERO,
                    }
                    .abi_encode(),
                ),
            },
            dispatch: true,
        }
        .abi_encode()
    }

    fn pack(operation: Operation, to: Address, data: &[u8]) -> Vec<u8> {
        pack_with_value(operation, to, U256::ZERO, data)
    }

    fn pack_with_value(operation: Operation, to: Address, value: U256, data: &[u8]) -> Vec<u8> {
        let mut out = vec![operation as u8];
        out.extend_from_slice(to.as_slice());
        out.extend_from_slice(&value.to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(data.len()).to_be_bytes::<32>());
        out.extend_from_slice(data);
        out
    }

    fn multisend(sub_txs: &[Vec<u8>]) -> Bytes {
        let transactions: Vec<u8> = sub_txs.iter().flatten().cloned().collect();
        Bytes::from(
            multi_send_bindings::multiSendCall {
                transactions: Bytes::from(transactions),
            }
            .abi_encode(),
        )
    }

    /// A batch approving `GPv2VaultRelayer` for `approved_amount`, co-batched
    /// with a presignature call for `order_uid`.
    fn batched_presig_tx(approved_amount: U256, order_uid: Bytes) -> SafeTransaction {
        let approve = approve_data_with_amount(GP_V2_VAULT_RELAYER, approved_amount);
        let presig = setPreSignatureCall {
            orderUid: order_uid,
            signed: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, GP_V2_SETTLEMENT, &presig),
        ]);
        tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)
    }

    #[tokio::test]
    async fn no_opinion_when_no_relayer_approval_is_present() {
        let data = approve_data(Address::new([9u8; 20]));
        assert_eq!(
            check(&tx(TOKEN, data, Operation::CALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn denies_a_standalone_approval_to_the_relayer() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        assert_eq!(
            check(&tx(TOKEN, data, Operation::CALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn no_opinion_when_approval_is_batched_with_a_presignature_call() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let presig = setPreSignatureCall {
            orderUid: Bytes::from(vec![0xab; 56]),
            signed: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, GP_V2_SETTLEMENT, &presig),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn approves_when_the_approval_exactly_covers_the_twap_order() {
        let approve = approve_amount_data(GP_V2_VAULT_RELAYER, U256::from(30u64));
        let create = twap_create_data(U256::from(10u64), U256::from(3u64));
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn denies_when_the_approval_exceeds_the_twap_order() {
        let approve = approve_amount_data(GP_V2_VAULT_RELAYER, U256::from(1_000u64));
        let create = twap_create_data(U256::from(10u64), U256::from(3u64));
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_5ExcessiveApproval)
        );
    }

    #[tokio::test]
    async fn denies_when_the_approval_is_for_a_different_token_than_the_twap_order_sells() {
        // Amount alone is within the order's cap, but the approve is on a
        // different token than the order's own `sellToken` — an allowance
        // this order doesn't need at all, which is itself excessive.
        let approve = approve_amount_data(GP_V2_VAULT_RELAYER, U256::from(30u64));
        let create = twap_create_data_for_token(
            Address::new([9u8; 20]),
            U256::from(10u64),
            U256::from(3u64),
        );
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_5ExcessiveApproval)
        );
    }

    #[tokio::test]
    async fn approves_when_the_approval_is_smaller_than_the_twap_order() {
        // Under-authorization is a trade-soundness concern (the order may
        // not fully fill), not a security one — this check only guards
        // against excessive authorization.
        let approve = approve_amount_data(GP_V2_VAULT_RELAYER, U256::from(29u64));
        let create = twap_create_data(U256::from(10u64), U256::from(3u64));
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn approves_when_the_twap_orders_receiver_is_zero_address() {
        let approve = approve_amount_data(GP_V2_VAULT_RELAYER, U256::from(30u64));
        let create = twap_create_data_for_receiver(
            Address::new([0u8; 20]),
            U256::from(10u64),
            U256::from(3u64),
        );
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn denies_when_the_twap_orders_receiver_is_not_the_safe() {
        let approve = approve_amount_data(GP_V2_VAULT_RELAYER, U256::from(30u64));
        let create = twap_create_data_for_receiver(
            Address::new([9u8; 20]),
            U256::from(10u64),
            U256::from(3u64),
        );
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn no_opinion_when_the_twap_batch_has_more_than_two_calls() {
        // Deliberately narrow: only an exact 2-call batch is recognized.
        let approve = approve_amount_data(GP_V2_VAULT_RELAYER, U256::from(30u64));
        let create = twap_create_data(U256::from(10u64), U256::from(3u64));
        let extra = approve_data(Address::new([9u8; 20]));
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
            pack(Operation::CALL, TOKEN, &extra),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn no_opinion_when_the_batch_pairs_an_unrelated_approval_with_a_twap_create_call() {
        let unrelated_approve = approve_data(Address::new([9u8; 20]));
        let create = twap_create_data(U256::from(10u64), U256::from(3u64));
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &unrelated_approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn denies_a_batched_approval_without_a_recognized_trigger() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let data = multisend(&[pack(Operation::CALL, TOKEN, &approve)]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn denies_when_batch_targets_an_unrecognized_create_handler() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let create = createCall {
            params: ConditionalOrderParams {
                handler: Address::new([7u8; 20]),
                salt: B256::ZERO,
                staticInput: Bytes::new(),
            },
            dispatch: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::CALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    // A `DELEGATECALL` executes `tx.to`'s code in the Safe's own storage
    // context rather than a real ERC-20 `approve`, so calldata that merely
    // matches the selector must not be treated as one.
    #[tokio::test]
    async fn no_opinion_when_the_approval_itself_is_a_delegatecall() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        assert_eq!(
            check(&tx(TOKEN, data, Operation::DELEGATECALL)).await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn denies_when_the_presignature_trigger_is_a_delegatecall() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let presig = setPreSignatureCall {
            orderUid: Bytes::from(vec![0xab; 56]),
            signed: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::DELEGATECALL, GP_V2_SETTLEMENT, &presig),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn denies_when_the_twap_create_trigger_is_a_delegatecall() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let create = createCall {
            params: ConditionalOrderParams {
                handler: TWAP_HANDLER,
                salt: B256::ZERO,
                staticInput: Bytes::new(),
            },
            dispatch: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack(Operation::DELEGATECALL, COMPOSABLE_COW, &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    // A genuine `approve` never carries native value.
    #[tokio::test]
    async fn no_opinion_when_the_approval_carries_native_value() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        let transaction = SafeTransaction {
            value: U256::from(1u64),
            ..tx(TOKEN, data, Operation::CALL)
        };
        assert_eq!(check(&transaction).await, CheckOutcome::Unknown);
    }

    #[tokio::test]
    async fn denies_when_the_presignature_trigger_carries_native_value() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let presig = setPreSignatureCall {
            orderUid: Bytes::from(vec![0xab; 56]),
            signed: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack_with_value(Operation::CALL, GP_V2_SETTLEMENT, U256::from(1u64), &presig),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn denies_when_the_twap_create_trigger_carries_native_value() {
        let approve = approve_data(GP_V2_VAULT_RELAYER);
        let create = createCall {
            params: ConditionalOrderParams {
                handler: TWAP_HANDLER,
                salt: B256::ZERO,
                staticInput: Bytes::new(),
            },
            dispatch: true,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, TOKEN, &approve),
            pack_with_value(Operation::CALL, COMPOSABLE_COW, U256::from(1u64), &create),
        ]);
        assert_eq!(
            check(&tx(MULTI_SEND, data.into(), Operation::DELEGATECALL)).await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn no_opinion_on_an_unsupported_chain() {
        let data = approve_data(GP_V2_VAULT_RELAYER);
        let transaction = SafeTransaction {
            chainId: U256::from(137u64),
            ..tx(TOKEN, data, Operation::CALL)
        };
        assert_eq!(check(&transaction).await, CheckOutcome::Unknown);
    }

    #[tokio::test]
    async fn no_opinion_when_the_order_lookup_fails() {
        let calls = sub_transactions(&batched_presig_tx(
            U256::from(100u64),
            ORDER_UID.to_vec().into(),
        ));
        assert_eq!(
            CowChecker::with_order_api(FakeOrderApi::NotFound)
                .check_presignature_batch(SAFE, U256::from(1u64), &calls)
                .await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn approves_when_the_batched_approval_matches_the_swap_order() {
        let order = order(100);
        let calls = sub_transactions(&batched_presig_tx(
            U256::from(100u64),
            order_uid_for(&order),
        ));
        assert_eq!(
            CowChecker::with_order_api(FakeOrderApi::Found(order))
                .check_presignature_batch(SAFE, U256::from(1u64), &calls)
                .await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn denies_when_the_batched_approval_does_not_match_the_swap_order_amount() {
        let order = order(1000);
        let calls = sub_transactions(&batched_presig_tx(U256::from(1u64), order_uid_for(&order)));
        assert_eq!(
            CowChecker::with_order_api(FakeOrderApi::Found(order))
                .check_presignature_batch(SAFE, U256::from(1u64), &calls)
                .await,
            CheckOutcome::Denied(RuleId::R4_5ExcessiveApproval)
        );
    }

    #[tokio::test]
    async fn denies_when_the_swap_order_receiver_is_not_the_safe() {
        let order = CowOrder {
            receiver: Some(Address::new([9u8; 20])),
            ..order(100)
        };
        let calls = sub_transactions(&batched_presig_tx(
            U256::from(100u64),
            order_uid_for(&order),
        ));
        assert_eq!(
            CowChecker::with_order_api(FakeOrderApi::Found(order))
                .check_presignature_batch(SAFE, U256::from(1u64), &calls)
                .await,
            CheckOutcome::Denied(RuleId::R4_4AuthorizationTarget)
        );
    }

    #[tokio::test]
    async fn approves_when_the_swap_order_receiver_is_the_zero_address() {
        // CoW orders are commonly created with `receiver: 0x0` to mean
        // "same as owner" — that must fall back to `owner` (the Safe) just
        // like `receiver: null` does, not be taken as a literal (and
        // therefore mismatching) recipient.
        let order = CowOrder {
            receiver: Some(Address::ZERO),
            ..order(100)
        };
        let calls = sub_transactions(&batched_presig_tx(
            U256::from(100u64),
            order_uid_for(&order),
        ));
        assert_eq!(
            CowChecker::with_order_api(FakeOrderApi::Found(order))
                .check_presignature_batch(SAFE, U256::from(1u64), &calls)
                .await,
            CheckOutcome::Approved
        );
    }

    #[tokio::test]
    async fn no_opinion_when_the_order_response_does_not_match_the_requested_order_uid() {
        // The response decodes fine and, taken at face value, looks like a
        // legitimate match — but its own recomputed digest doesn't
        // correspond to the `orderUid` the presignature actually commits
        // to, e.g. a compromised/malicious API responding to the right UID
        // with a different order's terms. Must not be approved on that
        // basis.
        let order = order(100);
        let calls = sub_transactions(&batched_presig_tx(
            U256::from(100u64),
            ORDER_UID.to_vec().into(),
        ));
        assert_eq!(
            CowChecker::with_order_api(FakeOrderApi::Found(order))
                .check_presignature_batch(SAFE, U256::from(1u64), &calls)
                .await,
            CheckOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn no_opinion_outside_the_recognized_presignature_shape() {
        let checker = CowChecker::with_order_api(FakeOrderApi::NotFound);
        let calls = sub_transactions(&batched_presig_tx(
            U256::from(100u64),
            ORDER_UID.to_vec().into(),
        ));

        // Unsupported chain.
        assert_eq!(
            checker
                .check_presignature_batch(SAFE, U256::from(137u64), &calls)
                .await,
            CheckOutcome::Unknown
        );

        // Not a two-call batch.
        let single = calls[..1].to_vec();
        assert_eq!(
            checker
                .check_presignature_batch(SAFE, U256::from(1u64), &single)
                .await,
            CheckOutcome::Unknown
        );
    }

    /// Pins [`compute_order_uid`] against a real, independently-fetched
    /// mainnet order (`GET api.cow.fi/mainnet/api/v1/orders/{uid}`) rather
    /// than only round-tripping against itself — every field below (and the
    /// expected `uid`) is exactly what CoW's API returned for this order,
    /// not test-authored data.
    #[test]
    fn compute_order_uid_matches_a_real_mainnet_order() {
        let order = CowOrder {
            owner: address!("f7698b47a3897ab4d3fb25940849d11c24be0c28"),
            receiver: Some(address!("f7698b47a3897ab4d3fb25940849d11c24be0c28")),
            sell_token: address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            buy_token: address!("77e06c9eccf2e797fd462a92b6d7642ef85b0a44"),
            sell_amount: U256::from(1231744908760303320u64),
            buy_amount: U256::from(12257931028u64),
            valid_to: 1785225939,
            app_data: B256::ZERO,
            fee_amount: U256::ZERO,
            kind: "sell".to_string(),
            partially_fillable: false,
            sell_token_balance: "erc20".to_string(),
            buy_token_balance: "erc20".to_string(),
        };
        // The full, unsplit orderUid exactly as CoW's API returned it, for
        // this same order: `aad8d0...b97b2` (digest) ‖ `f7698b...be0c28`
        // (owner, matches `order.owner` above) ‖ `6a6862d3` (validTo,
        // 1785225939 — matches `order.valid_to` above).
        let expected_uid = alloy::hex!(
            "aad8d0e2423d65dd21862a14df1012f9ba720eb1b39fca83eb370881422b97b2"
            "f7698b47a3897ab4d3fb25940849d11c24be0c28"
            "6a6862d3"
        );
        assert_eq!(compute_order_uid(U256::from(1u64), &order), expected_uid);
    }
}
