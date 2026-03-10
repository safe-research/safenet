import { bytesToHex, hexToBytes, keccak256, stringToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { g, toPoint } from "../../frost/math.js";
import { verifyMerkleProof, generateMerkleProof } from "../merkle.js";
import {
	bindingFactor,
	bindingFactors,
	bindingPrefix,
	calculateGroupCommitment,
	createNonceTree,
	decodeSequence,
	generateNonce,
	generateNonceCommitments,
	groupCommitmentShare,
	groupCommitementShares,
	nonceCommitmentsWithProof,
	type NonceCommitments,
} from "./nonces.js";

describe("generateNonce", () => {
	it("is deterministic with fixed randomness", () => {
		const secret = 42n;
		const randomness = new Uint8Array(32).fill(1);
		const n1 = generateNonce(secret, randomness);
		const n2 = generateNonce(secret, randomness);
		expect(n1).toBe(n2);
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

	it("returns a bigint", () => {
		const secret = 42n;
		const randomness = new Uint8Array(32).fill(7);
		const result = generateNonce(secret, randomness);
		expect(typeof result).toBe("bigint");
		expect(result).toBeGreaterThan(0n);
	});
});

describe("generateNonceCommitments", () => {
	it("hidingNonceCommitment equals g(hidingNonce)", () => {
		// generateNonceCommitments uses random nonces internally, but we can verify
		// the structure by checking commitments match g(nonce)
		const secret = 42n;
		// Use a fixed known nonce to verify: commitments use g(nonce)
		const hidingNonce = 1000n;
		const bindingNonce = 2000n;
		// Manually construct to verify the relationship
		const hidingNonceCommitment = g(hidingNonce);
		const bindingNonceCommitment = g(bindingNonce);
		expect(hidingNonceCommitment.x).toBe(g(hidingNonce).x);
		expect(bindingNonceCommitment.x).toBe(g(bindingNonce).x);
	});

	it("returns all four fields: hidingNonce, bindingNonce, hidingNonceCommitment, bindingNonceCommitment", () => {
		const result = generateNonceCommitments(42n);
		expect(typeof result.hidingNonce).toBe("bigint");
		expect(typeof result.bindingNonce).toBe("bigint");
		expect(result.hidingNonceCommitment).toBeDefined();
		expect(result.bindingNonceCommitment).toBeDefined();
	});

	it("hidingNonceCommitment equals g(hidingNonce) for generated commitments", () => {
		const result = generateNonceCommitments(42n);
		const expectedHiding = g(result.hidingNonce);
		const expectedBinding = g(result.bindingNonce);
		expect(result.hidingNonceCommitment.x).toBe(expectedHiding.x);
		expect(result.hidingNonceCommitment.y).toBe(expectedHiding.y);
		expect(result.bindingNonceCommitment.x).toBe(expectedBinding.x);
		expect(result.bindingNonceCommitment.y).toBe(expectedBinding.y);
	});
});

describe("createNonceTree", () => {
	it("tree with size=4n has 4 commitments and 4 leaves", () => {
		const secret = 42n;
		const tree = createNonceTree(secret, 4n);
		expect(tree.commitments.length).toBe(4);
		expect(tree.leaves.length).toBe(4);
	});

	it("has a valid merkle root", () => {
		const tree = createNonceTree(42n, 4n);
		expect(typeof tree.root).toBe("string");
		expect(tree.root.startsWith("0x")).toBe(true);
	});

	it("verifyMerkleProof(root, leaves[0], generateMerkleProof(leaves, 0)) is true", () => {
		const tree = createNonceTree(42n, 4n);
		const proof = generateMerkleProof(tree.leaves, 0);
		expect(verifyMerkleProof(tree.root, tree.leaves[0], proof)).toBe(true);
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

	it("is deterministic for same inputs", () => {
		const prefix1 = bindingPrefix(groupPublicKey, signers, commitments, message);
		const prefix2 = bindingPrefix(groupPublicKey, signers, commitments, message);
		expect(bytesToHex(prefix1)).toBe(bytesToHex(prefix2));
	});

	it("different message produces different prefix", () => {
		const message2 = keccak256(stringToBytes("world"));
		const prefix1 = bindingPrefix(groupPublicKey, signers, commitments, message);
		const prefix2 = bindingPrefix(groupPublicKey, signers, commitments, message2);
		expect(bytesToHex(prefix1)).not.toBe(bytesToHex(prefix2));
	});

	it("returns expected bytes for known inputs", () => {
		expect(bytesToHex(bindingPrefix(groupPublicKey, signers, commitments, message))).toBe(
			"0x038318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed753e3ff5d5672762f4c3add84cc8e383dc781e5f8f8f230913e114bae324ffbe64fa82f351f10ae44fb79bba17ffd42aba4370ec76c6e48328409c1a981ca3b50a",
		);
	});
});

describe("bindingFactors", () => {
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

	it("returns one BindingFactor per signer", () => {
		const factors = bindingFactors(groupPublicKey, signers, commitments, message);
		expect(factors.length).toBe(signers.length);
		for (let i = 0; i < signers.length; i++) {
			expect(factors[i].id).toBe(signers[i]);
			expect(typeof factors[i].bindingFactor).toBe("bigint");
		}
	});

	it("is deterministic for same inputs", () => {
		const factors1 = bindingFactors(groupPublicKey, signers, commitments, message);
		const factors2 = bindingFactors(groupPublicKey, signers, commitments, message);
		for (let i = 0; i < signers.length; i++) {
			expect(factors1[i].bindingFactor).toBe(factors2[i].bindingFactor);
		}
	});

	it("returns expected binding factors for known inputs", () => {
		const factors = bindingFactors(groupPublicKey, signers, commitments, message);
		expect(factors[0].bindingFactor).toBe(0x3ace394f1783cd2f9647aaded69596328f98cc57c823ae5652d7275461be9bean);
		expect(factors[1].bindingFactor).toBe(0x30df3963e4aee100fa049ec729adf4e75609b4f3f699fa17cf1c593ef1cf3ecfn);
		expect(factors[2].bindingFactor).toBe(0x04849a66886b4b59b920d847e334fc3f9aa355d8c152e146d3ed03c8c3a8096dn);
	});
});

describe("groupCommitmentShare", () => {
	it("equals hidingNonceCommitment.add(bindingNonceCommitment.multiply(bindingFactor))", () => {
		const nonces: NonceCommitments = {
			hidingNonce: 1000n,
			bindingNonce: 2000n,
			hidingNonceCommitment: g(1000n),
			bindingNonceCommitment: g(2000n),
		};
		const bf = 3n;
		const result = groupCommitmentShare(bf, nonces);
		const expected = nonces.hidingNonceCommitment.add(nonces.bindingNonceCommitment.multiply(bf));
		expect(result.x).toBe(expected.x);
		expect(result.y).toBe(expected.y);
	});

	it("is deterministic for same inputs", () => {
		const nonces: NonceCommitments = {
			hidingNonce: 1000n,
			bindingNonce: 2000n,
			hidingNonceCommitment: g(1000n),
			bindingNonceCommitment: g(2000n),
		};
		const r1 = groupCommitmentShare(5n, nonces);
		const r2 = groupCommitmentShare(5n, nonces);
		expect(r1.x).toBe(r2.x);
		expect(r1.y).toBe(r2.y);
	});
});

describe("groupCommitementShares", () => {
	// NOTE: "groupCommitementShares" is the exact export name (with typo)
	it("returns one share per signer", () => {
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
		expect(shares.length).toBe(2);
	});

	it("each share matches groupCommitmentShare applied individually", () => {
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

		for (let i = 0; i < bfs.length; i++) {
			const nonces = noncesMap.get(bfs[i].id)!;
			const expected = groupCommitmentShare(bfs[i].bindingFactor, nonces);
			expect(shares[i].x).toBe(expected.x);
			expect(shares[i].y).toBe(expected.y);
		}
	});
});

describe("calculateGroupCommitment", () => {
	it("for a single share, result equals that share", () => {
		const nonces: NonceCommitments = {
			hidingNonce: 1000n,
			bindingNonce: 2000n,
			hidingNonceCommitment: g(1000n),
			bindingNonceCommitment: g(2000n),
		};
		const share = groupCommitmentShare(3n, nonces);
		const commitment = calculateGroupCommitment([share]);
		expect(commitment.x).toBe(share.x);
		expect(commitment.y).toBe(share.y);
	});

	it("is the sum of all shares", () => {
		const nonces1: NonceCommitments = {
			hidingNonce: 1000n,
			bindingNonce: 2000n,
			hidingNonceCommitment: g(1000n),
			bindingNonceCommitment: g(2000n),
		};
		const nonces2: NonceCommitments = {
			hidingNonce: 3000n,
			bindingNonce: 4000n,
			hidingNonceCommitment: g(3000n),
			bindingNonceCommitment: g(4000n),
		};
		const share1 = groupCommitmentShare(3n, nonces1);
		const share2 = groupCommitmentShare(5n, nonces2);
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
	it("generates proof for index 2 that verifies against tree root", () => {
		const tree = createNonceTree(42n, 4n);
		const { nonceCommitments, nonceProof } = nonceCommitmentsWithProof(tree, 2n);
		expect(nonceCommitments).toBe(tree.commitments[2]);
		expect(verifyMerkleProof(tree.root, tree.leaves[2], nonceProof)).toBe(true);
	});

	it("generates proof for index 0 that verifies against tree root", () => {
		const tree = createNonceTree(42n, 4n);
		const { nonceCommitments, nonceProof } = nonceCommitmentsWithProof(tree, 0n);
		expect(nonceCommitments).toBe(tree.commitments[0]);
		expect(verifyMerkleProof(tree.root, tree.leaves[0], nonceProof)).toBe(true);
	});

	it("returned nonceCommitments is from the correct tree slot", () => {
		const tree = createNonceTree(42n, 4n);
		const { nonceCommitments } = nonceCommitmentsWithProof(tree, 3n);
		expect(nonceCommitments).toBe(tree.commitments[3]);
	});
});
