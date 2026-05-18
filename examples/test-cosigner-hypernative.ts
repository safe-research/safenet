/**
 * End-to-end integration test: Safenet Cosigner + Hypernative Guard on Sepolia.
 *
 * Handles both setup (Phase 0) and the integration test (Phase 1).
 * All setup steps are idempotent — safe to run multiple times.
 *
 * Phase 0 — Deploy and configure (skipped if already in place):
 *   0.1  Fetch active epoch and group key from Gnosis Chain
 *   0.2  Deploy SafenetCosigner via Safe Singleton Factory
 *   0.3  Create Safe proxy (single owner, threshold 1 initially)
 *   0.4  Deploy HypernativeGuard via Safe Singleton Factory
 *   0.5  Atomic Safe configuration via MultiSendCallOnly (DelegateCall):
 *          setGuard(guard) + addOwnerWithThreshold(cosigner, 2)
 *   0.6  Activate HypernativeGuard enforcement (disablePassThroughMode)
 *
 * Phase 1 — Integration test:
 *   Proposes a no-op transaction through Safenet, collects attestation,
 *   assembles the full 387-byte signatures blob, and executes on-chain.
 *
 * Environment setup:
 *   Copy examples/.env.sample to examples/.env and fill in the values.
 *   The script loads examples/.env automatically via dotenv.
 *
 * Required env vars:
 *   SEPOLIA_RPC_URL, GNOSIS_RPC_URL, CONSENSUS_ADDRESS
 *   OWNER1_ADDRESS, OWNER1_ACCOUNT, OWNER2_ADDRESS
 *   KEEPER_ADDRESS, KEEPER_ACCOUNT, DEPLOYER_ACCOUNT
 *   SAFE_TX_SERVICE_URL, SAFE_TX_SERVICE_API_KEY
 *
 * Optional env vars:
 *   CONSENSUS_CHAIN_ID          (default: 100)
 *   ALLOW_TX_DELAY              (default: 60 s)
 *   COSIGNER_SALT               (default: bytes32(0))
 *   GUARD_SALT                  (default: bytes32(0))
 *   SAFE_CREATION_SALT_NONCE    (default: 0)
 *   ATTESTATION_TIMEOUT_SECONDS (default: 120)
 */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import dotenv from "dotenv";
import {
	type Address,
	concat,
	createPublicClient,
	decodeAbiParameters,
	encodeAbiParameters,
	encodeFunctionData,
	encodePacked,
	getAddress,
	getCreate2Address,
	type Hex,
	http,
	keccak256,
	pad,
	parseAbi,
	zeroAddress,
	zeroHash,
} from "viem";
import { gnosis, sepolia } from "viem/chains";
import { HYPERNATIVE_GUARD_CREATION_CODE } from "./hypernative-guard-bytecode.js";

dotenv.config({ path: resolve(import.meta.dirname, ".env"), quiet: true });

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SAFE_SINGLETON_FACTORY = "0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7" as Address;
const SAFE_SINGLETON = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762" as Address; // Safe 1.4.1
const SAFE_PROXY_FACTORY = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67" as Address; // Safe 1.4.1
// getGuard() is internal in Safe 1.4.1; the guard address must be read via getStorageAt(slot, 1).
// Slot value from contracts/lib/safe-smart-account/contracts/libraries/SafeStorage.sol.
const GUARD_STORAGE_SLOT = 0x4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c8n;
// keccak256("KEEPER_ROLE") — matches HypernativeGuard.KEEPER_ROLE
const KEEPER_ROLE = "0xfc8737ab85eb45125971625a9ebdb75cc78e01d5c1fa80c4c6e5203f47bc4fab" as Hex;
const ZERO_SALT = zeroHash;

// ---------------------------------------------------------------------------
// Explorer URL helpers
// ---------------------------------------------------------------------------

const sepoliaAddress = (addr: Address): string => `https://sepolia.etherscan.io/address/${addr}`;
const sepoliaTx = (hash: Hex): string => `https://sepolia.etherscan.io/tx/${hash}/advanced`;
const gnosisAddress = (addr: Address): string => `https://gnosisscan.io/address/${addr}`;
const gnosisTx = (hash: Hex): string => `https://gnosisscan.io/tx/${hash}`;
const tenderlyTx = (hash: Hex): string => `https://dashboard.tenderly.co/tx/${hash}`;
const safeUiLink = (addr: Address): string => `https://app.safe.global/home?safe=sep:${addr}`;
const safenetTx = (hash: Hex): string =>
	`https://explorer.safenet-beta.eth.limo/#/safeTx?chainId=${sepolia.id}&safeTxHash=${hash}`;


// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function requireEnv(name: string): string {
	const v = process.env[name];
	if (!v) throw new Error(`Missing required env var: ${name}`);
	return v;
}

function optionalEnv(name: string, defaultValue: string): string {
	return process.env[name] ?? defaultValue;
}

/** Sign a hash using a forge keystore account via `cast wallet sign`. */
function castSign(account: string, hash: Hex): Hex {
	process.stdout.write(`  Sign with ${account} account\n`);
	return execFileSync("cast", ["wallet", "sign", "--no-hash", "--account", account, hash], {
		encoding: "utf8",
		stdio: ["inherit", "pipe", "inherit"],
	}).trim() as Hex;
}

/** Broadcast a transaction via `cast send` using a forge keystore account. */
function castSend(account: string, rpcUrl: string, to: string, sig: string, ...args: string[]): string {
	process.stdout.write(`  Sign with ${account} account\n`);
	return execFileSync("cast", ["send", "--account", account, "--rpc-url", rpcUrl, to, sig, ...args], {
		encoding: "utf8",
		stdio: ["inherit", "pipe", "inherit"],
	});
}

/** Broadcast a raw-data transaction (used for Safe Singleton Factory deployments). */
function castSendRaw(account: string, rpcUrl: string, to: string, data: Hex): string {
	process.stdout.write(`  Sign with ${account} account\n`);
	return execFileSync("cast", ["send", "--account", account, "--rpc-url", rpcUrl, to, data], {
		encoding: "utf8",
		stdio: ["inherit", "pipe", "inherit"],
	});
}

/** Strip 0x and verify a 65-byte ECDSA signature. */
function sigToBytes(sig: Hex): string {
	const bytes = sig.startsWith("0x") ? sig.slice(2) : sig;
	if (bytes.length !== 130) throw new Error(`Expected 65-byte sig, got ${bytes.length / 2} bytes`);
	return bytes;
}

/**
 * Walk the viem/RPC error cause chain and return the first hex revert data found.
 * viem wraps execution reverts in nested BaseError objects; the raw data lives on
 * an inner cause (typically ExecutionRevertedError) as a `data` string.
 */
function extractRevertData(err: unknown): Hex {
	let current: unknown = err;
	while (current !== null && current !== undefined) {
		if (typeof current === "object") {
			const data = (current as Record<string, unknown>).data;
			if (typeof data === "string" && data.startsWith("0x")) return data as Hex;
		}
		current = (current as { cause?: unknown }).cause;
	}
	return "0x";
}

/** Log the raw viem error chain as JSON for deep debugging. */
function dumpErrorChain(err: unknown): void {
	const seen = new WeakSet();
	function sanitize(v: unknown, depth: number): unknown {
		if (depth > 6 || v === null || v === undefined) return v;
		if (typeof v !== "object") return v;
		if (seen.has(v as object)) return "[circular]";
		seen.add(v as object);
		const out: Record<string, unknown> = {};
		for (const k of ["message", "shortMessage", "name", "code", "data", "details", "cause"] as const) {
			const val = (v as Record<string, unknown>)[k];
			if (val !== undefined) out[k] = sanitize(val, depth + 1);
		}
		return out;
	}
	console.error("   [error chain]", JSON.stringify(sanitize(err, 0), null, 2));
}

/** Decode and log a revert reason from raw ABI-encoded error bytes. */
function logRevertReason(raw: Hex): void {
	if (raw === "0x") {
		console.error("   => No revert data (RPC did not return reason bytes — check debug_traceCall output below)");
		return;
	}
	const sel = raw.slice(0, 10);
	const known: Record<string, string> = {
		"0x08c379a0": "Error(string) — Safe GS0xx or require(false, string)",
		"0x4e487b71": "Panic(uint256) — assertion / overflow / out-of-bounds",
		"0x70cc6907": "UnapprovedHash() — HypernativeGuard: keeper sig invalid or hash not approved",
		"0x6c9652e2": "InvalidEpoch() — SafenetCosigner: epoch not current or previous",
		"0x5f15d672": "InvalidScalar() — FROST: z >= N",
		"0xa5a2f839": "InvalidMulMulAddWitness() — FROST: ecrecover witness mismatch",
		"0x894e13bc": "NotOnCurve() — Secp256k1: point not on curve",
	};
	const name = known[sel] ?? `unknown selector ${sel}`;
	console.error(`   => ${name}`);
	if (sel === "0x08c379a0" && raw.length > 10) {
		try {
			const [str] = decodeAbiParameters([{ type: "string" }], `0x${raw.slice(10)}` as Hex);
			console.error(`   => Message: "${str}"`);
		} catch {
			/* ignore decode failure */
		}
	}
}

/** Read compiled bytecode from a Foundry JSON artifact (path relative to repo root). */
function readBytecode(pathFromRoot: string): Hex {
	const fullPath = resolve(process.cwd(), pathFromRoot);
	let artifact: { bytecode: { object: string } };
	try {
		artifact = JSON.parse(readFileSync(fullPath, "utf8"));
	} catch {
		throw new Error(`Could not read artifact at ${fullPath}. Run \`forge build\` in contracts/ first.`);
	}
	const bc = artifact.bytecode.object;
	return (bc.startsWith("0x") ? bc : `0x${bc}`) as Hex;
}

/** Compute a CREATE2 address using the Safe Singleton Factory formula. */
function computeSingletonFactoryAddress(salt: Hex, creationCode: Hex, constructorArgs: Hex): Address {
	return getCreate2Address({
		from: SAFE_SINGLETON_FACTORY,
		salt,
		bytecodeHash: keccak256(concat([creationCode, constructorArgs])),
	});
}

/**
 * Assembles the `signatures` bytes for Safe.execTransaction.
 *
 * Layout (387 bytes):
 *   [owner1_static: 65]       standard ECDSA
 *   [cosigner_static: 65]     EIP-1271: r=address, s=130 (offset), v=0
 *   [attestation_length: 32]  = 128
 *   [attestation: 128]        abi.encode(uint64 epoch, FROST.Signature{r:{x,y}, z})
 *   [keeper_sig: 65]
 *   [context_length: 32]      = 0
 *
 * Static slots are ordered ascending by owner address as required by Safe.
 */
function assembleSafeSignatures(opts: {
	owner1Address: Address;
	owner1Sig: Hex;
	cosignerAddress: Address;
	attestation: Hex;
	keeperSig: Hex;
}): Hex {
	const { owner1Address, owner1Sig, cosignerAddress, attestation, keeperSig } = opts;

	const owner1Static = sigToBytes(owner1Sig);
	const cosignerStatic = [
		cosignerAddress.toLowerCase().slice(2).padStart(64, "0"),
		(130).toString(16).padStart(64, "0"),
		"00",
	].join("");

	const attestationBytes = attestation.slice(2);
	if (attestationBytes.length !== 256)
		throw new Error(`Expected 128-byte attestation, got ${attestationBytes.length / 2} bytes`);
	const dynamicData = (128).toString(16).padStart(64, "0") + attestationBytes;

	const keeperTrailer = sigToBytes(keeperSig) + (0).toString(16).padStart(64, "0");

	const cosignerFirst = BigInt(cosignerAddress) < BigInt(owner1Address);
	const staticSlots = cosignerFirst ? cosignerStatic + owner1Static : owner1Static + cosignerStatic;

	return `0x${staticSlots}${dynamicData}${keeperTrailer}` as Hex;
}

// ---------------------------------------------------------------------------
// ABIs
// ---------------------------------------------------------------------------

const SAFE_ABI = parseAbi([
	"function getTransactionHash(address,uint256,bytes calldata,uint8,uint256,uint256,uint256,address,address,uint256) external view returns (bytes32)",
	"function nonce() external view returns (uint256)",
	"function execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes) external payable returns (bool)",
	"function getStorageAt(uint256,uint256) external view returns (bytes)",
]);

const PROXY_FACTORY_ABI = parseAbi(["function proxyCreationCode() external pure returns (bytes memory)"]);

const CONSENSUS_ABI = parseAbi([
	"function getActiveEpoch() external view returns (uint64 epoch, bytes32 groupId)",
	"function getCoordinator() external view returns (address coordinator)",
	"function proposeBasicTransaction(uint256,address,address,uint256,bytes memory,uint256) external returns (bytes32)",
	"function getRecentTransactionAttestationByHash(bytes32) external view returns (uint64 epoch, ((uint256 x, uint256 y) r, uint256 z) signature)",
	// FROSTCoordinator.signatureValue reverts with this when no signing round has completed yet.
	"error NotSigned()",
]);

const COORDINATOR_ABI = parseAbi([
	"function groupKey(bytes32 gid) external view returns ((uint256 x, uint256 y) groupKey)",
]);

const COSIGNER_ABI = parseAbi([
	"function activeEpoch() external view returns (uint64)",
	"function previousEpoch() external view returns (uint64)",
	"function isValidSignature(bytes32 hash, bytes calldata signature) external view returns (bytes4)",
]);

const GUARD_ABI = parseAbi([
	"function isPassThroughMode() external view returns (bool)",
	"function disablePassThroughMode() external",
	"function hasRole(bytes32 role, address account) external view returns (bool)",
]);

// ---------------------------------------------------------------------------
// Phase 0 — Setup helpers
// ---------------------------------------------------------------------------

type PublicClient = ReturnType<typeof createPublicClient>;

async function fetchEpoch(
	gnosisClient: PublicClient,
	consensusAddress: Address,
): Promise<{ epoch: bigint; groupKeyX: bigint; groupKeyY: bigint }> {
	const [epoch, groupId] = await gnosisClient.readContract({
		address: consensusAddress,
		abi: CONSENSUS_ABI,
		functionName: "getActiveEpoch",
	});
	const coordinator = await gnosisClient.readContract({
		address: consensusAddress,
		abi: CONSENSUS_ABI,
		functionName: "getCoordinator",
	});
	const groupKey = await gnosisClient.readContract({
		address: coordinator,
		abi: COORDINATOR_ABI,
		functionName: "groupKey",
		args: [groupId],
	});
	return { epoch: BigInt(epoch), groupKeyX: groupKey.x, groupKeyY: groupKey.y };
}

async function deployCosigner(
	sepoliaClient: PublicClient,
	deployerAccount: string,
	rpcUrl: string,
	consensusAddress: Address,
	consensusChainId: bigint,
	epoch: bigint,
	groupKeyX: bigint,
	groupKeyY: bigint,
	allowTxDelay: bigint,
	salt: Hex,
): Promise<Address> {
	const creationCode = readBytecode("contracts/build/out/SafenetCosigner.sol/SafenetCosigner.json");
	const constructorArgs = encodeAbiParameters(
		[
			{ type: "uint256" },
			{ type: "address" },
			{ type: "uint64" },
			{
				type: "tuple",
				components: [
					{ type: "uint256", name: "x" },
					{ type: "uint256", name: "y" },
				],
			},
			{ type: "uint256" },
		],
		[consensusChainId, consensusAddress, epoch, { x: groupKeyX, y: groupKeyY }, allowTxDelay],
	);
	const address = computeSingletonFactoryAddress(salt, creationCode, constructorArgs);

	const code = await sepoliaClient.getCode({ address });
	if (code && code !== "0x") {
		console.log(`  already deployed at ${address}`);
		console.log(`  ${sepoliaAddress(address)}`);
		return address;
	}

	console.log(`  deploying (epoch ${epoch})...`);
	castSendRaw(deployerAccount, rpcUrl, SAFE_SINGLETON_FACTORY, concat([salt, creationCode, constructorArgs]));
	console.log(`  deployed at ${address}`);
	console.log(`  ${sepoliaAddress(address)}`);
	return address;
}

async function createSafe(
	sepoliaClient: PublicClient,
	deployerAccount: string,
	rpcUrl: string,
	owners: Address[],
	cosignerAddress: Address,
	saltNonce: bigint,
): Promise<Address> {
	const initializer = encodeFunctionData({
		abi: parseAbi(["function setup(address[],uint256,address,bytes,address,address,uint256,address) external"]),
		functionName: "setup",
		args: [[...owners, cosignerAddress], 2n, zeroAddress, "0x", zeroAddress, zeroAddress, 0n, zeroAddress],
	});

	const proxyCreationCode = await sepoliaClient.readContract({
		address: SAFE_PROXY_FACTORY,
		abi: PROXY_FACTORY_ABI,
		functionName: "proxyCreationCode",
	});

	// Matches SafeProxyFactory.createProxyWithNonce salt derivation:
	// salt = keccak256(abi.encodePacked(keccak256(initializer), saltNonce))
	const innerSalt = keccak256(encodePacked(["bytes32", "uint256"], [keccak256(initializer), saltNonce]));
	const deploymentData = concat([proxyCreationCode as Hex, pad(SAFE_SINGLETON, { size: 32 })]);
	const address = getCreate2Address({
		from: SAFE_PROXY_FACTORY,
		salt: innerSalt,
		bytecodeHash: keccak256(deploymentData),
	});

	const code = await sepoliaClient.getCode({ address });
	if (code && code !== "0x") {
		console.log(`  already deployed at ${address}`);
		console.log(`  ${sepoliaAddress(address)}`);
		return address;
	}

	console.log("  creating Safe proxy...");
	castSend(
		deployerAccount,
		rpcUrl,
		SAFE_PROXY_FACTORY,
		"createProxyWithNonce(address,bytes,uint256)",
		SAFE_SINGLETON,
		initializer,
		String(saltNonce),
	);
	console.log(`  deployed at ${address}`);
	console.log(`  ${sepoliaAddress(address)}`);
	return address;
}

async function deployGuard(
	sepoliaClient: PublicClient,
	deployerAccount: string,
	rpcUrl: string,
	safeAddress: Address,
	keeperAddress: Address,
	salt: Hex,
): Promise<Address> {
	const creationCode = HYPERNATIVE_GUARD_CREATION_CODE;

	const constructorArgs = encodeAbiParameters([{ type: "address" }, { type: "address" }], [safeAddress, keeperAddress]);
	const address = computeSingletonFactoryAddress(salt, creationCode, constructorArgs);

	const code = await sepoliaClient.getCode({ address });
	if (code && code !== "0x") {
		console.log(`  already deployed at ${address}`);
		console.log(`  ${sepoliaAddress(address)}`);
		return address;
	}

	console.log("  deploying HypernativeGuard...");
	castSendRaw(deployerAccount, rpcUrl, SAFE_SINGLETON_FACTORY, concat([salt, creationCode, constructorArgs]));
	console.log(`  deployed at ${address}`);
	console.log(`  ${sepoliaAddress(address)}`);
	return address;
}

async function configureSafe(
	sepoliaClient: PublicClient,
	owner1Account: string,
	owner1Address: Address,
	owner2Account: string,
	owner2Address: Address,
	rpcUrl: string,
	safeAddress: Address,
	guardAddress: Address,
): Promise<void> {
	// guardSlotData is a 32-byte EVM word; the guard address occupies the last 20 bytes.
	const guardSlotData = await sepoliaClient.readContract({
		address: safeAddress,
		abi: SAFE_ABI,
		functionName: "getStorageAt",
		args: [GUARD_STORAGE_SLOT, 1n],
	});
	const guard = getAddress(`0x${guardSlotData.slice(-40)}`);
	const guardSet = guard.toLowerCase() === guardAddress.toLowerCase();

	if (guardSet) {
		console.log("  already configured (guard set)");
		return;
	}

	// Safe was created with threshold=2; owner1 (executor, v=1) + owner2 (ECDSA) satisfy it.
	// No guard is active yet so no keeper signature is required.
	console.log("  calling setGuard...");
	const setGuardCalldata = encodeFunctionData({
		abi: parseAbi(["function setGuard(address)"]),
		functionName: "setGuard",
		args: [guardAddress],
	});

	const nonce = await sepoliaClient.readContract({ address: safeAddress, abi: SAFE_ABI, functionName: "nonce" });
	const txHash = await sepoliaClient.readContract({
		address: safeAddress,
		abi: SAFE_ABI,
		functionName: "getTransactionHash",
		args: [safeAddress, 0n, setGuardCalldata, 0, 0n, 0n, 0n, zeroAddress, zeroAddress, nonce],
	});

	// owner1 is msg.sender (executor): v=1, r=owner1Address, s=0.
	const owner1ApprovedSig = `${owner1Address.toLowerCase().slice(2).padStart(64, "0") + (0).toString(16).padStart(64, "0")}01`;
	const owner2Sig = sigToBytes(castSign(owner2Account, txHash));
	const [lowSig, highSig] =
		BigInt(owner1Address) < BigInt(owner2Address) ? [owner1ApprovedSig, owner2Sig] : [owner2Sig, owner1ApprovedSig];
	const sig: Hex = `0x${lowSig}${highSig}`;

	castSend(
		owner1Account,
		rpcUrl,
		safeAddress,
		"execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)",
		safeAddress,
		"0",
		setGuardCalldata,
		"0",
		"0",
		"0",
		"0",
		zeroAddress,
		zeroAddress,
		sig,
	);

	const nonceAfter = await sepoliaClient.readContract({ address: safeAddress, abi: SAFE_ABI, functionName: "nonce" });
	if (nonceAfter !== nonce + 1n) throw new Error("Safe configuration transaction failed (nonce did not advance)");

	console.log("  guard installed");
}

async function activateEnforcement(
	sepoliaClient: PublicClient,
	keeperAccount: string,
	rpcUrl: string,
	guardAddress: Address,
): Promise<void> {
	const isPassThrough = await sepoliaClient.readContract({
		address: guardAddress,
		abi: GUARD_ABI,
		functionName: "isPassThroughMode",
	});

	if (!isPassThrough) {
		console.log("  enforcement already active");
		return;
	}

	console.log("  calling disablePassThroughMode...");
	castSend(keeperAccount, rpcUrl, guardAddress, "disablePassThroughMode()");
	const stillPassThrough = await sepoliaClient.readContract({
		address: guardAddress,
		abi: GUARD_ABI,
		functionName: "isPassThroughMode",
	});
	if (stillPassThrough) throw new Error("disablePassThroughMode transaction did not take effect");
	console.log("  enforcement active");
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
	const sepoliaRpc = requireEnv("SEPOLIA_RPC_URL");
	const gnosisRpc = requireEnv("GNOSIS_RPC_URL");
	const consensusAddress = requireEnv("CONSENSUS_ADDRESS") as Address;
	const owner1Address = requireEnv("OWNER1_ADDRESS") as Address;
	const owner1Account = requireEnv("OWNER1_ACCOUNT");
	const owner2Address = requireEnv("OWNER2_ADDRESS") as Address;
	const owner2Account = requireEnv("OWNER2_ACCOUNT");
	const keeperAddress = requireEnv("KEEPER_ADDRESS") as Address;
	const keeperAccount = requireEnv("KEEPER_ACCOUNT");
	const deployerAccount = requireEnv("DEPLOYER_ACCOUNT");
	const safeTxServiceUrl = requireEnv("SAFE_TX_SERVICE_URL");
	const safeTxServiceApiKey = requireEnv("SAFE_TX_SERVICE_API_KEY");

	const consensusChainId = BigInt(optionalEnv("CONSENSUS_CHAIN_ID", "100"));
	const allowTxDelay = BigInt(optionalEnv("ALLOW_TX_DELAY", "60"));
	const cosignerSalt = optionalEnv("COSIGNER_SALT", ZERO_SALT) as Hex;
	const guardSalt = optionalEnv("GUARD_SALT", ZERO_SALT) as Hex;
	const safeCreationSaltNonce = BigInt(optionalEnv("SAFE_CREATION_SALT_NONCE", "0"));
	const attestationTimeout = Number.parseInt(optionalEnv("ATTESTATION_TIMEOUT_SECONDS", "120"), 10);

	const sepoliaClient = createPublicClient({ chain: sepolia, transport: http(sepoliaRpc) });
	const gnosisClient = createPublicClient({ chain: gnosis, transport: http(gnosisRpc) });

	console.log("=== Safenet Cosigner + Hypernative Guard Integration Test ===\n");

	// -----------------------------------------------------------------------
	// Phase 0: Setup (idempotent)
	// -----------------------------------------------------------------------

	console.log("[Phase 0] Setup\n");

	console.log("[0.1] Fetching active epoch from Gnosis Chain...");
	const { epoch, groupKeyX, groupKeyY } = await fetchEpoch(gnosisClient, consensusAddress);
	console.log(`  epoch=${epoch}  key.x=${groupKeyX.toString(16).slice(0, 16)}...\n`);

	console.log("[0.2] SafenetCosigner...");
	const cosignerAddress = await deployCosigner(
		sepoliaClient,
		deployerAccount,
		sepoliaRpc,
		consensusAddress,
		consensusChainId,
		epoch,
		groupKeyX,
		groupKeyY,
		allowTxDelay,
		cosignerSalt,
	);

	console.log("\n[0.3] Safe...");
	const safeAddress = await createSafe(
		sepoliaClient,
		deployerAccount,
		sepoliaRpc,
		[owner1Address, owner2Address],
		cosignerAddress,
		safeCreationSaltNonce,
	);

	console.log("\n[0.4] HypernativeGuard...");
	const guardAddress = await deployGuard(
		sepoliaClient,
		deployerAccount,
		sepoliaRpc,
		safeAddress,
		keeperAddress,
		guardSalt,
	);

	console.log("\n[0.5] Safe configuration...");
	await configureSafe(
		sepoliaClient,
		owner1Account,
		owner1Address,
		owner2Account,
		owner2Address,
		sepoliaRpc,
		safeAddress,
		guardAddress,
	);

	console.log("\n[0.6] Enforcement...");
	await activateEnforcement(sepoliaClient, keeperAccount, sepoliaRpc, guardAddress);

	console.log(`\nSafe:     ${safeAddress}`);
	console.log(`          ${sepoliaAddress(safeAddress)}`);
	console.log(`          ${safeUiLink(safeAddress)}`);
	console.log(`Cosigner: ${cosignerAddress}`);
	console.log(`          ${sepoliaAddress(cosignerAddress)}`);
	console.log(`Guard:    ${guardAddress}`);
	console.log(`          ${sepoliaAddress(guardAddress)}`);
	console.log(`Consensus (Gnosis): ${consensusAddress}`);
	console.log(`                    ${gnosisAddress(consensusAddress)}`);

	// -----------------------------------------------------------------------
	// Phase 1: Integration test
	// -----------------------------------------------------------------------

	console.log("\n[Phase 1] Integration test\n");

	// Step 1: Read current Safe nonce
	const currentNonce = await sepoliaClient.readContract({ address: safeAddress, abi: SAFE_ABI, functionName: "nonce" });
	console.log(`[1] Safe nonce: ${currentNonce}`);

	const txParams = {
		to: owner2Address,
		value: 0n,
		data: "0x" as Hex,
		operation: 0,
		safeTxGas: 0n,
		baseGas: 0n,
		gasPrice: 0n,
		gasToken: zeroAddress,
		refundReceiver: zeroAddress,
		nonce: currentNonce,
	} as const;

	// Step 2: Compute safeTxHash
	const safeTxHash = await sepoliaClient.readContract({
		address: safeAddress,
		abi: SAFE_ABI,
		functionName: "getTransactionHash",
		args: [
			txParams.to,
			txParams.value,
			txParams.data,
			txParams.operation,
			txParams.safeTxGas,
			txParams.baseGas,
			txParams.gasPrice,
			txParams.gasToken,
			txParams.refundReceiver,
			txParams.nonce,
		],
	});
	console.log(`[2] safeTxHash: ${safeTxHash}`);

	// Step 3: Propose transaction to Safenet on Gnosis Chain (skip if already attested)
	console.log("\n[3] Proposing transaction to Safenet (Gnosis Chain)...");
	let alreadyAttested = false;
	try {
		const [, existingSig] = await gnosisClient.readContract({
			address: consensusAddress,
			abi: CONSENSUS_ABI,
			functionName: "getRecentTransactionAttestationByHash",
			args: [safeTxHash],
		});
		alreadyAttested = existingSig.r.x !== 0n || existingSig.r.y !== 0n || existingSig.z !== 0n;
	} catch (e: unknown) {
		if (!(e instanceof Error && e.message.includes("NotSigned"))) throw e;
	}

	if (alreadyAttested) {
		console.log("   Already attested from a previous run, skipping proposal.");
	} else {
		const proposalOutput = castSend(
			keeperAccount,
			gnosisRpc,
			consensusAddress,
			"proposeBasicTransaction(uint256,address,address,uint256,bytes,uint256)",
			String(sepolia.id),
			safeAddress,
			txParams.to,
			String(txParams.value),
			txParams.data,
			String(currentNonce),
		);
		const proposalTxHash = proposalOutput.match(/transactionHash\s+(0x[0-9a-f]+)/i)?.[1] as Hex | undefined;
		console.log(`   Proposal tx: ${proposalTxHash ?? "sent"}`);
		if (proposalTxHash) console.log(`   ${gnosisTx(proposalTxHash)}`);
	}

	// Step 4: Poll for attestation
	console.log(`\n[4] Polling for attestation (timeout: ${attestationTimeout}s)...`);
	const deadline = Date.now() + attestationTimeout * 1000;
	let attestedEpoch = 0n;
	type FrostSig = { r: { x: bigint; y: bigint }; z: bigint };
	let attestedSig: FrostSig | null = null;

	while (Date.now() < deadline) {
		try {
			const [epochResult, sig] = await gnosisClient.readContract({
				address: consensusAddress,
				abi: CONSENSUS_ABI,
				functionName: "getRecentTransactionAttestationByHash",
				args: [safeTxHash],
			});
			if (sig.r.x !== 0n || sig.r.y !== 0n || sig.z !== 0n) {
				attestedEpoch = epochResult;
				attestedSig = sig;
				console.log(`\n   Attestation received! epoch=${epochResult}`);
				console.log(`   ${safenetTx(safeTxHash)}`);
				break;
			}
		} catch (e: unknown) {
			// NotSigned() means the FROST signing round hasn't completed yet — keep polling.
			if (!(e instanceof Error && e.message.includes("NotSigned"))) throw e;
		}
		process.stdout.write(".");
		await new Promise((r) => setTimeout(r, 5000));
	}
	if (attestedSig === null) throw new Error(`Attestation timeout after ${attestationTimeout}s`);
	const finalSig = attestedSig;

	// Steps 5 & 6: Sign with owner1 and keeper
	console.log("\n[5] Signing with owner1...");
	const owner1Sig = castSign(owner1Account, safeTxHash);
	console.log(`   owner1 sig: ${owner1Sig.slice(0, 20)}...`);

	console.log("[6] Signing with keeper...");
	const keeperSig = castSign(keeperAccount, safeTxHash);
	console.log(`   keeper sig: ${keeperSig.slice(0, 20)}...`);

	// Step 7: Assemble 387-byte signatures blob
	console.log("\n[7] Assembling signatures...");
	const attestation = encodeAbiParameters(
		[
			{ type: "uint64" },
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
		[attestedEpoch, { r: { x: finalSig.r.x, y: finalSig.r.y }, z: finalSig.z }],
	) as Hex;

	const signatures = assembleSafeSignatures({ owner1Address, owner1Sig, cosignerAddress, attestation, keeperSig });
	console.log(`   Signatures assembled (${(signatures.length - 2) / 2} bytes).`);

	// Step 8: Submit to Safe TX Service
	console.log("\n[8] Submitting to Safe TX Service...");
	const submitResponse = await fetch(`${safeTxServiceUrl}/api/v1/safes/${safeAddress}/multisig-transactions/`, {
		method: "POST",
		headers: { "Content-Type": "application/json", Authorization: `Token ${safeTxServiceApiKey}` },
		body: JSON.stringify({
			to: txParams.to,
			value: txParams.value.toString(),
			data: txParams.data,
			operation: txParams.operation,
			safeTxGas: txParams.safeTxGas.toString(),
			baseGas: txParams.baseGas.toString(),
			gasPrice: txParams.gasPrice.toString(),
			gasToken: txParams.gasToken,
			refundReceiver: txParams.refundReceiver,
			nonce: currentNonce.toString(),
			contractTransactionHash: safeTxHash,
			sender: owner1Address,
			signature: signatures,
		}),
	});
	if (!submitResponse.ok) {
		const errorText = await submitResponse.text();
		throw new Error(`Safe TX Service submission failed (${submitResponse.status}): ${errorText}`);
	}
	console.log("   Transaction submitted to Safe TX Service.");

	// [9.1] Verify keeper has KEEPER_ROLE on the guard
	console.log("\n[9.1] Verifying keeper role on HypernativeGuard...");
	const keeperHasRole = await sepoliaClient.readContract({
		address: guardAddress,
		abi: GUARD_ABI,
		functionName: "hasRole",
		args: [KEEPER_ROLE, keeperAddress],
	});
	if (!keeperHasRole) throw new Error(`Keeper ${keeperAddress} does NOT have KEEPER_ROLE on guard ${guardAddress}`);
	console.log(`   keeper ${keeperAddress} has KEEPER_ROLE ✓`);

	// [9.2] Re-verify attestation epoch is still current or previous on the cosigner
	console.log("[9.2] Re-verifying epoch validity on cosigner...");
	const activeNow = await sepoliaClient.readContract({
		address: cosignerAddress,
		abi: COSIGNER_ABI,
		functionName: "activeEpoch",
	});
	let previousNow: bigint | null = null;
	try {
		previousNow = await sepoliaClient.readContract({
			address: cosignerAddress,
			abi: COSIGNER_ABI,
			functionName: "previousEpoch",
		});
	} catch {
		// previousEpoch reverts if not yet set
	}
	if (attestedEpoch !== activeNow && attestedEpoch !== previousNow)
		throw new Error(
			`Attested epoch ${attestedEpoch} is no longer current (active=${activeNow}, previous=${previousNow ?? "none"})`,
		);
	console.log(`   attestedEpoch=${attestedEpoch} active=${activeNow} previous=${previousNow ?? "none"} ✓`);

	// [9.3] FROST dry-run: call isValidSignature directly to confirm attestation is accepted
	console.log("[9.3] FROST dry-run (isValidSignature)...");
	{
		let ivResult: string | undefined;
		try {
			ivResult = await sepoliaClient.readContract({
				address: cosignerAddress,
				abi: COSIGNER_ABI,
				functionName: "isValidSignature",
				args: [safeTxHash, attestation],
			});
		} catch (ivErr) {
			const raw = extractRevertData(ivErr);
			console.error("   isValidSignature REVERTED — FROST verification failure:");
			logRevertReason(raw);
			console.error(`   Raw revert data: ${raw}`);
			throw new Error("isValidSignature failed — FROST or precompile issue (see above)");
		}
		const MAGIC = "0x1626ba7e";
		if (ivResult !== MAGIC) {
			throw new Error(`isValidSignature returned wrong magic: ${ivResult} (expected ${MAGIC})`);
		}
		console.log("   isValidSignature: FROST signature valid ✓");
	}

	// [9.4] eth_call simulation — capture exact revert reason before committing gas
	console.log("[9.4] Simulating execTransaction via eth_call...");
	try {
		await sepoliaClient.call({
			account: owner1Address,
			to: safeAddress,
			data: encodeFunctionData({
				abi: SAFE_ABI,
				functionName: "execTransaction",
				args: [
					txParams.to,
					txParams.value,
					txParams.data,
					txParams.operation,
					txParams.safeTxGas,
					txParams.baseGas,
					txParams.gasPrice,
					txParams.gasToken,
					txParams.refundReceiver,
					signatures,
				],
			}),
		});
		console.log("   eth_call simulation: SUCCESS (no revert)");
	} catch (simErr) {
		const raw = extractRevertData(simErr);
		console.error("   eth_call simulation REVERTED:");
		logRevertReason(raw);
		console.error(`   Raw revert data: ${raw}`);
		dumpErrorChain(simErr);

		// Attempt a debug_traceCall to locate the exact revert site.
		// Most public nodes do not support this; the try/catch makes it a best-effort diagnostic.
		try {
			const callData = encodeFunctionData({
				abi: SAFE_ABI,
				functionName: "execTransaction",
				args: [
					txParams.to,
					txParams.value,
					txParams.data,
					txParams.operation,
					txParams.safeTxGas,
					txParams.baseGas,
					txParams.gasPrice,
					txParams.gasToken,
					txParams.refundReceiver,
					signatures,
				],
			});
			const traceResult = await sepoliaClient.request({
				method: "debug_traceCall" as never,
				params: [{ from: owner1Address, to: safeAddress, data: callData }, "latest", { tracer: "callTracer" }] as never,
			});
			console.error("   [debug_traceCall result]", JSON.stringify(traceResult, null, 2).slice(0, 10000));
		} catch {
			console.error("   (debug_traceCall not supported by this node)");
		}

		throw new Error("execTransaction eth_call simulation reverted (see above)");
	}

	// Step 10: Execute on-chain
	console.log("\n[10] Executing execTransaction on Sepolia...");
	const execOutput = castSend(
		owner1Account,
		sepoliaRpc,
		safeAddress,
		"execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)",
		txParams.to,
		String(txParams.value),
		txParams.data,
		String(txParams.operation),
		String(txParams.safeTxGas),
		String(txParams.baseGas),
		String(txParams.gasPrice),
		txParams.gasToken,
		txParams.refundReceiver,
		signatures,
	);
	const execTxHash = execOutput.match(/transactionHash\s+(0x[0-9a-f]+)/i)?.[1] as Hex | undefined;
	console.log(`   execTransaction tx: ${execTxHash ?? "sent"}`);
	if (execTxHash) {
		console.log(`   ${sepoliaTx(execTxHash)}`);
		console.log(`   ${tenderlyTx(execTxHash)}`);
	}

	// Step 11: Verify nonce advanced and receipt success
	console.log("\n[11] Verifying...");
	const newNonce = await sepoliaClient.readContract({ address: safeAddress, abi: SAFE_ABI, functionName: "nonce" });
	if (newNonce !== currentNonce + 1n) throw new Error(`Expected nonce ${currentNonce + 1n}, got ${newNonce}`);
	console.log(`   Safe nonce advanced: ${currentNonce} -> ${newNonce} ✓`);

	if (execTxHash) {
		const receipt = await sepoliaClient.waitForTransactionReceipt({ hash: execTxHash });
		if (receipt.status !== "success") throw new Error(`Transaction reverted (status: ${receipt.status})`);
		console.log("   Transaction receipt status: success ✓");
	}

	console.log("\n=== Integration test PASSED ===");
}

main().catch((err) => {
	console.error("\n=== Integration test FAILED ===");
	console.error(err instanceof Error ? err.message : err);
	process.exit(1);
});
