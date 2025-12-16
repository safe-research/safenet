import { describe, expect, it } from "vitest";
import { toPoint } from "../../frost/math.js";
import { SqliteActionQueue } from "./sqlite.js";
import type { ActionWithTimeout } from "./types.js";

const TEST_POINT = toPoint({
	x: 105587021125387004117772930966558154492652686110919450580386247155506502192059n,
	y: 97790146336079427917878178932139533907352200097479391118658154349645214584696n,
});

const actions: ActionWithTimeout[] = [
	{
		id: "sign_request",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		message: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		validUntil: 0,
	},
	{
		id: "sign_register_nonce_commitments",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		nonceCommitmentsHash: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		validUntil: 0,
	},
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
		validUntil: 1,
	},
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
		validUntil: 1,
	},
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
		validUntil: 1,
	},
	{
		id: "key_gen_start",
		participants: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		count: 4n,
		threshold: 3n,
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
		validUntil: 1,
	},
	{
		id: "key_gen_publish_secret_shares",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		verificationShare: TEST_POINT,
		shares: [1n, 2n, 3n, 5n, 8n, 13n],
		validUntil: 1,
	},
	{
		id: "key_gen_complain",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		accused: 1n,
		validUntil: 1,
	},
	{
		id: "key_gen_complaint_response",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		plaintiff: 2n,
		secretShare: 0x5afe5afe5afen,
		validUntil: 1,
	},
	{
		id: "key_gen_confirm",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
	{
		id: "key_gen_confirm",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		callbackContext: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
	{
		id: "consensus_attest_transaction",
		epoch: 10n,
		transactionHash: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
	{
		id: "consensus_stage_epoch",
		proposedEpoch: 10n,
		rolloverBlock: 30n,
		groupId: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
];

describe("SqliteActionQueue", () => {
	it("should store all actions and return in correct order", () => {
		const storage = new SqliteActionQueue(":memory:");

		expect(storage.peek()).toBeUndefined();
		for (const action of actions) {
			storage.push(action);
		}
		for (const action of actions) {
			expect(storage.peek()).toStrictEqual(action);
			expect(storage.pop()).toStrictEqual(action);
		}
		expect(storage.peek()).toBeUndefined();
	});
});
