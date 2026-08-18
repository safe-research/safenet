// SPDX-License-Identifier: GPL-3.0-only
pragma solidity ^0.8.30;

import {Test} from "@forge-std/Test.sol";
import {ConsensusMessages} from "@/libraries/ConsensusMessages.sol";
import {SafeTransaction} from "@/libraries/SafeTransaction.sol";
import {Secp256k1} from "@/libraries/Secp256k1.sol";

contract ConsensusMessagesTest is Test {
    using ConsensusMessages for bytes32;
    using SafeTransaction for SafeTransaction.T;

    function test_EpochRollover() public pure {
        bytes32 message = ConsensusMessages.domain(23, 0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97)
            .epochRollover(
                0,
                1,
                0xbaddad42,
                Secp256k1.Point({
                    x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75,
                    y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5
                })
            );

        assertEq(message, hex"c1e4d484d6c376741c904290cc043f4afb4618f9d567dcdd0edcbf22abae57f7");
    }

    function test_TransactionProposalTypehash() public pure {
        assertEq(
            ConsensusMessages.TRANSACTION_PROPOSAL_TYPEHASH,
            keccak256("TransactionProposal(uint64 epoch,address oracle,bytes oracleData,bytes32 safeTxHash)")
        );
    }

    function test_TransactionProposal() public pure {
        bytes32 message = ConsensusMessages.domain(23, 0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97)
            .transactionProposal(
                1, 0x1234567890123456789012345678901234567890, keccak256(hex"cafe"), bytes32(uint256(42))
            );

        assertEq(message, hex"973d5f5f4f873250bac60f83283dcff0f1bfbfa8ef19c2120830189a4ffa7084");
    }

    /// Cross-language parity vector for `oracle_tx_proposal_hash_parity` in `crates/sentinel/src/hashing.rs`:
    /// domain.chain=23, consensus=0x22Cb221c..., epoch=11, oracle=safe=0x4838B106..., to=consensus, chainId=1,
    /// all other fields zero. Keep both in sync if either implementation or expected hash changes.
    function test_TransactionProposal_SentinelParityVector() public pure {
        address consensus = 0x22Cb221caE98D6097082C80158B1472C45FEd729;
        address oracle = 0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97;

        SafeTransaction.T memory transaction = SafeTransaction.T({
            chainId: 1,
            safe: oracle,
            to: consensus,
            value: 0,
            data: "",
            operation: SafeTransaction.Operation.CALL,
            safeTxGas: 0,
            baseGas: 0,
            gasPrice: 0,
            gasToken: address(0),
            refundReceiver: address(0),
            nonce: 0
        });

        bytes32 message = ConsensusMessages.domain(23, consensus)
            .transactionProposal(11, oracle, keccak256(hex""), transaction.hash());

        assertEq(message, hex"8080890fb312c10d10238e8eb3d58a5682e4e691862afee94e94726ad1a16dd5");
    }

    /// The EIP-712 message families are domain-separated by their distinct type hashes: an epoch-rollover
    /// message and a transaction-proposal message (under the same domain, with numerically overlapping
    /// inputs) must never collide.
    function test_messageFamiliesDoNotCollide() public pure {
        bytes32 sep = ConsensusMessages.domain(23, 0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97);
        bytes32 rollover = sep.epochRollover(1, 2, 3, Secp256k1.Point({x: 7, y: 9}));
        bytes32 proposal = sep.transactionProposal(1, address(uint160(7)), bytes32(uint256(9)), bytes32(uint256(2)));
        assertTrue(rollover != proposal, "epochRollover and transactionProposal must be domain-separated");
    }

    /// transactionProposal is injective in each of its arguments (and in the domain separator): changing
    /// the epoch, oracle, oracleData hash, Safe tx hash, or the consensus deployment changes the message.
    /// This is what makes an attestation bind exactly its (epoch, oracle, oracleData, safeTx) tuple.
    function test_transactionProposal_injectiveInEachField() public pure {
        bytes32 sep = ConsensusMessages.domain(23, 0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97);
        uint64 epoch = 5;
        address oracle = 0x1234567890123456789012345678901234567890;
        bytes32 odh = keccak256(hex"cafe");
        bytes32 stx = bytes32(uint256(42));

        bytes32 base = sep.transactionProposal(epoch, oracle, odh, stx);
        assertTrue(base != sep.transactionProposal(epoch + 1, oracle, odh, stx), "epoch is bound");
        assertTrue(base != sep.transactionProposal(epoch, address(uint160(oracle) + 1), odh, stx), "oracle is bound");
        assertTrue(
            base != sep.transactionProposal(epoch, oracle, keccak256(hex"beef"), stx), "oracleData hash is bound"
        );
        assertTrue(base != sep.transactionProposal(epoch, oracle, odh, bytes32(uint256(43))), "safeTxHash is bound");

        bytes32 sep2 = ConsensusMessages.domain(23, 0x1111111111111111111111111111111111111111);
        assertTrue(base != sep2.transactionProposal(epoch, oracle, odh, stx), "domain separator is bound");
    }
}
