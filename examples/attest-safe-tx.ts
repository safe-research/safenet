/**
 * Attach a Safenet FROST attestation to an existing Safe transaction.
 *
 * Fetches the FROST attestation for a given Safe transaction hash from the
 * Safenet Consensus contract, then posts it as a cosigner EIP-1271 signature
 * to the Safe Transaction Service so the Safe UI shows it as confirmed.
 *
 * Usage:
 *   npm run examples:attest-safe-tx -- <safeTxHash> <cosignerAddress>
 *
 * Environment:
 *   Copy examples/.env.sample to examples/.env and fill in the values.
 *   Required: CONSENSUS_ADDRESS, RPC_URL, SAFE_TX_SERVICE_URL, SAFE_TX_SERVICE_API_KEY
 */

import { resolve } from "node:path";
import dotenv from "dotenv";
import z from "zod";
import {
    concat,
    createPublicClient,
    encodeAbiParameters,
    getAddress,
    http,
    isAddress,
    isHex,
    numberToHex,
    pad,
    parseAbi,
    size,
    type Address,
    type Hex,
} from "viem";
import { gnosis } from "viem/chains";

dotenv.config({ path: resolve(import.meta.dirname, ".env"), quiet: true });

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

function usage(): never {
    console.error("Usage: npm run examples:attest-safe-tx -- <safeTxHash> <cosignerAddress>");
    process.exit(1);
}

const args = process.argv.slice(2);
if (args.length !== 2) usage();
const [rawHash = "", rawCosigner = ""] = args;
if (!isHex(rawHash) || size(rawHash) !== 32) usage();
if (!isAddress(rawCosigner)) usage();

const safeTxHash: Hex = rawHash;
const cosignerAddress: Address = rawCosigner;

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

const envSchema = z.object({
    CONSENSUS_ADDRESS: z
        .string()
        .refine((a) => isAddress(a, { strict: false }), "Invalid address format")
        .transform((a) => getAddress(a)),
    RPC_URL: z.url(),
    SAFE_TX_SERVICE_URL: z.url().transform((url) => url.replace(/\/$/, "")),
    SAFE_TX_SERVICE_API_KEY: z.string().min(1),
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
    RPC_URL: rpc,
    SAFE_TX_SERVICE_URL: safeTxServiceUrl,
    SAFE_TX_SERVICE_API_KEY: safeTxServiceApiKey,
    ATTESTATION_TIMEOUT_SECONDS: attestationTimeout,
} = envParseResult.data;

const authHeaders = { Authorization: `Bearer ${safeTxServiceApiKey}` };
const gnosisClient = createPublicClient({ chain: gnosis, transport: http(rpc) });

// ---------------------------------------------------------------------------
// ABI
// ---------------------------------------------------------------------------

const CONSENSUS_ABI = parseAbi([
    "function getRecentTransactionAttestationByHash(bytes32) external view returns (uint64 epoch, ((uint256 x, uint256 y) r, uint256 z) signature)",
    "error NotSigned()",
]);

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
});

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
    // Step 1: Poll for attestation from Gnosis Chain
    console.log(`[1] Polling for attestation (timeout: ${attestationTimeout}s)...`);

    type FrostSig = { r: { x: bigint; y: bigint }; z: bigint };
    let attested: { epoch: bigint; sig: FrostSig } | null = null;
    const deadline = Date.now() + attestationTimeout * 1000;

    while (Date.now() < deadline) {
        try {
            const [epoch, sig] = await gnosisClient.readContract({
                address: consensusAddress,
                abi: CONSENSUS_ABI,
                functionName: "getRecentTransactionAttestationByHash",
                args: [safeTxHash],
            });
            if (sig.r.x !== 0n || sig.r.y !== 0n || sig.z !== 0n) {
                attested = { epoch, sig };
                console.log(`\n   Attestation received! epoch=${epoch}`);
                break;
            }
        } catch (e: unknown) {
            if (!(e instanceof Error && e.message.includes("NotSigned"))) throw e;
        }
        process.stdout.write(".");
        await new Promise((r) => setTimeout(r, 5000));
    }
    if (attested === null) throw new Error(`Attestation timeout after ${attestationTimeout}s`);

    // Step 2: Fetch multisig transaction details from Safe TX Service
    console.log(`\n[2] Fetching transaction details from Safe TX Service...`);

    const txResponse = await fetch(`${safeTxServiceUrl}/api/v2/multisig-transactions/${safeTxHash}/`, {
        headers: authHeaders,
    });
    if (!txResponse.ok) {
        const text = await txResponse.text();
        throw new Error(`Safe TX Service GET failed (${txResponse.status}): ${text}`);
    }

    const tx = txSchema.parse(await txResponse.json());
    const safeAddress = tx.safe;
    console.log(`   Safe:  ${safeAddress}`);
    console.log(`   Nonce: ${tx.nonce}`);

    // Step 3: Encode cosigner EIP-1271 contract signature
    //
    //   Static slot (65 bytes):
    //     r (32 bytes) = cosigner address left-padded with zeros
    //     s (32 bytes) = 65 — byte offset to dynamic data within this signature blob
    //     v (1 byte)   = 0x00 — marks as EIP-1271 contract signature
    //
    //   Dynamic data (160 bytes):
    //     length (32 bytes) = 128
    //     data   (128 bytes) = abi.encode(uint64 epoch, FROST.Signature{r:{x,y}, z})
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
        [attested.epoch, { r: { x: attested.sig.r.x, y: attested.sig.r.y }, z: attested.sig.z }],
    );

    const contractSignature: Hex = concat([
        pad(cosignerAddress, { size: 32 }), // r: 20-byte address left-padded to 32 bytes
        numberToHex(65, { size: 32 }), // s: 65 (byte offset to dynamic data)
        numberToHex(0, { size: 1 }), // v: 0x00 (EIP-1271 contract signature marker)
        numberToHex(128, { size: 32 }), // dynamic data length
        attestation, // 128 bytes: abi.encode(epoch, FROST.Signature)
    ]);

    // Step 4: POST cosigner signature to Safe TX Service
    console.log(`\n[3] Posting cosigner signature to Safe TX Service...`);

    const postResponse = await fetch(`${safeTxServiceUrl}/api/v2/safes/${safeAddress}/multisig-transactions/`, {
        method: "POST",
        headers: { "Content-Type": "application/json", ...authHeaders },
        body: JSON.stringify({
            to: tx.to,
            value: tx.value,
            data: tx.data ?? "0x",
            operation: tx.operation,
            safeTxGas: tx.safeTxGas,
            baseGas: tx.baseGas,
            gasPrice: tx.gasPrice,
            gasToken: tx.gasToken,
            refundReceiver: tx.refundReceiver,
            nonce: tx.nonce,
            contractTransactionHash: safeTxHash,
            sender: cosignerAddress,
            signature: contractSignature,
        }),
    });
    if (!postResponse.ok) {
        const text = await postResponse.text();
        throw new Error(`Safe TX Service POST failed (${postResponse.status}): ${text}`);
    }

    console.log("   Signature posted successfully.");
    console.log(`\nSafenet attestation for ${safeTxHash} is now visible in the Safe UI.`);
}

main().catch((e: unknown) => {
    console.error("Error:", e instanceof Error ? e.message : e);
    process.exit(1);
});
