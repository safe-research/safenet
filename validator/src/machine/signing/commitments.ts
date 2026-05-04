import type { Address, Hex } from "viem";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { GroupId, SignatureId } from "../../frost/types.js";
import type { BaseSigningState, MachineConfig, StateDiff } from "../types.js";

export const buildNonceCommitmentsDiff = (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	{
		gid,
		signatureId,
		message,
		sequence,
		signers,
		block,
		packet,
	}: {
		gid: GroupId;
		signatureId: SignatureId;
		message: Hex;
		sequence: bigint;
		signers: readonly Address[];
		block: bigint;
		packet: BaseSigningState["packet"];
	},
): StateDiff => {
	const { nonceCommitments, nonceProof } = signingClient.createNonceCommitments(
		gid,
		machineConfig.account,
		signatureId,
		message,
		sequence,
		signers,
	);
	return {
		consensus: { signatureIdToMessage: [signatureId, message] },
		signing: [
			message,
			{
				id: "collect_nonce_commitments",
				signatureId,
				deadline: block + machineConfig.signingTimeout,
				lastSigner: undefined,
				packet,
			},
		],
		actions: [{ id: "sign_reveal_nonce_commitments", signatureId, nonceCommitments, nonceProof }],
	};
};
