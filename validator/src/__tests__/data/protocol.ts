import { type Address, type Hex, zeroAddress } from "viem";
import { entryPoint06Address, entryPoint07Address } from "viem/account-abstraction";
import { BaseProtocol, type SubmittedAction } from "../../consensus/protocol/base.js";
import type { EthTransactionData } from "../../consensus/protocol/onchain.js";
import type {
	AttestTransaction,
	Complain,
	ComplaintResponse,
	ConfirmKeyGen,
	ProtocolAction,
	PublishSecretShares,
	PublishSignatureShare,
	RegisterNonceCommitments,
	RequestSignature,
	RevealNonceCommitments,
	SetValidatorStaker,
	StageEpoch,
	StartKeyGen,
} from "../../consensus/protocol/types.js";
import { toPoint } from "../../frost/math.js";
import type { ProtocolLog } from "../../machine/transitions/onchain.js";
import type { StateTransition } from "../../machine/transitions/types.js";

export class TestProtocol extends BaseProtocol {
	chainId(): bigint {
		throw new Error("Method not implemented.");
	}
	consensus(): Address {
		throw new Error("Method not implemented.");
	}
	coordinator(): Address {
		throw new Error("Method not implemented.");
	}
	public startKeyGen(_args: StartKeyGen): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public publishKeygenSecretShares(_args: PublishSecretShares): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public complain(_args: Complain): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public complaintResponse(_args: ComplaintResponse): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public confirmKeyGen(_args: ConfirmKeyGen): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public requestSignature(_args: RequestSignature): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public registerNonceCommitments(_args: RegisterNonceCommitments): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public revealNonceCommitments(_args: RevealNonceCommitments): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public publishSignatureShare(_args: PublishSignatureShare): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public attestTransaction(_args: AttestTransaction): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public stageEpoch(_args: StageEpoch): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
	public setValidatorStaker(_args: SetValidatorStaker): Promise<SubmittedAction> {
		throw new Error("Method not implemented.");
	}
}

export const TEST_POINT = toPoint({
	x: 105587021125387004117772930966558154492652686110919450580386247155506502192059n,
	y: 97790146336079427917878178932139533907352200097479391118658154349645214584696n,
});

export const TEST_CONSENSUS = entryPoint06Address;
export const TEST_COORDINATOR = entryPoint07Address;

export const TEST_ACTIONS: [ProtocolAction, keyof TestProtocol, EthTransactionData][] = [
	[
		{
			id: "sign_request",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			message: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		},
		"requestSignature",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x86f576355afe0000000000000000000000000000000000000000000000000000000000005afe5afe00000000000000000000000000000000000000000000000000000000",
			gas: 400_000n,
		},
	],
	[
		{
			id: "sign_register_nonce_commitments",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			nonceCommitmentsHash: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		},
		"registerNonceCommitments",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x42b29c615afe0000000000000000000000000000000000000000000000000000000000005afe5afe00000000000000000000000000000000000000000000000000000000",
			gas: 250_000n,
		},
	],
	[
		{
			id: "sign_reveal_nonce_commitments",
			signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			nonceCommitments: {
				bindingNonceCommitment: TEST_POINT,
				hidingNonceCommitment: TEST_POINT,
			},
			nonceProof: [
				"0x5afe010000000000000000000000000000000000000000000000000000000000",
				"0x5afe020000000000000000000000000000000000000000000000000000000000",
			],
		},
		"revealNonceCommitments",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x527bdde95afe000000000000000000000000000000000000000000000000000000000000e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f78e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f7800000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000025afe0100000000000000000000000000000000000000000000000000000000005afe020000000000000000000000000000000000000000000000000000000000",
			gas: 200_000n,
		},
	],
	[
		{
			id: "sign_publish_signature_share",
			signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			signersRoot: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			signersProof: [
				"0x5afe010000000000000000000000000000000000000000000000000000000000",
				"0x5afe020000000000000000000000000000000000000000000000000000000000",
			],
			groupCommitment: TEST_POINT,
			commitmentShare: TEST_POINT,
			signatureShare: 1n,
			lagrangeCoefficient: 2n,
		},
		"publishSignatureShare",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x243e8b835afe000000000000000000000000000000000000000000000000000000000000e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f785afe000000000000000000000000000000000000000000000000000000000000e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f7800000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000000025afe0100000000000000000000000000000000000000000000000000000000005afe020000000000000000000000000000000000000000000000000000000000",
			gas: 400_000n,
		},
	],
	[
		{
			id: "sign_publish_signature_share",
			signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			signersRoot: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			signersProof: [
				"0x5afe010000000000000000000000000000000000000000000000000000000000",
				"0x5afe020000000000000000000000000000000000000000000000000000000000",
			],
			groupCommitment: TEST_POINT,
			commitmentShare: TEST_POINT,
			signatureShare: 1n,
			lagrangeCoefficient: 2n,
			callbackContext: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		},
		"publishSignatureShare",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x95b57d9d5afe000000000000000000000000000000000000000000000000000000000000e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f785afe000000000000000000000000000000000000000000000000000000000000e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f7800000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000000000000000000000025afe0100000000000000000000000000000000000000000000000000000000005afe0200000000000000000000000000000000000000000000000000000000000000000000000000000000005ff137d4b0fdcd49dca30c7cf57e578a026d2789000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000205afe00aa00000000000000000000000000000000000000000000000000000000",
			gas: 400_000n,
		},
	],
	[
		{
			id: "key_gen_start",
			participants: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			count: 4,
			threshold: 3,
			context: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
			encryptionPublicKey: TEST_POINT,
			commitments: [TEST_POINT, TEST_POINT],
			pok: {
				r: TEST_POINT,
				mu: 5n,
			},
			poap: [
				"0x5afe010000000000000000000000000000000000000000000000000000000000",
				"0x5afe020000000000000000000000000000000000000000000000000000000000",
			],
		},
		"startKeyGen",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x38b544635afe000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000035afe00aa0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000000025afe0100000000000000000000000000000000000000000000000000000000005afe020000000000000000000000000000000000000000000000000000000000e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f7800000000000000000000000000000000000000000000000000000000000000c0e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f7800000000000000000000000000000000000000000000000000000000000000050000000000000000000000000000000000000000000000000000000000000002e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f78e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f78",
			gas: 250_000n,
		},
	],
	[
		{
			id: "key_gen_publish_secret_shares",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			verificationShare: TEST_POINT,
			shares: [1n, 2n, 3n, 5n, 8n, 13n],
		},
		"publishKeygenSecretShares",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x7d10c04b5afe0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f780000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000050000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000d",
			gas: 400_000n,
		},
	],
	[
		{
			id: "key_gen_complain",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			accused: "0x0000000000000000000000000000000000000001",
		},
		"complain",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x0b2b35375afe0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
			gas: 300_000n,
		},
	],
	[
		{
			id: "key_gen_complaint_response",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			plaintiff: "0x0000000000000000000000000000000000000002",
			secretShare: 0x5afe5afe5afen,
		},
		"complaintResponse",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x01b443335afe000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000005afe5afe5afe",
			gas: 300_000n,
		},
	],
	[
		{
			id: "key_gen_confirm",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		},
		"confirmKeyGen",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x1169f60e5afe000000000000000000000000000000000000000000000000000000000000",
			gas: 200_000n,
		},
	],
	[
		{
			id: "key_gen_confirm",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			callbackContext: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		},
		"confirmKeyGen",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x1896ae365afe00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000005ff137d4b0fdcd49dca30c7cf57e578a026d2789000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000205afe00aa00000000000000000000000000000000000000000000000000000000",
			gas: 300_000n,
		},
	],
	[
		{
			id: "consensus_attest_transaction",
			epoch: 10n,
			chainId: 100n,
			safe: zeroAddress,
			safeTxStructHash: "0x5afe00aa00000000000000000000000000000000000000000000000000000000" as Hex,
			signatureId: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		},
		"attestTransaction",
		{
			to: TEST_CONSENSUS,
			value: 0n,
			data: "0xaa8d1739000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000006400000000000000000000000000000000000000000000000000000000000000005afe00aa000000000000000000000000000000000000000000000000000000005afe00aa00000000000000000000000000000000000000000000000000000000",
			gas: 400_000n,
		},
	],
	[
		{
			id: "consensus_stage_epoch",
			proposedEpoch: 10n,
			rolloverBlock: 30n,
			groupId: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
			signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		},
		"stageEpoch",
		{
			to: TEST_CONSENSUS,
			value: 0n,
			data: "0xea5eeafa000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000001e5afe00aa000000000000000000000000000000000000000000000000000000005afe000000000000000000000000000000000000000000000000000000000000",
			gas: 400_000n,
		},
	],
	[
		{
			id: "consensus_set_validator_staker",
			staker: "0x5AFE000000000000000000000000000000000000",
		},
		"setValidatorStaker",
		{
			to: TEST_CONSENSUS,
			value: 0n,
			data: "0xbbce66a60000000000000000000000005afe000000000000000000000000000000000000",
			gas: 400_000n,
		},
	],
];

export const TEST_EVENTS: [ProtocolLog | null, StateTransition][] = [
	[
		null,
		{
			id: "block_new",
			block: 111n,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// KeyGen(bytes32 indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 indexed context)
			eventName: "KeyGen",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				participants: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
				count: 4,
				threshold: 3,
				context: "0x5afecc0000000000000000000000000000000000000000000000000000000000",
			},
		},
		{
			id: "event_key_gen",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			participants: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
			count: 4,
			threshold: 3,
			context: "0x5afecc0000000000000000000000000000000000000000000000000000000000",
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// KeyGenCommitted(bytes32 indexed gid, address participant, ((uint256 x, uint256 y) q, (uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment, bool committed)
			eventName: "KeyGenCommitted",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				participant: zeroAddress,
				commitment: {
					q: TEST_POINT,
					r: TEST_POINT,
					mu: 123n,
					c: [TEST_POINT, TEST_POINT],
				},
				committed: true,
			},
		},
		{
			id: "event_key_gen_committed",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			participant: zeroAddress,
			commitment: {
				q: TEST_POINT,
				r: TEST_POINT,
				mu: 123n,
				c: [TEST_POINT, TEST_POINT],
			},
			committed: true,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// KeyGenSecretShared(bytes32 indexed gid, address participant, ((uint256 x, uint256 y) y, uint256[] f) share, bool shared)
			eventName: "KeyGenSecretShared",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				participant: zeroAddress,
				share: {
					y: TEST_POINT,
					f: [1n, 2n, 3n, 5n, 8n],
				},
				shared: true,
			},
		},
		{
			id: "event_key_gen_secret_shared",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			participant: zeroAddress,
			share: {
				y: TEST_POINT,
				f: [1n, 2n, 3n, 5n, 8n],
			},
			shared: true,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// KeyGenComplained(bytes32 indexed gid, address plaintiff, address accused, bool compromised)
			eventName: "KeyGenComplained",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				plaintiff: "0x0000000000000000000000000000000000000001",
				accused: "0x0000000000000000000000000000000000000002",
				compromised: false,
			},
		},
		{
			id: "event_key_gen_complaint_submitted",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			plaintiff: "0x0000000000000000000000000000000000000001",
			accused: "0x0000000000000000000000000000000000000002",
			compromised: false,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// KeyGenComplaintResponded(bytes32 indexed gid, address plaintiff, address accused, uint256 secretShare)
			eventName: "KeyGenComplaintResponded",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				plaintiff: "0x0000000000000000000000000000000000000001",
				accused: "0x0000000000000000000000000000000000000002",
				secretShare: 0x5afe5afe5afen,
			},
		},
		{
			id: "event_key_gen_complaint_responded",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			plaintiff: "0x0000000000000000000000000000000000000001",
			accused: "0x0000000000000000000000000000000000000002",
			secretShare: 0x5afe5afe5afen,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed)
			eventName: "KeyGenConfirmed",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				participant: zeroAddress,
				confirmed: true,
			},
		},
		{
			id: "event_key_gen_confirmed",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			participant: zeroAddress,
			confirmed: true,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment)
			eventName: "Preprocess",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				participant: zeroAddress,
				chunk: 100n,
				commitment: "0x5afeaabb00000000000000000000000000000000000000000000000000000000",
			},
		},
		{
			id: "event_nonce_commitments_hash",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			participant: zeroAddress,
			chunk: 100n,
			commitment: "0x5afeaabb00000000000000000000000000000000000000000000000000000000",
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// Sign(address indexed initiator, bytes32 indexed gid, bytes32 indexed message, bytes32 sid, uint64 sequence)
			eventName: "Sign",
			args: {
				initiator: zeroAddress,
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				message: "0x5afeaabbcc000000000000000000000000000000000000000000000000000000",
				sequence: 23n,
			},
		},
		{
			id: "event_sign_request",
			block: 111n,
			index: 0,
			initiator: zeroAddress,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
			message: "0x5afeaabbcc000000000000000000000000000000000000000000000000000000",
			sequence: 23n,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// SignRevealedNonces(bytes32 indexed sid, address participant, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces)
			eventName: "SignRevealedNonces",
			args: {
				sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				participant: zeroAddress,
				nonces: {
					d: TEST_POINT,
					e: TEST_POINT,
				},
			},
		},
		{
			id: "event_nonce_commitments",
			block: 111n,
			index: 0,
			sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
			participant: zeroAddress,
			nonces: {
				d: TEST_POINT,
				e: TEST_POINT,
			},
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, address participant, uint256 z)
			eventName: "SignShared",
			args: {
				sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				selectionRoot: "0x5af35af35af35af3000000000000000000000000000000000000000000000000",
				participant: zeroAddress,
				z: 12345n,
			},
		},
		{
			id: "event_signature_share",
			block: 111n,
			index: 0,
			sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
			selectionRoot: "0x5af35af35af35af3000000000000000000000000000000000000000000000000",
			participant: zeroAddress,
			z: 12345n,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// SignCompleted(bytes32 indexed sid, bytes32 indexed selectionRoot, ((uint256 x, uint256 y) r, uint256 z) signature)
			eventName: "SignCompleted",
			args: {
				sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				selectionRoot: "0x5af35af35af35af3000000000000000000000000000000000000000000000000",
				signature: {
					z: 12345n,
					r: TEST_POINT,
				},
			},
		},
		{
			id: "event_signed",
			block: 111n,
			index: 0,
			sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
			selectionRoot: "0x5af35af35af35af3000000000000000000000000000000000000000000000000",
			signature: {
				z: 12345n,
				r: TEST_POINT,
			},
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256 x, uint256 y) groupKey)
			eventName: "EpochProposed",
			args: {
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 3n,
				groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				groupKey: TEST_POINT,
			},
		},
		{
			id: "event_epoch_proposed",
			block: 111n,
			index: 0,
			activeEpoch: 1n,
			proposedEpoch: 2n,
			rolloverBlock: 3n,
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			groupKey: TEST_POINT,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, bytes32 groupId, (uint256 x, uint256 y) groupKey, bytes32 signatureId, ((uint256 x, uint256 y) r, uint256 z) attestation)
			eventName: "EpochStaged",
			args: {
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 3n,
				groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				groupKey: TEST_POINT,
				signatureId: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				attestation: {
					r: TEST_POINT,
					z: 12345n,
				},
			},
		},
		{
			id: "event_epoch_staged",
			block: 111n,
			index: 0,
			activeEpoch: 1n,
			proposedEpoch: 2n,
			rolloverBlock: 3n,
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			groupKey: TEST_POINT,
			signatureId: "0x5af3000000000000000000000000000000000000000000000000000000000000",
			attestation: {
				z: 12345n,
				r: TEST_POINT,
			},
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// TransactionProposed(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, (uint256 chainId, address safe, address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, uint256 nonce) transaction)
			eventName: "TransactionProposed",
			args: {
				safeTxHash: "0x5af3aabbcc000000000000000000000000000000000000000000000000000000",
				chainId: 100n,
				safe: zeroAddress,
				epoch: 2n,
				transaction: {
					chainId: 100n,
					safe: zeroAddress,
					to: zeroAddress,
					value: 10n,
					data: "0x",
					operation: 1,
					safeTxGas: 42n,
					baseGas: 1337n,
					gasPrice: 1000000n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 3n,
				},
			},
		},
		{
			id: "event_transaction_proposed",
			block: 111n,
			index: 0,
			safeTxHash: "0x5af3aabbcc000000000000000000000000000000000000000000000000000000",
			chainId: 100n,
			safe: zeroAddress,
			epoch: 2n,
			transaction: {
				chainId: 100n,
				safe: zeroAddress,
				to: zeroAddress,
				value: 10n,
				data: "0x",
				operation: 1,
				safeTxGas: 42n,
				baseGas: 1337n,
				gasPrice: 1000000n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 3n,
			},
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// TransactionAttested(bytes32 indexed safeTxHash, uint256 indexed chainId, address indexed safe, uint64 epoch, bytes32 signatureId, ((uint256 x, uint256 y) r, uint256 z) attestation)
			eventName: "TransactionAttested",
			args: {
				safeTxHash: "0x5af3330000000000000000000000000000000000000000000000000000000000",
				chainId: 100n,
				safe: zeroAddress,
				epoch: 42n,
				signatureId: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				attestation: {
					r: TEST_POINT,
					z: 12345n,
				},
			},
		},
		{
			id: "event_transaction_attested",
			block: 111n,
			index: 0,
			safeTxHash: "0x5af3330000000000000000000000000000000000000000000000000000000000",
			chainId: 100n,
			safe: zeroAddress,
			epoch: 42n,
			signatureId: "0x5af3000000000000000000000000000000000000000000000000000000000000",
			attestation: {
				r: TEST_POINT,
				z: 12345n,
			},
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			address: zeroAddress,
			// OracleResult(bytes32 indexed requestId, address indexed proposer, bytes result, bool approved)
			eventName: "OracleResult",
			args: {
				requestId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				proposer: zeroAddress,
				result: "0x",
				approved: true,
			},
		},
		{
			id: "event_oracle_result",
			block: 111n,
			index: 0,
			oracle: zeroAddress,
			requestId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			proposer: zeroAddress,
			result: "0x",
			approved: true,
		},
	],
];
