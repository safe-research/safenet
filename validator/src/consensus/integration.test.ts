import { randomBytes } from "node:crypto";
import { bytesToNumberBE } from "@noble/curves/utils.js";
import {
	createPublicClient,
	createWalletClient,
	type Hex,
	http,
	keccak256,
	parseAbi,
	parseAbiItem,
	stringToBytes,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { anvil } from "viem/chains";
import { describe, expect, it } from "vitest";
import { toPoint } from "../frost/math.js";
import type { GroupId } from "../frost/types.js";
import { OnchainCoordinator } from "./coordinator.js";
import {
	linkKeyGenClientToCoordinator,
	linkSigningClientToCoordinator,
} from "./events.js";
import { KeyGenClient } from "./keyGen/client.js";
import { calculateParticipantsRoot } from "./merkle.js";
import { SigningClient } from "./signing/client.js";
import { verifySignature } from "./signing/verify.js";
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
		const coordinatorAddress = "0xC7756D613DdFD54c051BFbb7A3f5cD5a16448A14";
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
			const sc = new SigningClient(storage, coordinator, {
				onRequestSigned: (signatureId, participantId, message) => {
					console.log(
						`Participant ${participantId} signed request ${signatureId} for ${message}`,
					);
				},
			});
			const kc = new KeyGenClient(storage, coordinator, {
				onGroupSetup: (groupId, participantId) => {
					console.log(
						`Participant ${participantId} is setup for group ${groupId}`,
					);
					sc.commitNonces(groupId).catch(console.error);
				},
			});
			linkKeyGenClientToCoordinator(kc, publicClient, coordinatorAddress);
			linkSigningClientToCoordinator(sc, publicClient, coordinatorAddress);
			kc.registerParticipants(participants);
			return {
				kc,
				sc,
			};
		});

		const initiatorClient = createWalletClient({
			chain: anvil,
			transport: http(),
			account: accounts[0],
		});
		const abi = parseAbi([
			"function groupKey(bytes32 id) external view returns ((uint256 x, uint256 y) memory key)",
			"function keyGen(uint64 domain, bytes32 participants, uint64 count, uint64 threshold) external",
			"function sign(bytes32 gid, bytes32 message) external returns (bytes32 sid)",
			"function groupSignature(bytes32 sid, bytes32 root) external view returns ((uint256 x, uint256 y) memory r, uint256 z)",
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
		const groups: Set<GroupId> = new Set();
		for (const { kc } of clients) {
			const knownGroups = kc.knownGroups();
			expect(knownGroups.length).toBe(1);
			for (const groupId of knownGroups) {
				const groupKey = await readClient.readContract({
					address: coordinatorAddress,
					abi: abi,
					functionName: "groupKey",
					args: [groupId],
				});
				const localGroupKey = kc.groupPublicKey(groupId);
				expect(localGroupKey !== undefined).toBeTruthy();
				expect(localGroupKey?.x).toBe(groupKey.x);
				expect(localGroupKey?.y).toBe(groupKey.y);
				groups.add(groupId);
			}
		}

		expect(groups.size).toBe(1);
		const message = keccak256(stringToBytes("Hello, Shieldnet!"));
		for (const groupId of groups) {
			await initiatorClient.writeContract({
				address: coordinatorAddress,
				abi: abi,
				functionName: "sign",
				args: [groupId, message],
			});
		}
		await new Promise((resolve) => setTimeout(resolve, 10000));
		for (const groupId of groups) {
			const signEvents = await readClient.getLogs({
				address: coordinatorAddress,
				event: parseAbiItem(
					"event Sign(bytes32 indexed gid, bytes32 sid, bytes32 indexed message, uint256 sequence)",
				),
				args: {
					gid: groupId,
					message: message,
				},
				fromBlock: "earliest",
				toBlock: "latest",
			});
			expect(signEvents.length).toBe(1);
			const sid = signEvents[0].args.sid;
			expect(sid).toBeDefined();
			const signShareEvent = await readClient.getLogs({
				address: coordinatorAddress,
				event: parseAbiItem(
					"event SignShare(bytes32 indexed sid, uint256 identifier, uint256 z, bytes32 signersRoot)",
				),
				args: {
					sid,
				},
				fromBlock: "earliest",
				toBlock: "latest",
			});
			expect(signShareEvent.length).toBeGreaterThan(0);
			const signersRoot = signShareEvent[0].args.signersRoot;
			expect(signersRoot).toBeDefined();
			const groupKey = await readClient.readContract({
				address: coordinatorAddress,
				abi: abi,
				functionName: "groupKey",
				args: [groupId],
			});
			const groupSignaure = await readClient.readContract({
				address: coordinatorAddress,
				abi: abi,
				functionName: "groupSignature",
				args: [sid as Hex, signersRoot as Hex],
			});
			expect(
				verifySignature(
					toPoint(groupSignaure[0]),
					groupSignaure[1],
					toPoint(groupKey),
					message,
				),
			).toBeTruthy();
		}
	});
});
