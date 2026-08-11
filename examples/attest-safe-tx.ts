/**
 * Build a SafenetGuard attestation for an existing Safe transaction.
 *
 * SafenetGuard verifies an inline attestation *trailer* appended to the Safe `signatures` at
 * `execTransaction` time — it does not post anything to the Safe Transaction Service. This
 * script therefore:
 *
 *   1. fetches the FROST attestation for a Safe tx hash from the Consensus contract (Gnosis),
 *   2. resolves the attesting group key from the guard's own epoch events (Consensus exposes no
 *      group-key getter — the key is only emitted in events),
 *   3. fetches the Safe transaction and its collected owner confirmations from the Tx Service,
 *   4. assembles `signatures = <sorted owner signatures> || <payload || uint256(payloadLength) || TYPE_HASH>`, and
 *   5. prints that blob and the `execTransaction` parameters, ready for a relayer to submit.
 *
 * Both the attestation and the guard's epoch events are read over a single `RPC_URL`, so this example
 * assumes the guard is deployed on Gnosis alongside Consensus. For a guard on another chain, read its
 * epoch events from that chain's RPC.
 *
 * Usage:
 *   npm run attest-safe-tx -w @safenet/examples -- <safeTxHash> <guardAddress>
 *
 * Environment (copy examples/.env.sample to examples/.env):
 *   CONSENSUS_ADDRESS, ORACLE_ADDRESS, RPC_URL, SAFE_TX_SERVICE_URL, SAFE_TX_SERVICE_API_KEY
 */

import { resolve } from "node:path";
import dotenv from "dotenv";
import {
	type Address,
	concat,
	createPublicClient,
	encodeAbiParameters,
	getAddress,
	type Hex,
	http,
	isAddress,
	isHex,
	keccak256,
	numberToHex,
	parseAbiItem,
	size,
	toBytes,
} from "viem";
import { gnosis } from "viem/chains";
import z from "zod";

dotenv.config({ path: resolve(import.meta.dirname, ".env"), quiet: true });

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Terminal marker of a v1 attestation trailer (AttestationTrailer.TYPE_HASH).
const TYPE_HASH: Hex = keccak256(toBytes("SafenetGuard.AttestationTrailer.v1"));

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

function usage(): never {
	console.error("Usage: npm run attest-safe-tx -w @safenet/examples -- <safeTxHash> <guardAddress>");
	process.exit(1);
}

const args = process.argv.slice(2);
if (args.length !== 2) usage();
const [rawHash = "", rawGuard = ""] = args;
if (!isHex(rawHash) || size(rawHash) !== 32) usage();
if (!isAddress(rawGuard)) usage();

const safeTxHash: Hex = rawHash;
const guardAddress: Address = getAddress(rawGuard);

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

const envSchema = z.object({
	CONSENSUS_ADDRESS: z
		.string()
		.refine((a) => isAddress(a, { strict: false }), "Invalid address format")
		.transform((a) => getAddress(a)),
	ORACLE_ADDRESS: z
		.string()
		.refine((a) => isAddress(a, { strict: false }), "Invalid address format")
		.transform((a) => getAddress(a)),
	RPC_URL: z.url(),
	SAFE_TX_SERVICE_URL: z.url().transform((url) => url.replace(/\/$/, "")),
	SAFE_TX_SERVICE_API_KEY: z.string().min(1),
	GUARD_FROM_BLOCK: z
		.string()
		.optional()
		.transform((v) => BigInt(v ?? "0")),
	ATTESTATION_TIMEOUT_SECONDS: z
		.string()
		.optional()
		.transform((v) => Number.parseInt(v ?? "120"))
		.pipe(z.number().int().positive()),
});

const envParseResult = envSchema.safeParse(process.env);
if (!envParseResult.success) {
	console.error("Configuration error:", envParseResult.error.message);
	process.exit(1);
}

const {
	CONSENSUS_ADDRESS: consensusAddress,
	ORACLE_ADDRESS: oracleAddress,
	RPC_URL: rpc,
	SAFE_TX_SERVICE_URL: safeTxServiceUrl,
	SAFE_TX_SERVICE_API_KEY: safeTxServiceApiKey,
	GUARD_FROM_BLOCK: guardFromBlock,
	ATTESTATION_TIMEOUT_SECONDS: attestationTimeout,
} = envParseResult.data;

const authHeaders = { Authorization: `Bearer ${safeTxServiceApiKey}` };
const gnosisClient = createPublicClient({ chain: gnosis, transport: http(rpc) });

// ---------------------------------------------------------------------------
// ABIs
// ---------------------------------------------------------------------------

const GET_ACTIVE_EPOCH = parseAbiItem("function getActiveEpoch() view returns (uint64 epoch, bytes32 groupId)");

const GET_ATTESTATION = parseAbiItem(
	"function getOracleTransactionAttestationByHash(uint64, address, bytes32) view returns (((uint256 x, uint256 y) r, uint256 z) signature)",
);

// The getter reverts NotSigned() until the FROST round completes; declaring the error lets viem decode
// it so the poll loop can tell "not signed yet" apart from a genuine failure.
const NOT_SIGNED = parseAbiItem("error NotSigned()");

// The guard mirrors EpochRollover events; the group key is only available from these logs.
const EPOCH_INITIALIZED = parseAbiItem("event EpochInitialized(uint64 indexed epoch, (uint256 x, uint256 y) groupKey)");
const EPOCH_ROLLED_OVER = parseAbiItem(
	"event EpochRolledOver(uint64 indexed parentEpoch, uint64 indexed epoch, (uint256 x, uint256 y) parentKey, (uint256 x, uint256 y) groupKey)",
);

// ---------------------------------------------------------------------------
// Safe TX Service response schema
// ---------------------------------------------------------------------------

const txSchema = z.object({
	safe: z
		.string()
		.refine((a) => isAddress(a, { strict: false }), "Invalid address")
		.transform((a) => getAddress(a)),
	to: z.string(),
	value: z.string(),
	data: z.string().nullable(),
	operation: z.coerce.number().int(),
	safeTxGas: z.string(),
	baseGas: z.string(),
	gasPrice: z.string(),
	gasToken: z.string(),
	refundReceiver: z.string(),
	nonce: z.coerce.number().int(),
	confirmations: z
		.array(
			z.object({
				owner: z.string().refine((a) => isAddress(a, { strict: false }), "Invalid owner"),
				signature: z.string().refine((s) => isHex(s), "Invalid signature"),
			}),
		)
		.default([]),
});

type Point = { x: bigint; y: bigint };

// ---------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------

async function pollAttestation(): Promise<{ epoch: bigint; sig: { r: Point; z: bigint } }> {
	console.log(`[1] Polling Consensus for attestation (timeout: ${attestationTimeout}s)...`);
	const [epoch] = await gnosisClient.readContract({
		address: consensusAddress,
		abi: [GET_ACTIVE_EPOCH],
		functionName: "getActiveEpoch",
	});
	const deadline = Date.now() + attestationTimeout * 1000;
	while (Date.now() < deadline) {
		try {
			const sig = await gnosisClient.readContract({
				address: consensusAddress,
				abi: [GET_ATTESTATION, NOT_SIGNED],
				functionName: "getOracleTransactionAttestationByHash",
				args: [epoch, oracleAddress, safeTxHash],
			});
			if (sig.r.x !== 0n || sig.r.y !== 0n || sig.z !== 0n) {
				console.log(`\n    attestation received (epoch=${epoch})`);
				return { epoch, sig };
			}
		} catch (e: unknown) {
			if (!(e instanceof Error && e.message.includes("NotSigned"))) throw e;
		}
		process.stdout.write(".");
		await new Promise((r) => setTimeout(r, 5000));
	}
	throw new Error(`Attestation timeout after ${attestationTimeout}s`);
}

async function resolveGroupKey(epoch: bigint): Promise<Point> {
	console.log(`[2] Resolving group key for epoch ${epoch} from the guard's epoch events...`);
	const rolled = await gnosisClient.getLogs({
		address: guardAddress,
		event: EPOCH_ROLLED_OVER,
		args: { epoch },
		fromBlock: guardFromBlock,
		toBlock: "latest",
	});
	const rolledKey = rolled[0]?.args.groupKey;
	if (rolledKey) return rolledKey;

	const initialized = await gnosisClient.getLogs({
		address: guardAddress,
		event: EPOCH_INITIALIZED,
		args: { epoch },
		fromBlock: guardFromBlock,
		toBlock: "latest",
	});
	const initKey = initialized[0]?.args.groupKey;
	if (initKey) return initKey;

	throw new Error(`No EpochInitialized/EpochRolledOver event for epoch ${epoch} on guard ${guardAddress}`);
}

async function fetchTransaction() {
	console.log("[3] Fetching transaction + owner confirmations from the Safe TX Service...");
	const res = await fetch(`${safeTxServiceUrl}/api/v2/multisig-transactions/${safeTxHash}/`, {
		headers: authHeaders,
	});
	if (!res.ok) throw new Error(`Safe TX Service GET failed (${res.status}): ${await res.text()}`);
	return txSchema.parse(await res.json());
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
	const { epoch, sig } = await pollAttestation();
	const groupKey = await resolveGroupKey(epoch);
	const tx = await fetchTransaction();

	// Owner signatures must be concatenated in ascending owner-address order (Safe requirement).
	const ownerSignatures = concat(
		[...tx.confirmations]
			.sort((a, b) => (a.owner.toLowerCase() < b.owner.toLowerCase() ? -1 : 1))
			.map((c) => c.signature as Hex),
	);

	// Signature extension = payload || uint256(payloadLength) || TYPE_HASH,
	// where payload = abi.encode(uint64 epoch, address oracle, Point groupKey, FROST.Signature sig).
	const payload = encodeAbiParameters(
		[
			{ type: "uint64" },
			{ type: "address" },
			{
				type: "tuple",
				components: [
					{ type: "uint256", name: "x" },
					{ type: "uint256", name: "y" },
				],
			},
			{
				type: "tuple",
				components: [
					{
						type: "tuple",
						name: "r",
						components: [
							{ type: "uint256", name: "x" },
							{ type: "uint256", name: "y" },
						],
					},
					{ type: "uint256", name: "z" },
				],
			},
		],
		[epoch, oracleAddress, groupKey, sig],
	);
	const trailer = concat([payload, numberToHex(size(payload), { size: 32 }), TYPE_HASH]);
	const signatures = concat([ownerSignatures, trailer]);

	console.log("\n[4] Attested signatures ready. Submit via execTransaction on the Safe's chain:");
	console.log(`    safe:            ${tx.safe}`);
	console.log(`    to:              ${tx.to}`);
	console.log(`    value:           ${tx.value}`);
	console.log(`    data:            ${tx.data ?? "0x"}`);
	console.log(`    operation:       ${tx.operation}`);
	console.log(`    safeTxGas:       ${tx.safeTxGas}`);
	console.log(`    baseGas:         ${tx.baseGas}`);
	console.log(`    gasPrice:        ${tx.gasPrice}`);
	console.log(`    gasToken:        ${tx.gasToken}`);
	console.log(`    refundReceiver:  ${tx.refundReceiver}`);
	console.log(`    signatures:      ${signatures}`);
}

main().catch((e: unknown) => {
	console.error("Error:", e instanceof Error ? e.message : e);
	process.exit(1);
});
