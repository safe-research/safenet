import type { Address, Hex } from "viem";
import { BaseProtocol } from "../../consensus/protocol/base.js";
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

export const TEST_ACTIONS: [ProtocolAction, keyof TestProtocol][] = [
	[
		{
			id: "sign_request",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			message: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		},
		"requestSignature",
	],
	[
		{
			id: "sign_register_nonce_commitments",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			nonceCommitmentsHash: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		},
		"registerNonceCommitments",
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
	],
	[
		{
			id: "key_gen_publish_secret_shares",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			verificationShare: TEST_POINT,
			shares: [1n, 2n, 3n, 5n, 8n, 13n],
		},
		"publishKeygenSecretShares",
	],
	[
		{
			id: "key_gen_complain",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			accused: 1n,
		},
		"complain",
	],
	[
		{
			id: "key_gen_complaint_response",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			plaintiff: 2n,
			secretShare: 0x5afe5afe5afen,
		},
		"complaintResponse",
	],
	[
		{
			id: "key_gen_confirm",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		},
		"confirmKeyGen",
	],
	[
		{
			id: "key_gen_confirm",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			callbackContext: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		},
		"confirmKeyGen",
	],
	[
		{
			id: "consensus_attest_transaction",
			epoch: 10n,
			transactionHash: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
			signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		},
		"attestTransaction",
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
	],
];
