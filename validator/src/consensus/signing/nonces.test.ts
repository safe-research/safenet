import { bytesToHex, hexToBytes, keccak256, stringToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { g, toPoint } from "../../frost/math.js";
import { generateMerkleProof, verifyMerkleProof } from "../merkle.js";
import {
	bindingFactors,
	bindingPrefix,
	calculateGroupCommitment,
	createNonceTree,
	decodeSequence,
	generateNonce,
	groupCommitementShares,
	groupCommitmentShare,
	type NonceCommitments,
	nonceCommitmentsWithProof,
} from "./nonces.js";

const groupPublicKey = toPoint({
	x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75n,
	y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5n,
});
const signers = [1n, 2n, 3n];
const commitments = new Map<bigint, NonceCommitments>();
for (const p of signers) {
	commitments.set(p, {
		hidingNonce: 0xd0n + p,
		bindingNonce: 0xe0n + p,
		hidingNonceCommitment: g(0xd0n + p),
		bindingNonceCommitment: g(0xe0n + p),
	});
}
const message = keccak256(stringToBytes("hello"));

describe("generateNonce", () => {
	it("should generate correct nonce", async () => {
		const random = hexToBytes("0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a");
		const secret = 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80n;
		expect(generateNonce(secret, random)).toBe(0x03d979abaa17ca44e015f9e248c6cefc167ad21e814256f2a0a02cce70d57ba1n);
	});

	it("produces different nonces for different secrets with same randomness", () => {
		const randomness = new Uint8Array(32).fill(1);
		const n1 = generateNonce(42n, randomness);
		const n2 = generateNonce(137n, randomness);
		expect(n1).not.toBe(n2);
	});

	it("throws when randomness length is not 32 (e.g. length 16)", () => {
		const secret = 42n;
		const badRandomness = new Uint8Array(16).fill(1);
		expect(() => generateNonce(secret, badRandomness)).toThrow("invalid nonce randomness");
	});
});

describe("createNonceTree", () => {
	it("tree with size=4n has 4 commitments and 4 leaves", () => {
		const tree = createNonceTree(42n, 4n);
		expect(tree.commitments.length).toBe(4);
		expect(tree.leaves.length).toBe(4);
	});

	it("verifyMerkleProof works for any valid leaf index", () => {
		const tree = createNonceTree(42n, 4n);
		for (let i = 0; i < 4; i++) {
			const proof = generateMerkleProof(tree.leaves, i);
			expect(verifyMerkleProof(tree.root, tree.leaves[i], proof)).toBe(true);
		}
	});
});

describe("bindingPrefix", () => {
	it("returns expected bytes for known inputs", () => {
		expect(bytesToHex(bindingPrefix(groupPublicKey, signers, commitments, message))).toBe(
			"0x038318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed753e3ff5d5672762f4c3add84cc8e383dc781e5f8f8f230913e114bae324ffbe64fa82f351f10ae44fb79bba17ffd42aba4370ec76c6e48328409c1a981ca3b50a",
		);
	});
});

describe("bindingFactors", () => {
	it("returns one BindingFactor per signer", () => {
		const factors = bindingFactors(groupPublicKey, signers, commitments, message);
		expect(factors.length).toBe(signers.length);
		for (let i = 0; i < signers.length; i++) {
			expect(factors[i].id).toBe(signers[i]);
		}
	});

	it("returns expected binding factors for known inputs", () => {
		const factors = bindingFactors(groupPublicKey, signers, commitments, message);
		expect(factors[0].bindingFactor).toBe(0x3ace394f1783cd2f9647aaded69596328f98cc57c823ae5652d7275461be9bean);
		expect(factors[1].bindingFactor).toBe(0x30df3963e4aee100fa049ec729adf4e75609b4f3f699fa17cf1c593ef1cf3ecfn);
		expect(factors[2].bindingFactor).toBe(0x04849a66886b4b59b920d847e334fc3f9aa355d8c152e146d3ed03c8c3a8096dn);
	});
});

describe("groupCommitementShares", () => {
	const signers = [1n, 2n];
	const noncesMap = new Map<bigint, NonceCommitments>();
	for (const p of signers) {
		noncesMap.set(p, {
			hidingNonce: 1000n + p,
			bindingNonce: 2000n + p,
			hidingNonceCommitment: g(1000n + p),
			bindingNonceCommitment: g(2000n + p),
		});
	}
	const bfs = [
		{ id: 1n, bindingFactor: 3n },
		{ id: 2n, bindingFactor: 5n },
	];
	const shares = groupCommitementShares(bfs, noncesMap);

	it("returns one share per signer", () => {
		expect(shares.length).toBe(2);
	});

	it("each share matches groupCommitmentShare applied individually", () => {
		for (let i = 0; i < bfs.length; i++) {
			const nonces = noncesMap.get(bfs[i].id);
			if (!nonces) throw new Error(`No nonces for signer ${bfs[i].id}`);
			const expected = groupCommitmentShare(bfs[i].bindingFactor, nonces);
			expect(shares[i].x).toBe(expected.x);
			expect(shares[i].y).toBe(expected.y);
		}
	});
});

describe("calculateGroupCommitment", () => {
	const noncesA: NonceCommitments = {
		hidingNonce: 1000n,
		bindingNonce: 2000n,
		hidingNonceCommitment: g(1000n),
		bindingNonceCommitment: g(2000n),
	};

	const noncesB: NonceCommitments = {
		hidingNonce: 3000n,
		bindingNonce: 4000n,
		hidingNonceCommitment: g(3000n),
		bindingNonceCommitment: g(4000n),
	};

	it("for a single share, result equals that share", () => {
		const share = groupCommitmentShare(3n, noncesA);
		const commitment = calculateGroupCommitment([share]);
		expect(commitment.x).toBe(share.x);
		expect(commitment.y).toBe(share.y);
	});

	it("is the sum of all shares", () => {
		const share1 = groupCommitmentShare(3n, noncesA);
		const share2 = groupCommitmentShare(5n, noncesB);
		const result = calculateGroupCommitment([share1, share2]);
		const expected = share1.add(share2);
		expect(result.x).toBe(expected.x);
		expect(result.y).toBe(expected.y);
	});
});

describe("decodeSequence", () => {
	it("(0n) → {chunk: 0n, offset: 0n}", () => {
		expect(decodeSequence(0n)).toEqual({ chunk: 0n, offset: 0n });
	});

	it("(1023n) → {chunk: 0n, offset: 1023n}", () => {
		expect(decodeSequence(1023n)).toEqual({ chunk: 0n, offset: 1023n });
	});

	it("(1024n) → {chunk: 1n, offset: 0n}", () => {
		expect(decodeSequence(1024n)).toEqual({ chunk: 1n, offset: 0n });
	});

	it("(1025n) → {chunk: 1n, offset: 1n}", () => {
		expect(decodeSequence(1025n)).toEqual({ chunk: 1n, offset: 1n });
	});

	it("custom chunkSize: decodeSequence(10n, 5n) → {chunk: 2n, offset: 0n}", () => {
		expect(decodeSequence(10n, 5n)).toEqual({ chunk: 2n, offset: 0n });
	});

	it("custom chunkSize: decodeSequence(11n, 5n) → {chunk: 2n, offset: 1n}", () => {
		expect(decodeSequence(11n, 5n)).toEqual({ chunk: 2n, offset: 1n });
	});
});

describe("nonceCommitmentsWithProof", () => {
	it.each([0, 2])("generates proof for index %d that verifies against tree root", (index) => {
		const tree = createNonceTree(42n, 4n);
		const { nonceCommitments, nonceProof } = nonceCommitmentsWithProof(tree, BigInt(index));
		expect(nonceCommitments).toBe(tree.commitments[index]);
		expect(verifyMerkleProof(tree.root, tree.leaves[index], nonceProof)).toBe(true);
	});
});
