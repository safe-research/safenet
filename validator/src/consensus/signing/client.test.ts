import { type Address, encodeAbiParameters, type Hex, keccak256, parseAbiParameters, stringToBytes, zeroHash } from "viem";
import { describe, expect, it } from "vitest";
import { log } from "../../__tests__/config.js";
import { createClientStorage } from "../../__tests__/utils.js";
import { addmod, g, toPoint } from "../../frost/math.js";
import type { FrostPoint, SignatureId } from "../../frost/types.js";
import { verifyMerkleProof } from "../merkle.js";
import { SigningClient } from "./client.js";
import { groupChallenge } from "./group.js";
import { createNonceTree, type NonceCommitments, type PublicNonceCommitments } from "./nonces.js";
import { lagrangeChallenge } from "./shares.js";
import { verifySignature, verifySignatureShare } from "./verify.js";

const TEST_GROUP = {
	groupId: "0x93df36aea8e8fc3d254282cf738cd4171a2675e12ae725680000000000000000",
	participants: [
		"0x17dA3E04a30e9Dec247FddDCbFb7B0497Cd2AF95",
		"0x690f083b2968f6cB0Ab6d8885d563b7977cff43B",
		"0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
		"0xbF4e298652F7e39d9062A4e7ec5C48Bf76e48e10",
		"0xf22BE54C085Dc0621ad076D881de8251c5a25fF1",
	] as Address[],
	publicKey: toPoint({
		x: 84170342083342046397084658286143833881385705429382200330874980718209595271985n,
		y: 111565381103637648897565888500643513470463855578292478313367060330928451504515n,
	}),
} as const;
const TEST_SIGNERS = [
	{
		account: "0x17dA3E04a30e9Dec247FddDCbFb7B0497Cd2AF95" as Address,
		signingShare: 92616603195930045475330214960755134594574429097855230829573477440534317429993n,
		verificationShare: toPoint({
			x: 64261139819204851855244563172704531594599903129651325303989702961646113765865n,
			y: 84617880903058707597561846243195427190847825954421979576005455488635048167951n,
		}),
	},
	{
		account: "0x690f083b2968f6cB0Ab6d8885d563b7977cff43B" as Address,
		signingShare: 65840621212864096956224136028940543657870580760596441578126625855708404229905n,
		verificationShare: toPoint({
			x: 37213650586135434554301508857716015703663624186131749523514676647747801089936n,
			y: 6236367474068391151235483003042845133507117409844064500737987761560520755049n,
		}),
	},
	{
		account: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D" as Address,
		signingShare: 68722891159576630907435858937774180708777996115765358145405774196995605039429n,
		verificationShare: toPoint({
			x: 41741098108972902426831876015445239529523912969640162546161086083710807385070n,
			y: 48947717088249892923702441418821968428786331634365135200513906456433783689879n,
		}),
	},
	{
		account: "0xbF4e298652F7e39d9062A4e7ec5C48Bf76e48e10" as Address,
		signingShare: 44833201630411931922855141261524414673023026320673166473864650125267998209599n,
		verificationShare: toPoint({
			x: 84877201132516405020234240153119389112001594610742699913395786424745518339279n,
			y: 73082033105851640734385047250007221059892527096280794788900747213539676720136n,
		}),
	},
	{
		account: "0xf22BE54C085Dc0621ad076D881de8251c5a25fF1" as Address,
		signingShare: 13086322243572108858288841808712792921456551656485227174142249973868521057300n,
		verificationShare: toPoint({
			x: 9830174855169825843069194197328411062217074564768016955757420615522985358269n,
			y: 25192176312498426392986427521508055061952211194077411665484541357835081011820n,
		}),
	},
] as const;
const NONCE_TREES = [
	{
		d: 100339483097864921407303963156202886029728085263802626541507900904023147081938n,
		e: 39378701897598841999422172638590664020201030986014526216168600034328350884585n,
		root: "0x8fca9c92bba08607b1ff312404bee8db98335ef66e9f636057c2210dc1016489",
	},
	{
		d: 6926154550275497734730869859891418054250698937672136615156260576950168255450n,
		e: 82083109928449620620617973086496218550996793071149418122332520728518455300173n,
		root: "0xd431b6de53f387dd1e13e97ac45e07afb6cf477012c6651cc9450449c3ca46b7",
	},
	{
		d: 92161213647751105232390981274216540125641236705550063974295768705932428406061n,
		e: 21582462166075747704005326932966632938649118917623681727464400944770714480392n,
		root: "0xc3b358639a362c1c99171bf396c305eaaa814670384c3623ab728c4d9b3dc302",
	},
	{
		d: 23902942957119420487132146098902988419566719008273117425453144185089521906267n,
		e: 51141065954932338734987882292384749540046085425753583299903549290509407237879n,
		root: "0x8fea97c49f45f72277c2a643f53f9a43522adc34a370d958cd28a763e70a94b2",
	},
	{
		d: 109763946547770367946244301537859315415841735216394507767465169783517615867622n,
		e: 7177519604416187819675204115425593011080760721237062629048923898622487474786n,
		root: "0x47341e5da9b21ea5f1695797980d22690cb57d0b517b665c7c4c062273a46bd4",
	},
] as const;

const SIGNATURE_ID = "0x0000000000000000000000017fa9385be102ac3eac297483dd6233d62b3e1496";
const MESSAGE = keccak256(stringToBytes("Hello, Safenet!"));

const createTestClient = (signerIndex: number, threshold = TEST_GROUP.participants.length) => {
	const signer = TEST_SIGNERS[signerIndex];
	const storage = createClientStorage(signer.account);
	storage.registerGroup(TEST_GROUP.groupId, TEST_GROUP.participants, threshold);
	storage.registerVerification(TEST_GROUP.groupId, TEST_GROUP.publicKey, signer.verificationShare);
	storage.registerSigningShare(TEST_GROUP.groupId, signer.signingShare);
	const client = new SigningClient(storage);
	return { storage, client };
};

const createTestClientWithNonces = (signerIndex: number, threshold = TEST_GROUP.participants.length) => {
	const { storage, client } = createTestClient(signerIndex, threshold);
	const treeInfo = NONCE_TREES[signerIndex];
	const commitments0: NonceCommitments = {
		hidingNonce: treeInfo.d,
		bindingNonce: treeInfo.e,
		hidingNonceCommitment: g(treeInfo.d),
		bindingNonceCommitment: g(treeInfo.e),
	};
	const nonceTree = {
		commitments: [commitments0],
		leaves: [zeroHash],
		root: treeInfo.root as Hex,
	};
	storage.registerNonceTree(TEST_GROUP.groupId, nonceTree);
	client.handleNonceCommitmentsHash(TEST_GROUP.groupId, storage.participant(TEST_GROUP.groupId), nonceTree.root, 0n);
	return { storage, client };
};

// --- Tests ---
describe("SigningClient", () => {
	describe("e2e signing flow", () => {
		it("produces a valid FROST signature across all participants", () => {
			const nonceRevealEvent: {
				signatureId: SignatureId;
				signerId: Address;
				nonces: PublicNonceCommitments;
			}[] = [];
			const signatureShareEvents: {
				signatureId: SignatureId;
				signerId: Address;
				z: bigint;
				r: FrostPoint;
			}[] = [];
			const clients = TEST_SIGNERS.map((a) => {
				const storage = createClientStorage(a.account);
				storage.registerGroup(TEST_GROUP.groupId, TEST_GROUP.participants, TEST_GROUP.participants.length);
				storage.registerVerification(TEST_GROUP.groupId, TEST_GROUP.publicKey, a.verificationShare);
				storage.registerSigningShare(TEST_GROUP.groupId, a.signingShare);
				const client = new SigningClient(storage);
				return {
					storage,
					client,
				};
			});
			const groupId = TEST_GROUP.groupId;
			log("------------------------ Inject Nonce Commitments ------------------------");
			for (const { client, storage } of clients) {
				const participant = storage.participant(groupId);
				const treeInfo = NONCE_TREES[TEST_SIGNERS.findIndex((s) => s.account === participant)];
				const commitments0: NonceCommitments = {
					hidingNonce: treeInfo.d,
					bindingNonce: treeInfo.e,
					hidingNonceCommitment: g(treeInfo.d),
					bindingNonceCommitment: g(treeInfo.e),
				};
				const nonceTree = {
					commitments: [commitments0],
					leaves: [zeroHash],
					root: treeInfo.root as Hex,
				};
				storage.registerNonceTree(groupId, nonceTree);
				client.handleNonceCommitmentsHash(groupId, participant, nonceTree.root, 0n);
			}
			log("------------------------ Trigger Signing Request ------------------------");
			const signatureId = SIGNATURE_ID;
			const message = MESSAGE;
			for (const { client, storage } of clients) {
				const participant = storage.participant(groupId);
				log(`>>>> Signing request to ${participant} >>>>`);
				const commitments = client.createNonceCommitments(
					groupId,
					signatureId,
					message,
					0n,
					TEST_GROUP.participants,
				);
				nonceRevealEvent.push({
					signatureId,
					signerId: participant,
					nonces: commitments.nonceCommitments,
				});
			}
			log("------------------------ Reveal Nonces ------------------------");
			for (const e of nonceRevealEvent) {
				for (const { client, storage } of clients) {
					log(`>>>> Nonce reveal from ${e.signerId} to ${storage.participant(groupId)} >>>>`);
					const readyToSubmit = client.handleNonceCommitments(e.signatureId, e.signerId, e.nonces);
					if (!readyToSubmit) continue;

					const { commitmentShare, signatureShare } = client.createSignatureShare(e.signatureId);

					signatureShareEvents.push({
						signatureId: e.signatureId,
						signerId: storage.participant(groupId),
						z: signatureShare,
						r: commitmentShare,
					});
				}
			}
			log("------------------------ Verify Shares ------------------------");
			let r: FrostPoint | null = null;
			let z = 0n;
			for (const e of signatureShareEvents) {
				log({
					e,
				});
				r = r == null ? e.r : r.add(e.r);
				z = addmod(z, e.z);
			}
			if (r == null) throw new Error("r is null");
			expect(verifySignature(r, z, TEST_GROUP.publicKey, message)).toBeTruthy();
		});
	});

	describe("generateNonceTree", () => {
		it("returns a unique root on each call", () => {
			const { client } = createTestClient(0);
			const root1 = client.generateNonceTree(TEST_GROUP.groupId);
			const root2 = client.generateNonceTree(TEST_GROUP.groupId);
			expect(root1).not.toBe(root2);
		});

		it("throws when the signing share is missing", () => {
			const signer = TEST_SIGNERS[0];
			const storage = createClientStorage(signer.account);
			storage.registerGroup(TEST_GROUP.groupId, TEST_GROUP.participants, TEST_GROUP.participants.length);
			// Do not register signing share
			const client = new SigningClient(storage);
			expect(() => client.generateNonceTree(TEST_GROUP.groupId)).toThrow();
		});
	});

	describe("handleNonceCommitmentsHash", () => {
		it("links nonce tree when sender is own participant ID", () => {
			const { client, storage } = createTestClient(0);
			const ownAddress = storage.participant(TEST_GROUP.groupId);

			const treeRoot = client.generateNonceTree(TEST_GROUP.groupId);
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, ownAddress, treeRoot, 0n);

			expect(storage.nonceTree(TEST_GROUP.groupId, 0n)).toBeDefined();
		});

		it("ignores nonce commitments hash from other participants", () => {
			const { client, storage } = createTestClient(0);
			const ownAddress = storage.participant(TEST_GROUP.groupId);
			const otherAddress = TEST_GROUP.participants.find((p) => p !== ownAddress) as Address;

			const treeRoot = client.generateNonceTree(TEST_GROUP.groupId);

			// Should silently skip — nothing gets linked
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, otherAddress, treeRoot, 0n);

			expect(() => storage.nonceTree(TEST_GROUP.groupId, 0n)).toThrow();
		});
	});

	describe("createNonceCommitments", () => {
		it("returns nonce commitments and merkle proof", () => {
			const { client, storage } = createTestClient(0);
			const ownAddress = storage.participant(TEST_GROUP.groupId);
			const signers = TEST_GROUP.participants;

			// Generate and link a real nonce tree
			const root = client.generateNonceTree(TEST_GROUP.groupId);
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, ownAddress, root, 0n);

			const result = client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers);

			// Retrieve the stored nonce tree to access actual scalar nonces
			const nonceTree = storage.nonceTree(TEST_GROUP.groupId, 0n);
			const { hidingNonce, bindingNonce } = nonceTree.commitments[0];

			// Verify D = d·G and E = e·G
			expect(result.nonceCommitments.hidingNonceCommitment).toEqual(g(hidingNonce));
			expect(result.nonceCommitments.bindingNonceCommitment).toEqual(g(bindingNonce));

			// Verify the merkle proof is valid
			expect(verifyMerkleProof(root, nonceTree.leaves[0], result.nonceProof)).toBe(true);
		});

		it("throws when signers count is below threshold", () => {
			const { client } = createTestClientWithNonces(0);
			// Threshold is 5 (all participants), so providing only 2 signers should fail
			const signers = TEST_GROUP.participants.slice(0, 2);

			expect(() => client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers)).toThrow(
				"Not enough signers to start signing process",
			);
		});

		it("throws when signer ID is not a valid participant", () => {
			const { client } = createTestClientWithNonces(0);
			const invalidAddress = "0x0000000000000000000000000000000000000999" as Address;
			const signers = [...TEST_GROUP.participants.slice(0, 4), invalidAddress];

			expect(() => client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers)).toThrow(
				"Invalid signer id provided:",
			);
		});

		it("stores the signature request for later retrieval", () => {
			const { client } = createTestClientWithNonces(0);
			const signers = TEST_GROUP.participants;

			client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers);

			// After creating nonce commitments, we should be able to query signers and signing group
			expect(client.signers(SIGNATURE_ID)).toEqual(signers);
			expect(client.signingGroup(SIGNATURE_ID)).toBe(TEST_GROUP.groupId);
		});
	});

	describe("handleNonceCommitments", () => {
		it("returns false for own nonce commitments", () => {
			const { client, storage } = createTestClientWithNonces(0);
			const ownAddress = storage.participant(TEST_GROUP.groupId);
			const signers = TEST_GROUP.participants;
			const { nonceCommitments } = client.createNonceCommitments(
				TEST_GROUP.groupId,
				SIGNATURE_ID,
				MESSAGE,
				0n,
				signers,
			);

			const result = client.handleNonceCommitments(SIGNATURE_ID, ownAddress, nonceCommitments);
			expect(result).toBe(false);
		});

		it("returns false when not all nonces have been received", () => {
			const { client: client0 } = createTestClientWithNonces(0);
			const { client: client1 } = createTestClientWithNonces(1);
			const signers = TEST_GROUP.participants;

			// Set up signature request on client 0
			client0.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers);

			// Get nonce commitments from client 1
			const { nonceCommitments: nonces1 } = client1.createNonceCommitments(
				TEST_GROUP.groupId,
				SIGNATURE_ID,
				MESSAGE,
				0n,
				signers,
			);

			// Only one peer's nonces received, not complete yet
			const result = client0.handleNonceCommitments(SIGNATURE_ID, TEST_SIGNERS[1].account, nonces1);
			expect(result).toBe(false);
		});

		it("returns true when all nonces have been received", () => {
			const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));
			const signers = TEST_GROUP.participants;

			// All clients create nonce commitments
			const allNonces: { signerId: Address; nonces: PublicNonceCommitments }[] = [];
			for (const { client, storage } of allClients) {
				const { nonceCommitments } = client.createNonceCommitments(
					TEST_GROUP.groupId,
					SIGNATURE_ID,
					MESSAGE,
					0n,
					signers,
				);
				allNonces.push({
					signerId: storage.participant(TEST_GROUP.groupId),
					nonces: nonceCommitments,
				});
			}

			// Feed all peer nonces to client 0
			const { client: targetClient } = allClients[0];
			const results = allNonces.map(({ signerId, nonces }) =>
				targetClient.handleNonceCommitments(SIGNATURE_ID, signerId, nonces),
			);

			// Ready state should have been reached at some point during nonce collection
			expect(results.includes(true)).toBe(true);
		});
	});

	describe("createSignatureShare", () => {
		const setupFullCeremony = () => {
			const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));
			const signers = TEST_GROUP.participants;

			// All clients create nonce commitments
			const allNonces: { signerId: Address; nonces: PublicNonceCommitments }[] = [];
			for (const { client, storage } of allClients) {
				const { nonceCommitments } = client.createNonceCommitments(
					TEST_GROUP.groupId,
					SIGNATURE_ID,
					MESSAGE,
					0n,
					signers,
				);
				allNonces.push({
					signerId: storage.participant(TEST_GROUP.groupId),
					nonces: nonceCommitments,
				});
			}

			// Feed all nonces to all clients
			for (const { client } of allClients) {
				for (const { signerId, nonces } of allNonces) {
					client.handleNonceCommitments(SIGNATURE_ID, signerId, nonces);
				}
			}

			return allClients;
		};

		it("returns a valid signature share with required fields", () => {
			const allClients = setupFullCeremony();
			const { client, storage } = allClients[0];
			const signerAddress = storage.participant(TEST_GROUP.groupId);

			const result = client.createSignatureShare(SIGNATURE_ID);

			// Verify the signers merkle proof is valid for this signer's leaf
			const leaf = keccak256(
				encodeAbiParameters(parseAbiParameters("address, uint256, uint256, uint256, uint256, uint256"), [
					signerAddress,
					result.commitmentShare.x,
					result.commitmentShare.y,
					result.lagrangeCoefficient,
					result.groupCommitment.x,
					result.groupCommitment.y,
				]),
			);
			expect(verifyMerkleProof(result.signersRoot, leaf, result.signersProof)).toBe(true);

			// Verify the FROST signature share
			const challenge = groupChallenge(result.groupCommitment, TEST_GROUP.publicKey, MESSAGE);
			const cl = lagrangeChallenge(result.lagrangeCoefficient, challenge);
			expect(
				verifySignatureShare(result.signatureShare, TEST_SIGNERS[0].verificationShare, cl, result.commitmentShare),
			).toBe(true);
		});

		it("each signature share is individually verifiable", () => {
			const allClients = setupFullCeremony();

			for (const [i, { client }] of allClients.entries()) {
				const result = client.createSignatureShare(SIGNATURE_ID);
				const challenge = groupChallenge(result.groupCommitment, TEST_GROUP.publicKey, MESSAGE);
				const cl = lagrangeChallenge(result.lagrangeCoefficient, challenge);
				expect(
					verifySignatureShare(result.signatureShare, TEST_SIGNERS[i].verificationShare, cl, result.commitmentShare),
				).toBe(true);
			}
		});

		it("produces consistent group commitment across all participants", () => {
			const allClients = setupFullCeremony();

			const groupCommitments = allClients.map(({ client }) => {
				const result = client.createSignatureShare(SIGNATURE_ID);
				return result.groupCommitment;
			});

			// All participants should compute the same group commitment
			for (const commitment of groupCommitments.slice(1)) {
				expect({ x: commitment.x, y: commitment.y }).toEqual({ x: groupCommitments[0].x, y: groupCommitments[0].y });
			}
		});

		it("produces unique commitment shares per participant", () => {
			const allClients = setupFullCeremony();

			const commitmentShares = allClients.map(({ client }) => {
				const result = client.createSignatureShare(SIGNATURE_ID);
				return result.commitmentShare;
			});

			// Each participant should have a unique commitment share
			for (let i = 0; i < commitmentShares.length; i++) {
				for (let j = i + 1; j < commitmentShares.length; j++) {
					expect({ x: commitmentShares[i].x, y: commitmentShares[i].y }).not.toEqual({
						x: commitmentShares[j].x,
						y: commitmentShares[j].y,
					});
				}
			}
		});

		it("produces consistent signers root across all participants", () => {
			// The signers root is determined by the nonce commitments exchanged via createNonceCommitments
			const signers = TEST_GROUP.participants;
			const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));

			const allNonces = allClients.map(({ client, storage }) => ({
				signerId: storage.participant(TEST_GROUP.groupId),
				nonces: client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers).nonceCommitments,
			}));

			for (const { client } of allClients) {
				for (const { signerId, nonces } of allNonces) {
					client.handleNonceCommitments(SIGNATURE_ID, signerId, nonces);
				}
			}

			const roots = allClients.map(({ client }) => client.createSignatureShare(SIGNATURE_ID).signersRoot);
			for (const root of roots.slice(1)) {
				expect(root).toBe(roots[0]);
			}
		});

		it("burns nonces after creating signature share", () => {
			const allClients = setupFullCeremony();
			const { client, storage: ownStorage } = allClients[0];
			const ownAddress = ownStorage.participant(TEST_GROUP.groupId);

			client.createSignatureShare(SIGNATURE_ID);

			// Attempting to use the same nonces again should throw
			// We need to set up a new signature request with the same sequence
			const signers = TEST_GROUP.participants;
			const newSigId = "0x0000000000000000000000027fa9385be102ac3eac297483dd6233d62b3e1496";
			client.createNonceCommitments(TEST_GROUP.groupId, newSigId, MESSAGE, 0n, signers);

			// Feed nonces for new signature
			for (const { client: peerClient, storage } of allClients) {
				const peerAddress = storage.participant(TEST_GROUP.groupId);
				if (peerAddress === ownAddress) continue;
				const { nonceCommitments } = peerClient.createNonceCommitments(
					TEST_GROUP.groupId,
					newSigId,
					MESSAGE,
					0n,
					signers,
				);
				client.handleNonceCommitments(newSigId, peerAddress, nonceCommitments);
			}

			// Should throw because nonces at sequence 0 were already burned
			expect(() => client.createSignatureShare(newSigId)).toThrow("already burned");
		});
	});

	describe("availableNoncesCount", () => {
		it("returns 0 when no nonce tree exists", () => {
			const { client } = createTestClient(0);
			expect(client.availableNoncesCount(TEST_GROUP.groupId, 0n)).toBe(0n);
		});

		it("returns the number of nonces after tree generation", () => {
			const { client, storage } = createTestClient(0);
			const ownAddress = storage.participant(TEST_GROUP.groupId);
			const nonceTree = createNonceTree(TEST_SIGNERS[0].signingShare, 4n);
			storage.registerNonceTree(TEST_GROUP.groupId, nonceTree);
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, ownAddress, nonceTree.root, 0n);
			expect(client.availableNoncesCount(TEST_GROUP.groupId, 0n)).toBe(4n);
		});

		it("returns 0 for non-existent chunk", () => {
			const { client } = createTestClientWithNonces(0);
			expect(client.availableNoncesCount(TEST_GROUP.groupId, 999n)).toBe(0n);
		});
	});

	describe("threshold signing with subset of participants", () => {
		it("produces a valid signature with threshold-many signers when threshold < n", () => {
			// Create a group with threshold 3 out of 5
			const threshold = 3;
			const subsetClients = TEST_SIGNERS.slice(0, threshold).map((_, i) => createTestClientWithNonces(i, threshold));

			const signers = TEST_SIGNERS.slice(0, threshold).map((s) => s.account);
			const allNonces: { signerId: Address; nonces: PublicNonceCommitments }[] = [];

			for (const { client, storage } of subsetClients) {
				const { nonceCommitments } = client.createNonceCommitments(
					TEST_GROUP.groupId,
					SIGNATURE_ID,
					MESSAGE,
					0n,
					signers,
				);
				allNonces.push({
					signerId: storage.participant(TEST_GROUP.groupId),
					nonces: nonceCommitments,
				});
			}

			for (const { client } of subsetClients) {
				for (const { signerId, nonces } of allNonces) {
					client.handleNonceCommitments(SIGNATURE_ID, signerId, nonces);
				}
			}

			let r: FrostPoint | null = null;
			let z = 0n;
			for (const { client } of subsetClients) {
				const { commitmentShare, signatureShare } = client.createSignatureShare(SIGNATURE_ID);
				r = r == null ? commitmentShare : r.add(commitmentShare);
				z = addmod(z, signatureShare);
			}

			if (r == null) throw new Error("r is null");
			expect(verifySignature(r, z, TEST_GROUP.publicKey, MESSAGE)).toBeTruthy();
		});
	});

	describe("signing with different messages", () => {
		it("produces different signatures for different messages", () => {
			const signMessage = (message: Hex) => {
				const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));
				const signers = TEST_GROUP.participants;
				const sigId = keccak256(stringToBytes(`sig-${message}`));

				const allNonces: { signerId: Address; nonces: PublicNonceCommitments }[] = [];
				for (const { client, storage } of allClients) {
					const { nonceCommitments } = client.createNonceCommitments(TEST_GROUP.groupId, sigId, message, 0n, signers);
					allNonces.push({
						signerId: storage.participant(TEST_GROUP.groupId),
						nonces: nonceCommitments,
					});
				}

				for (const { client } of allClients) {
					for (const { signerId, nonces } of allNonces) {
						client.handleNonceCommitments(sigId, signerId, nonces);
					}
				}

				let r: FrostPoint | null = null;
				let z = 0n;
				for (const { client } of allClients) {
					const { commitmentShare, signatureShare } = client.createSignatureShare(sigId);
					r = r == null ? commitmentShare : r.add(commitmentShare);
					z = addmod(z, signatureShare);
				}
				if (r == null) throw new Error("r is null");
				return { r, z };
			};

			const msg1 = keccak256(stringToBytes("message A"));
			const msg2 = keccak256(stringToBytes("message B"));
			const sig1 = signMessage(msg1);
			const sig2 = signMessage(msg2);

			// Signatures should differ
			expect(sig1.z).not.toBe(sig2.z);

			// Both should verify against their respective messages
			expect(verifySignature(sig1.r, sig1.z, TEST_GROUP.publicKey, msg1)).toBeTruthy();
			expect(verifySignature(sig2.r, sig2.z, TEST_GROUP.publicKey, msg2)).toBeTruthy();

			// Cross-verification should fail
			expect(verifySignature(sig1.r, sig1.z, TEST_GROUP.publicKey, msg2)).toBeFalsy();
			expect(verifySignature(sig2.r, sig2.z, TEST_GROUP.publicKey, msg1)).toBeFalsy();
		});
	});
});
