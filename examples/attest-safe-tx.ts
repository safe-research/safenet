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
 *   Required: CONSENSUS_ADDRESS, GNOSIS_RPC_URL, SAFE_TX_SERVICE_URL, SAFE_TX_SERVICE_API_KEY
 */

import { resolve } from "node:path";
import dotenv from "dotenv";
import { createPublicClient, encodeAbiParameters, getAddress, http, parseAbi, type Address, type Hex } from "viem";
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
const [rawHash, rawCosigner] = args as [string, string];
if (!rawHash.startsWith("0x")) usage();

const safeTxHash = rawHash as Hex;
const cosignerAddress = getAddress(rawCosigner) as Address;

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

function requireEnv(name: string): string {
    const value = process.env[name];
    if (!value) throw new Error(`Missing required env var: ${name}`);
    return value;
}

const consensusAddress = getAddress(requireEnv("CONSENSUS_ADDRESS")) as Address;
const gnosisRpc = requireEnv("GNOSIS_RPC_URL");
const safeTxServiceUrl = requireEnv("SAFE_TX_SERVICE_URL").replace(/\/$/, "");
const safeTxServiceApiKey = requireEnv("SAFE_TX_SERVICE_API_KEY");
const attestationTimeout = Number(process.env["ATTESTATION_TIMEOUT_SECONDS"] ?? "120");

const authHeaders = { Authorization: `Bearer ${safeTxServiceApiKey}` };

// ---------------------------------------------------------------------------
// ABI
// ---------------------------------------------------------------------------

const CONSENSUS_ABI = parseAbi([
    "function getRecentTransactionAttestationByHash(bytes32) external view returns (uint64 epoch, ((uint256 x, uint256 y) r, uint256 z) signature)",
    "error NotSigned()",
]);

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
    // Step 1: Poll for attestation from Gnosis Chain
    console.log(`[1] Polling for attestation (timeout: ${attestationTimeout}s)...`);

    const gnosisClient = createPublicClient({ chain: gnosis, transport: http(gnosisRpc) });

    type FrostSig = { r: { x: bigint; y: bigint }; z: bigint };
    let attestedEpoch = 0n;
    let attestedSig: FrostSig | null = null;
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
                attestedEpoch = epoch;
                attestedSig = sig;
                console.log(`\n   Attestation received! epoch=${epoch}`);
                break;
            }
        } catch (e: unknown) {
            if (!(e instanceof Error && e.message.includes("NotSigned"))) throw e;
        }
        process.stdout.write(".");
        await new Promise((r) => setTimeout(r, 5000));
    }
    if (attestedSig === null) throw new Error(`Attestation timeout after ${attestationTimeout}s`);

    // Step 2: Fetch multisig transaction details from Safe TX Service
    console.log(`\n[2] Fetching transaction details from Safe TX Service...`);

    const txResponse = await fetch(`${safeTxServiceUrl}/api/v2/multisig-transactions/${safeTxHash}/`, {
        headers: authHeaders,
    });
    if (!txResponse.ok) {
        const text = await txResponse.text();
        throw new Error(`Safe TX Service GET failed (${txResponse.status}): ${text}`);
    }
    const tx = (await txResponse.json()) as {
        safe: string;
        to: string;
        value: string;
        data: string | null;
        operation: number;
        safeTxGas: string;
        baseGas: string;
        gasPrice: string;
        gasToken: string;
        refundReceiver: string;
        nonce: number;
    };
    const safeAddress = getAddress(tx.safe) as Address;
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
        [attestedEpoch, { r: { x: attestedSig.r.x, y: attestedSig.r.y }, z: attestedSig.z }],
    ) as Hex;

    const staticSlot =
        cosignerAddress.toLowerCase().slice(2).padStart(64, "0") +
        (65).toString(16).padStart(64, "0") +
        "00";
    const dynamicData = (128).toString(16).padStart(64, "0") + attestation.slice(2);
    const contractSignature = `0x${staticSlot}${dynamicData}` as Hex;

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
