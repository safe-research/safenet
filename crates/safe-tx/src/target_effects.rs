//! Pure calldata decoding: extracts the address a Safe transaction (or, when
//! it's a MultiSend batch, each of its sub-calls) sends value to, or grants
//! transfer/spending authority over tokens to. No policy decisions are made
//! here — see `crate::checks` for the existing base-guarantee policy, and
//! future `Check`s (elsewhere) that will consume this output.

use crate::multi_send::{decode_multi_send, known_deployment};
use crate::types::{Operation, SafeTransaction, erc20, erc721, erc1155, multi_send_bindings};
use alloy::{
    primitives::{Address, U256},
    sol_types::SolCall as _,
};

/// A decoded value- or authority-granting effect that a Safe transaction (or
/// one of its MultiSend sub-calls) has on a target address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetEffect {
    /// The address receiving value, tokens, or approved spending/transfer
    /// authority.
    pub recipient: Address,
    pub kind: EffectKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectKind {
    /// A native value transfer, or an ERC-20 `transfer`/`transferFrom`.
    ValueTransfer { amount: U256 },
    /// An ERC-20 `approve`. Note: ERC-721's single-token
    /// `approve(address,uint256)` has the exact same selector and is
    /// indistinguishable from calldata alone — it decodes the same way.
    Erc20Approval { amount: U256 },
    /// ERC-721/ERC-1155 `setApprovalForAll` (identical selector on both
    /// standards, so likewise not distinguishable from calldata alone).
    OperatorApproval { approved: bool },
    /// ERC-721 `safeTransferFrom(address,address,uint256)`.
    Erc721Transfer { token_id: U256 },
    /// ERC-1155 `safeTransferFrom(address,address,uint256,uint256,bytes)`.
    Erc1155Transfer { id: U256, amount: U256 },
    /// ERC-1155 `safeBatchTransferFrom`.
    Erc1155BatchTransfer { ids: Vec<U256>, amounts: Vec<U256> },
}

/// Decodes the target effects of a Safe transaction, recursing through
/// MultiSend so each batched sub-call is decoded individually.
pub fn decode_target_effects(tx: &SafeTransaction) -> Vec<TargetEffect> {
    if tx.operation == Operation::DELEGATECALL
        && let Some((version, _)) = known_deployment(tx.to)
        && let Ok(call) = multi_send_bindings::multiSendCall::abi_decode(&tx.data)
    {
        return decode_multi_send(tx.safe, &call.transactions, version)
            .unwrap_or_default()
            .iter()
            .flat_map(decode_target_effects)
            .collect();
    }

    decode_call(tx)
}

/// Decodes a single (non-MultiSend) call into its target effects: a native
/// value transfer (if `tx.value` is non-zero) plus, independently, whatever
/// token effect `tx.data` decodes to — a call can carry both at once (e.g. a
/// `payable` ERC-20 `transfer`), so neither is allowed to suppress the other.
fn decode_call(tx: &SafeTransaction) -> Vec<TargetEffect> {
    let mut effects = Vec::new();
    if !tx.value.is_zero() {
        effects.push(TargetEffect {
            recipient: tx.to,
            kind: EffectKind::ValueTransfer { amount: tx.value },
        });
    }

    let data = &tx.data;
    if data.is_empty() {
        return effects;
    }
    if let Ok(call) = erc20::transferCall::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.to,
            kind: EffectKind::ValueTransfer {
                amount: call.amount,
            },
        });
    } else if let Ok(call) = erc20::transferFromCall::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.to,
            kind: EffectKind::ValueTransfer {
                amount: call.amount,
            },
        });
    } else if let Ok(call) = erc20::approveCall::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.spender,
            kind: EffectKind::Erc20Approval {
                amount: call.amount,
            },
        });
    } else if let Ok(call) = erc721::setApprovalForAllCall::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.operator,
            kind: EffectKind::OperatorApproval {
                approved: call.approved,
            },
        });
    } else if let Ok(call) = erc721::safeTransferFrom_0Call::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.to,
            kind: EffectKind::Erc721Transfer {
                token_id: call.tokenId,
            },
        });
    } else if let Ok(call) = erc721::safeTransferFrom_1Call::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.to,
            kind: EffectKind::Erc721Transfer {
                token_id: call.tokenId,
            },
        });
    } else if let Ok(call) = erc1155::safeTransferFromCall::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.to,
            kind: EffectKind::Erc1155Transfer {
                id: call.id,
                amount: call.amount,
            },
        });
    } else if let Ok(call) = erc1155::safeBatchTransferFromCall::abi_decode(data) {
        effects.push(TargetEffect {
            recipient: call.to,
            kind: EffectKind::Erc1155BatchTransfer {
                ids: call.ids,
                amounts: call.amounts,
            },
        });
    }

    effects
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Bytes, address};

    fn tx(
        to: Address,
        value: U256,
        data: impl Into<Bytes>,
        operation: Operation,
    ) -> SafeTransaction {
        SafeTransaction {
            safe: address!("F01888f0677547Ec07cd16c8680e699c96588E6B"),
            to,
            value,
            data: data.into(),
            operation,
            ..Default::default()
        }
    }

    fn pack(operation: Operation, to: Address, value: U256, data: &[u8]) -> Vec<u8> {
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

    const TOKEN: Address = address!("9C58BAcC331c9aa871AFD802DB6379a98e80CEdb");
    const RECIPIENT: Address = address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110");

    #[test]
    fn decodes_native_value_transfer() {
        let effects = decode_target_effects(&tx(
            RECIPIENT,
            U256::from(5u64),
            Bytes::new(),
            Operation::CALL,
        ));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::ValueTransfer {
                    amount: U256::from(5u64)
                }
            }]
        );
    }

    #[test]
    fn no_effect_for_zero_value_empty_call() {
        assert_eq!(
            decode_target_effects(&tx(RECIPIENT, U256::ZERO, Bytes::new(), Operation::CALL)),
            vec![]
        );
    }

    #[test]
    fn no_effect_for_unrecognized_calldata() {
        let data = Bytes::from(vec![0xde, 0xad, 0xbe, 0xef, 0x01]);
        assert_eq!(
            decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL)),
            vec![]
        );
    }

    #[test]
    fn decodes_native_value_alongside_unrecognized_calldata() {
        let data = Bytes::from(vec![0xde, 0xad, 0xbe, 0xef, 0x01]);
        let effects = decode_target_effects(&tx(TOKEN, U256::from(9u64), data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: TOKEN,
                kind: EffectKind::ValueTransfer {
                    amount: U256::from(9u64)
                }
            }]
        );
    }

    #[test]
    fn decodes_native_value_alongside_token_effect() {
        let data = erc20::approveCall {
            spender: RECIPIENT,
            amount: U256::from(3u64),
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::from(9u64), data, Operation::CALL));
        assert_eq!(
            effects,
            vec![
                TargetEffect {
                    recipient: TOKEN,
                    kind: EffectKind::ValueTransfer {
                        amount: U256::from(9u64)
                    }
                },
                TargetEffect {
                    recipient: RECIPIENT,
                    kind: EffectKind::Erc20Approval {
                        amount: U256::from(3u64)
                    }
                },
            ]
        );
    }

    #[test]
    fn decodes_erc20_transfer() {
        let data = erc20::transferCall {
            to: RECIPIENT,
            amount: U256::from(1_000u64),
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::ValueTransfer {
                    amount: U256::from(1_000u64)
                }
            }]
        );
    }

    #[test]
    fn decodes_erc20_transfer_from() {
        let data = erc20::transferFromCall {
            from: TOKEN,
            to: RECIPIENT,
            amount: U256::from(7u64),
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::ValueTransfer {
                    amount: U256::from(7u64)
                }
            }]
        );
    }

    #[test]
    fn decodes_erc20_unlimited_approval() {
        let data = erc20::approveCall {
            spender: RECIPIENT,
            amount: U256::MAX,
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::Erc20Approval { amount: U256::MAX }
            }]
        );
    }

    #[test]
    fn decodes_operator_approval() {
        let data = erc721::setApprovalForAllCall {
            operator: RECIPIENT,
            approved: true,
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::OperatorApproval { approved: true }
            }]
        );
    }

    #[test]
    fn decodes_erc721_safe_transfer_from() {
        let safe = address!("F01888f0677547Ec07cd16c8680e699c96588E6B");
        let data = erc721::safeTransferFrom_0Call {
            from: safe,
            to: RECIPIENT,
            tokenId: U256::from(42u64),
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::Erc721Transfer {
                    token_id: U256::from(42u64)
                }
            }]
        );
    }

    #[test]
    fn decodes_erc721_safe_transfer_from_with_data() {
        let safe = address!("F01888f0677547Ec07cd16c8680e699c96588E6B");
        let data = erc721::safeTransferFrom_1Call {
            from: safe,
            to: RECIPIENT,
            tokenId: U256::from(7u64),
            data: Bytes::from(vec![0x01]),
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::Erc721Transfer {
                    token_id: U256::from(7u64)
                }
            }]
        );
    }

    #[test]
    fn decodes_erc1155_safe_transfer_from() {
        let safe = address!("F01888f0677547Ec07cd16c8680e699c96588E6B");
        let data = erc1155::safeTransferFromCall {
            from: safe,
            to: RECIPIENT,
            id: U256::from(1u64),
            amount: U256::from(3u64),
            data: Bytes::new(),
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::Erc1155Transfer {
                    id: U256::from(1u64),
                    amount: U256::from(3u64)
                }
            }]
        );
    }

    #[test]
    fn decodes_erc1155_safe_batch_transfer_from() {
        let safe = address!("F01888f0677547Ec07cd16c8680e699c96588E6B");
        let ids = vec![U256::from(1u64), U256::from(2u64)];
        let amounts = vec![U256::from(10u64), U256::from(20u64)];
        let data = erc1155::safeBatchTransferFromCall {
            from: safe,
            to: RECIPIENT,
            ids: ids.clone(),
            amounts: amounts.clone(),
            data: Bytes::new(),
        }
        .abi_encode();
        let effects = decode_target_effects(&tx(TOKEN, U256::ZERO, data, Operation::CALL));
        assert_eq!(
            effects,
            vec![TargetEffect {
                recipient: RECIPIENT,
                kind: EffectKind::Erc1155BatchTransfer { ids, amounts }
            }]
        );
    }

    #[test]
    fn recurses_through_multi_send() {
        let approve_data = erc20::approveCall {
            spender: RECIPIENT,
            amount: U256::MAX,
        }
        .abi_encode();
        let data = multisend(&[
            pack(Operation::CALL, RECIPIENT, U256::from(2u64), &[]),
            pack(Operation::CALL, TOKEN, U256::ZERO, &approve_data),
        ]);

        let effects = decode_target_effects(&tx(
            address!("218543288004CD07832472D464648173c77D7eB7"),
            U256::ZERO,
            data,
            Operation::DELEGATECALL,
        ));

        assert_eq!(
            effects,
            vec![
                TargetEffect {
                    recipient: RECIPIENT,
                    kind: EffectKind::ValueTransfer {
                        amount: U256::from(2u64)
                    }
                },
                TargetEffect {
                    recipient: RECIPIENT,
                    kind: EffectKind::Erc20Approval { amount: U256::MAX }
                },
            ]
        );
    }
}
