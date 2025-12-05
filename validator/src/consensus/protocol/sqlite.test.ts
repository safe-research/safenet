import { describe, expect, it } from "vitest";
import { toPoint } from "../../frost/math.js";
import { SqliteActionQueue } from "./sqlite.js";
import type { ActionWithRetry } from "./types.js";

const TEST_POINT = toPoint({
	x: 105587021125387004117772930966558154492652686110919450580386247155506502192059n,
	y: 97790146336079427917878178932139533907352200097479391118658154349645214584696n,
});

const actions: ActionWithRetry[] = [
	{
		id: "sign_request",
		groupId: "0x5afe",
		message: "0x5afe5afe",
		retryCount: 0,
	},
	{
		id: "sign_register_nonce_commitments",
		groupId: "0x5afe",
		nonceCommitmentsHash: "0x5afe5afe",
		retryCount: 0,
	},
	{
		id: "sign_reveal_nonce_commitments",
		signatureId: "0x5afe",
		nonceCommitments: {
			bindingNonceCommitment: TEST_POINT,
			hidingNonceCommitment: TEST_POINT,
		},
		nonceProof: ["0x5afe01", "0x5afe02"],
		retryCount: 1,
	},
	{
		id: "sign_publish_signature_share",
		signatureId: "0x5afe",
		signersRoot: "0x5afe00",
		signersProof: ["0x5afe01", "0x5afe02"],
		groupCommitment: TEST_POINT,
		commitmentShare: TEST_POINT,
		signatureShare: 1n,
		lagrangeCoefficient: 2n,
		retryCount: 1,
	},
	{
		id: "sign_publish_signature_share",
		signatureId: "0x5afe",
		signersRoot: "0x5afe00",
		signersProof: ["0x5afe01", "0x5afe02"],
		groupCommitment: TEST_POINT,
		commitmentShare: TEST_POINT,
		signatureShare: 1n,
		lagrangeCoefficient: 2n,
		callbackContext: "0x5afe00aa",
		retryCount: 1,
	},
	{
		id: "key_gen_start",
		participants: "0x5afe",
		count: 4n,
		threshold: 3n,
		context: "0x5afe00aa",
		participantId: 1n,
		commitments: [TEST_POINT, TEST_POINT],
		pok: {
			r: TEST_POINT,
			mu: 5n,
		},
		poap: ["0x5afe01", "0x5afe02"],
		retryCount: 1,
	},
	{
		id: "key_gen_publish_secret_shares",
		groupId: "0x5afe",
		verificationShare: TEST_POINT,
		shares: [1n, 2n, 3n, 5n, 8n, 13n],
		retryCount: 1,
	},
	{
		id: "key_gen_confirm",
		groupId: "0x5afe",
		retryCount: 1,
	},
	{
		id: "key_gen_confirm",
		groupId: "0x5afe",
		callbackContext: "0x5afe00aa",
		retryCount: 1,
	},
	{
		id: "consensus_attest_transaction",
		epoch: 10n,
		transactionHash: "0x5afe00aa",
		signatureId: "0x5afe",
		retryCount: 1,
	},
	{
		id: "consensus_stage_epoch",
		proposedEpoch: 10n,
		rolloverBlock: 30n,
		groupId: "0x5afe00aa",
		signatureId: "0x5afe",
		retryCount: 1,
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
