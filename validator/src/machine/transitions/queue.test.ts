import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import { toPoint } from "../../frost/math.js";
import { SqliteTransitionQueue } from "./queue.js";
import type { StateTransition } from "./types.js";

const TEST_POINT = toPoint({
	x: 105587021125387004117772930966558154492652686110919450580386247155506502192059n,
	y: 97790146336079427917878178932139533907352200097479391118658154349645214584696n,
});

const actions: StateTransition[] = [
	{
		id: "block_new",
		block: 111n,
	},
	{
		id: "event_key_gen",
		block: 111n,
		index: 0,
		gid: "0x5afe",
		participants: "0x5afe5afe",
		count: 4n,
		threshold: 3n,
		context: "0x5afecc",
	},
	{
		id: "event_key_gen_committed",
		block: 111n,
		index: 0,
		gid: "0x5afe",
		identifier: 1n,
		commitment: {
			r: TEST_POINT,
			mu: 123n,
			c: [TEST_POINT, TEST_POINT],
		},
		committed: true,
	},
	{
		id: "event_key_gen_secret_shared",
		block: 111n,
		index: 0,
		gid: "0x5afe",
		identifier: 1n,
		share: {
			y: TEST_POINT,
			f: [1n, 2n, 3n, 5n, 8n],
		},
		completed: true,
	},
	{
		id: "event_key_gen_confirmed",
		block: 111n,
		index: 0,
		gid: "0x5afe",
		identifier: 1n,
	},
	{
		id: "event_nonce_commitments_hash",
		block: 111n,
		index: 0,
		gid: "0x5afe",
		identifier: 1n,
		chunk: 100n,
		commitment: "0x5afeaabb",
	},
	{
		id: "event_sign_request",
		block: 111n,
		index: 0,
		initiator: zeroAddress,
		gid: "0x5afe",
		sid: "0x5af3",
		message: "0x5afeaabbcc",
		sequence: 23n,
	},
	{
		id: "event_nonce_commitments",
		block: 111n,
		index: 0,
		sid: "0x5af3",
		identifier: 1n,
		nonces: {
			d: TEST_POINT,
			e: TEST_POINT,
		},
	},
	{
		id: "event_signature_share",
		block: 111n,
		index: 0,
		sid: "0x5af3",
		identifier: 1n,
		z: 12345n,
	},
	{
		id: "event_signed",
		block: 111n,
		index: 0,
		sid: "0x5af3",
		signature: {
			z: 12345n,
			r: TEST_POINT,
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
	{
		id: "event_epoch_staged",
		block: 111n,
		index: 0,
		activeEpoch: 1n,
		proposedEpoch: 2n,
		rolloverBlock: 3n,
		groupKey: TEST_POINT,
	},
	{
		id: "event_transaction_proposed",
		block: 111n,
		index: 0,
		message: "0x5af333",
		transactionHash: "0x5af3aabbcc",
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
	{
		id: "event_transaction_attested",
		block: 111n,
		index: 0,
		message: "0x5af333",
	},
];

describe("SqliteActionQueue", () => {
	it("should store all actions and return in correct order", () => {
		const storage = new SqliteTransitionQueue(":memory:");

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
