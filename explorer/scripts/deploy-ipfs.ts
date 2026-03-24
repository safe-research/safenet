#!/usr/bin/env tsx
/// <reference types="node" />

/**
 * Deploy the built dist/ directory to IPFS via Pinata.
 *
 * Required environment variables:
 *   PINATA_JWT          — API JWT from https://app.pinata.cloud/developers/api-keys
 *
 * Optional environment variables:
 *   PINATA_GATEWAY      — Your dedicated gateway domain (e.g. "my-gateway.mypinata.cloud")
 *
 * Usage:
 *   tsx scripts/deploy-ipfs.ts              # build + upload
 *   tsx scripts/deploy-ipfs.ts --skip-build # upload only (dist/ must exist)
 *
 * The upload is always named with a snapshot timestamp, e.g. "safenet-explorer-2026-03-18T14:30:00.000Z".
 */

import { execSync, type ExecSyncOptions } from "node:child_process";
import { existsSync, globSync, readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { PinataSDK } from "pinata";

const ROOT = resolve(import.meta.dirname, "..");
const DIST = resolve(ROOT, "dist");

const skipBuild = process.argv.includes("--skip-build");

const uploadName = `safenet-explorer-${new Date().toISOString()}`;

function run(cmd: string, opts: ExecSyncOptions = {}): Buffer {
	console.log(`\n> ${cmd}`);
	return execSync(cmd, { cwd: ROOT, ...opts }) as Buffer;
}

/** Collect all files in a directory into File objects with relative paths. */
function collectFiles(dir: string): File[] {
	const files: File[] = [];
	for (const entry of globSync(`${dir}/**/*`, { withFileTypes: true })) {
		if (entry.isFile()) {
			const path = join(entry.parentPath, entry.name);
			const content = readFileSync(path);
			files.push(new File([content], relative(dir, path)));
		}
	}
	return files;
}

// --- Validate env ---
const jwt = process.env.PINATA_JWT;
const gateway = process.env.PINATA_GATEWAY;

if (!jwt) {
	console.error("Error: PINATA_JWT environment variable is required.");
	console.error("Get one at https://app.pinata.cloud/developers/api-keys");
	process.exit(1);
}

// --- 1. Build ---
if (!skipBuild) {
	console.log("\n--- Building production bundle ---");
	run("npm run build", { stdio: "inherit" });
}

if (!existsSync(resolve(DIST, "index.html"))) {
	console.error("Error: dist/index.html not found. Run `npm run build` first.");
	process.exit(1);
}

// --- 2. Upload to Pinata / IPFS ---
console.log(`\n--- Uploading dist/ to IPFS via Pinata (name: "${uploadName}") ---`);

const pinata = new PinataSDK({
	pinataJwt: jwt,
	...(gateway && { pinataGateway: gateway }),
});

const files = collectFiles(DIST);
console.log(`Collected ${files.length} files from dist/`);

const upload = await pinata.upload.public.fileArray(files).name(uploadName);

const cid = upload.cid;

console.log(`\nCID:        ${cid}`);
console.log(`IPFS:       ipfs://${cid}`);
if (gateway) {
	console.log(`Pinata:     https://${gateway}/ipfs/${cid}`);
}
console.log(`dweb.link:  https://${cid}.ipfs.dweb.link/`);
console.log(`cf-ipfs:    https://cloudflare-ipfs.com/ipfs/${cid}`);
