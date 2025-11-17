import { randomBytes } from "node:crypto";
import { bytesToNumberBE } from "@noble/curves/utils.js";
import { createPublicClient, createWalletClient, http, parseAbi } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { anvil } from "viem/chains";
import { describe, expect, it } from "vitest";
import { OnchainCoordinator } from "./coordinator.js";
import { linkClientToCoordinator } from "./events.js";
import { KeyGenClient } from "./keyGen/client.js";
import { calculateParticipantsRoot } from "./merkle.js";
import { InMemoryStorage } from "./storage.js";
import type { Participant } from "./types.js";

// --- Tests ---
describe("integration", () => {
	it.skip("keygen and signing flow", { timeout: 30000 }, async () => {
		// Make sure to first start the Anvil testnode (run `anvil` in the root)
		// and run the deployment script: forge script DeployScript --rpc-url http://127.0.0.1:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 --broadcast
		// Private key from Anvil testnet
		const accounts = [
			privateKeyToAccount(
				"0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
			),
			privateKeyToAccount(
				"0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
			),
			privateKeyToAccount(
				"0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6",
			),
		];
		const participants: Participant[] = accounts.map((a, i) => {
			return { id: BigInt(i + 1), address: a.address };
		});
		const coordinatorAddress = "0x03f1bc3eF969F18C33cAC917ED453068aEee2Db5";
		const clients = accounts.map((a) => {
			const publicClient = createPublicClient({
				chain: anvil,
				transport: http(),
				pollingInterval: 500,
			});
			const signingClient = createWalletClient({
				chain: anvil,
				transport: http(),
				account: a,
			});
			const coordinator = new OnchainCoordinator(
				publicClient,
				signingClient,
				coordinatorAddress,
			);
			const storage = new InMemoryStorage(a.address);
			const client = new KeyGenClient(storage, coordinator);
			linkClientToCoordinator(client, publicClient, coordinatorAddress);
			client.registerParticipants(participants);
			return client;
		});

		const initiatorClient = createWalletClient({
			chain: anvil,
			transport: http(),
			account: accounts[0],
		});
		const abi = parseAbi([
			"function groupKey(bytes32 id) external view returns ((uint256 x, uint256 y) memory key)",
			"function keyGen(uint64 domain, bytes32 participants, uint64 count, uint64 threshold) external",
		]);
		await initiatorClient.writeContract({
			address: coordinatorAddress,
			abi: abi,
			functionName: "keyGen",
			args: [
				bytesToNumberBE(randomBytes(8)),
				calculateParticipantsRoot(participants),
				BigInt(accounts.length),
				BigInt(Math.ceil(accounts.length / 2)),
			],
		});
		await new Promise((resolve) => setTimeout(resolve, 10000));

		const readClient = createPublicClient({
			chain: anvil,
			transport: http(),
		});
		for (const c of clients) {
			const knownGroups = c.knownGroups();
			expect(knownGroups.length).toBeGreaterThan(0);
			for (const groupId of knownGroups) {
				const groupKey = await readClient.readContract({
					address: coordinatorAddress,
					abi: abi,
					functionName: "groupKey",
					args: [groupId],
				});
				const localGroupKey = c.groupPublicKey(groupId);
				expect(localGroupKey !== undefined).toBeTruthy();
				expect(localGroupKey?.x).toBe(groupKey.x);
				expect(localGroupKey?.y).toBe(groupKey.y);
			}
		}
	});
});
