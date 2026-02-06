// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";

contract SafeTransactionTest is Test {
    using SafeTransaction for SafeTransaction.T;

    function test_SafeTransactionHash() public pure {
        SafeTransaction.T memory transaction = SafeTransaction.T({
            chainId: 0x5afe,
            safe: 0x1111111111111111111111111111111111111111,
            to: 0x2222222222222222222222222222222222222222,
            value: 3,
            data: hex"44444444",
            operation: SafeTransaction.Operation.CALL,
            safeTxGas: 5,
            baseGas: 6,
            gasPrice: 7,
            gasToken: 0x8888888888888888888888888888888888888888,
            refundReceiver: 0x9999999999999999999999999999999999999999,
            nonce: 10
        });

        bytes32 expectedHash = keccak256(
            abi.encodePacked(
                hex"1901",
                keccak256(
                    abi.encode(
                        keccak256("EIP712Domain(uint256 chainId,address verifyingContract)"),
                        0x5afe,
                        0x1111111111111111111111111111111111111111
                    )
                ),
                keccak256(
                    abi.encode(
                        keccak256(
                            "SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)"
                        ),
                        0x2222222222222222222222222222222222222222,
                        3,
                        keccak256(hex"44444444"),
                        0,
                        5,
                        6,
                        7,
                        0x8888888888888888888888888888888888888888,
                        0x9999999999999999999999999999999999999999,
                        10
                    )
                )
            )
        );

        assertEq(transaction.hash(), expectedHash);
    }
}
