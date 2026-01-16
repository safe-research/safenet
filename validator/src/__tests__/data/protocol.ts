import { type Address, type Hex, zeroAddress } from "viem";
import { entryPoint06Address, entryPoint07Address } from "viem/account-abstraction";
import { BaseProtocol } from "../../consensus/protocol/base.js";
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
	public startKeyGen(_args: StartKeyGen): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public publishKeygenSecretShares(_args: PublishSecretShares): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public complain(_args: Complain): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public complaintResponse(_args: ComplaintResponse): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public confirmKeyGen(_args: ConfirmKeyGen): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public requestSignature(_args: RequestSignature): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public registerNonceCommitments(_args: RegisterNonceCommitments): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public revealNonceCommitments(_args: RevealNonceCommitments): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public publishSignatureShare(_args: PublishSignatureShare): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public attestTransaction(_args: AttestTransaction): Promise<Hex> {
		throw new Error("Method not implemented.");
	}
	public stageEpoch(_args: StageEpoch): Promise<Hex> {
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
			gas: undefined,
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
			gas: undefined,
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
			gas: undefined,
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
			participantId: 1n,
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
			data: "0x4a53702c5afe000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000035afe00aa00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000000025afe0100000000000000000000000000000000000000000000000000000000005afe0200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000080e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f7800000000000000000000000000000000000000000000000000000000000000050000000000000000000000000000000000000000000000000000000000000002e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f78e97022d9e91554adbaecc6738ac72daa9710672694aafb09f18d485bcaa04bbbd83342eaaa01aefa449e9061a287f3bf39739fcfaeae8a7a13f507215a010f78",
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
			gas: 350_000n,
		},
	],
	[
		{
			id: "key_gen_complain",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			accused: 1n,
		},
		"complain",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x0b2b35375afe0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
			gas: undefined,
		},
	],
	[
		{
			id: "key_gen_complaint_response",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			plaintiff: 2n,
			secretShare: 0x5afe5afe5afen,
		},
		"complaintResponse",
		{
			to: TEST_COORDINATOR,
			value: 0n,
			data: "0x01b443335afe000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000005afe5afe5afe",
			gas: undefined,
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
			gas: 100_000n,
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
			transactionHash: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
			signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		},
		"attestTransaction",
		{
			to: TEST_CONSENSUS,
			value: 0n,
			data: "0x68c0ce39000000000000000000000000000000000000000000000000000000000000000a5afe00aa000000000000000000000000000000000000000000000000000000005afe000000000000000000000000000000000000000000000000000000000000",
			gas: undefined,
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
			gas: undefined,
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
			// KeyGen(bytes32 indexed gid, bytes32 participants, uint16 count, uint16 threshold, bytes32 context)
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
			// KeyGenCommitted(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment, bool committed)
			eventName: "KeyGenCommitted",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				identifier: 1n,
				commitment: {
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
			identifier: 1n,
			commitment: {
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
			// KeyGenSecretShared(bytes32 indexed gid, uint256 identifier, ((uint256 x, uint256 y) y, uint256[] f) share, bool shared)
			eventName: "KeyGenSecretShared",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				identifier: 1n,
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
			identifier: 1n,
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
			// KeyGenComplained(bytes32 indexed gid, uint256 plaintiff, uint256 accused, bool compromised)
			eventName: "KeyGenComplained",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				plaintiff: 1n,
				accused: 2n,
				compromised: false,
			},
		},
		{
			id: "event_key_gen_complaint_submitted",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			plaintiff: 1n,
			accused: 2n,
			compromised: false,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			// KeyGenComplaintResponded(bytes32 indexed gid, uint256 plaintiff, uint256 accused, uint256 secretShare)
			eventName: "KeyGenComplaintResponded",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				plaintiff: 1n,
				accused: 2n,
				secretShare: 0x5afe5afe5afen,
			},
		},
		{
			id: "event_key_gen_complaint_responded",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			plaintiff: 1n,
			accused: 2n,
			secretShare: 0x5afe5afe5afen,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			// KeyGenConfirmed(bytes32 indexed gid, uint256 identifier, bool confirmed)
			eventName: "KeyGenConfirmed",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				identifier: 1n,
				confirmed: true,
			},
		},
		{
			id: "event_key_gen_confirmed",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			identifier: 1n,
			confirmed: true,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			// Preprocess(bytes32 indexed gid, uint256 identifier, uint64 chunk, bytes32 commitment)
			eventName: "Preprocess",
			args: {
				gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				identifier: 1n,
				chunk: 100n,
				commitment: "0x5afeaabb00000000000000000000000000000000000000000000000000000000",
			},
		},
		{
			id: "event_nonce_commitments_hash",
			block: 111n,
			index: 0,
			gid: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			identifier: 1n,
			chunk: 100n,
			commitment: "0x5afeaabb00000000000000000000000000000000000000000000000000000000",
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
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
			// SignRevealedNonces(bytes32 indexed sid, uint256 identifier, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces)
			eventName: "SignRevealedNonces",
			args: {
				sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				identifier: 1n,
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
			identifier: 1n,
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
			// SignShared(bytes32 indexed sid, uint256 identifier, uint256 z, bytes32 root)
			eventName: "SignShared",
			args: {
				sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
				identifier: 1n,
				z: 12345n,
				root: "0x5af35af35af35af3000000000000000000000000000000000000000000000000",
			},
		},
		{
			id: "event_signature_share",
			block: 111n,
			index: 0,
			sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
			identifier: 1n,
			z: 12345n,
			root: "0x5af35af35af35af3000000000000000000000000000000000000000000000000",
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			// SignCompleted(bytes32 indexed sid, ((uint256 x, uint256 y) r, uint256 z) signature)
			eventName: "SignCompleted",
			args: {
				sid: "0x5af3000000000000000000000000000000000000000000000000000000000000",
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
			// EpochProposed(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, (uint256 x, uint256 y) groupKey)
			eventName: "EpochProposed",
			args: {
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 3n,
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
			groupKey: TEST_POINT,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			// EpochStaged(uint64 indexed activeEpoch, uint64 indexed proposedEpoch, uint64 rolloverBlock, (uint256 x, uint256 y) groupKey)
			eventName: "EpochStaged",
			args: {
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 3n,
				groupKey: TEST_POINT,
			},
		},
		{
			id: "event_epoch_staged",
			block: 111n,
			index: 0,
			activeEpoch: 1n,
			proposedEpoch: 2n,
			rolloverBlock: 3n,
			groupKey: TEST_POINT,
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			// TransactionProposed(bytes32 indexed message, bytes32 indexed transactionHash, uint64 epoch, (uint256 chainId, address account, address to, uint256 value, uint8 operation, bytes data, uint256 nonce) transaction)
			eventName: "TransactionProposed",
			args: {
				message: "0x5af3330000000000000000000000000000000000000000000000000000000000",
				transactionHash: "0x5af3aabbcc000000000000000000000000000000000000000000000000000000",
				epoch: 2n,
				transaction: {
					to: zeroAddress,
					value: 10n,
					data: "0x",
					operation: 1,
					nonce: 3n,
					chainId: 100n,
					account: zeroAddress,
				},
			},
		},
		{
			id: "event_transaction_proposed",
			block: 111n,
			index: 0,
			message: "0x5af3330000000000000000000000000000000000000000000000000000000000",
			transactionHash: "0x5af3aabbcc000000000000000000000000000000000000000000000000000000",
			epoch: 2n,
			transaction: {
				to: zeroAddress,
				value: 10n,
				data: "0x",
				operation: 1,
				nonce: 3n,
				chainId: 100n,
				account: zeroAddress,
			},
		},
	],
	[
		{
			blockNumber: 111n,
			logIndex: 0,
			// TransactionAttested(bytes32 indexed message)
			eventName: "TransactionAttested",
			args: {
				message: "0x5af3330000000000000000000000000000000000000000000000000000000000",
			},
		},
		{
			id: "event_transaction_attested",
			block: 111n,
			index: 0,
			message: "0x5af3330000000000000000000000000000000000000000000000000000000000",
		},
	],
];
