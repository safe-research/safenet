import { type Address, encodePacked, type Hex, keccak256, stringToBytes, zeroHash } from "viem";
import { describe, expect, it } from "vitest";
import { createClientStorage, log } from "../../__tests__/config.js";
import { addmod, g, toPoint } from "../../frost/math.js";
import type { FrostPoint, ParticipantId, SignatureId } from "../../frost/types.js";
import { verifyMerkleProof } from "../merkle.js";
import { SigningClient } from "./client.js";
import { groupChallenge } from "./group.js";
import { createNonceTree, type NonceCommitments, type PublicNonceCommitments } from "./nonces.js";
import { lagrangeChallenge } from "./shares.js";
import { verifySignature, verifySignatureShare } from "./verify.js";

const TEST_GROUP = {
	groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496" as Hex,
	participants: [
		{
			id: 1n,
			address: "0x17dA3E04a30e9Dec247FddDCbFb7B0497Cd2AF95" as Address,
		},
		{
			id: 2n,
			address: "0x690f083b2968f6cB0Ab6d8885d563b7977cff43B" as Address,
		},
		{
			id: 3n,
			address: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D" as Address,
		},
		{
			id: 4n,
			address: "0xbF4e298652F7e39d9062A4e7ec5C48Bf76e48e10" as Address,
		},
		{
			id: 5n,
			address: "0xf22BE54C085Dc0621ad076D881de8251c5a25fF1" as Address,
		},
	],
	publicKey: toPoint({
		x: 71064083542762312543389424882566275462227917749849605078973795482529746018304n,
		y: 18516174593957712908408406456950733439418726956735133896250976920482937040840n,
	}),
};
const TEST_SIGNERS = [
	{
		account: "0x17dA3E04a30e9Dec247FddDCbFb7B0497Cd2AF95" as Address,
		signingShare: 20562999615202090641202256481184490375429435244238288544262716592143955696382n,
		participantId: 1n,
		verificationShare: toPoint({
			x: 8157951670743782207572742157759285246997125817591478561509454646417563755134n,
			y: 56888799465634869784517292721691123160415451366201038719887189136540242661500n,
		}),
	},
	{
		account: "0x690f083b2968f6cB0Ab6d8885d563b7977cff43B" as Address,
		signingShare: 11521112607527998281706776924866429794302295528584870815702418782215238588532n,
		participantId: 2n,
		verificationShare: toPoint({
			x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
			y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
		}),
	},
	{
		account: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D" as Address,
		signingShare: 49315800323439827956806304959772670422395371345712878244203054714816206370500n,
		participantId: 3n,
		verificationShare: toPoint({
			x: 44679288968503427008336055401348610670311019206231050966573026822674597087871n,
			y: 55755209342996270094025410798290967844706637348941476346623362199006170171687n,
		}),
	},
	{
		account: "0xbF4e298652F7e39d9062A4e7ec5C48Bf76e48e10" as Address,
		signingShare: 18154973525621384242929855577215304406871098416547406447159461248428697547949n,
		participantId: 4n,
		verificationShare: toPoint({
			x: 112111548805574036052056537155641327571521863544152157231564193075408059401719n,
			y: 76557092302104387621595723426764926750450467869008997389281566585102109438507n,
		}),
	},
	{
		account: "0xf22BE54C085Dc0621ad076D881de8251c5a25fF1" as Address,
		signingShare: 33830721451388862563648413785882239600567041020163359807176801524570873615216n,
		participantId: 5n,
		verificationShare: toPoint({
			x: 105587021125387004117772930966558154492652686110919450580386247155506502192059n,
			y: 97790146336079427917878178932139533907352200097479391118658154349645214584696n,
		}),
	},
];
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
];

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

const createTestClientWithNonces = (signerIndex: number) => {
	const { storage, client } = createTestClient(signerIndex);
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
	client.handleNonceCommitmentsHash(TEST_GROUP.groupId, storage.participantId(TEST_GROUP.groupId), nonceTree.root, 0n);
	return { storage, client };
};

// --- Tests ---
describe("SigningClient", () => {
	describe("e2e signing flow", () => {
		it("produces a valid FROST signature across all participants", () => {
			const nonceRevealEvent: {
				signatureId: SignatureId;
				signerId: ParticipantId;
				nonces: PublicNonceCommitments;
			}[] = [];
			const signatureShareEvents: {
				signatureId: SignatureId;
				signerId: ParticipantId;
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
				const participantId = storage.participantId(groupId);
				const treeInfo = NONCE_TREES[Number(participantId) - 1];
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
				client.handleNonceCommitmentsHash(groupId, participantId, nonceTree.root, 0n);
			}
			log("------------------------ Trigger Signing Request ------------------------");
			const signatureId = SIGNATURE_ID;
			const message = MESSAGE;
			for (const { client, storage } of clients) {
				log(`>>>> Signing request to ${storage.participantId(groupId)} >>>>`);
				const commitments = client.createNonceCommitments(
					groupId,
					signatureId,
					message,
					0n,
					TEST_GROUP.participants.map((p) => p.id),
				);
				nonceRevealEvent.push({
					signatureId,
					signerId: storage.participantId(groupId),
					nonces: commitments.nonceCommitments,
				});
			}
			log("------------------------ Reveal Nonces ------------------------");
			for (const e of nonceRevealEvent) {
				for (const { client, storage } of clients) {
					log(`>>>> Nonce reveal from ${e.signerId} to ${storage.participantId(groupId)} >>>>`);
					const readyToSubmit = client.handleNonceCommitments(e.signatureId, e.signerId, e.nonces);
					if (!readyToSubmit) continue;

					const { commitmentShare, signatureShare } = client.createSignatureShare(e.signatureId);

					signatureShareEvents.push({
						signatureId: e.signatureId,
						signerId: storage.participantId(groupId),
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
			const ownId = storage.participantId(TEST_GROUP.groupId);

			const treeRoot = client.generateNonceTree(TEST_GROUP.groupId);
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, ownId, treeRoot, 0n);

			expect(storage.nonceTree(TEST_GROUP.groupId, 0n)).toBeDefined();
		});

		it("ignores nonce commitments hash from other participants", () => {
			const { client, storage } = createTestClient(0);
			const ownId = storage.participantId(TEST_GROUP.groupId);
			const otherId = ownId + 1n;

			const treeRoot = client.generateNonceTree(TEST_GROUP.groupId);

			// Should silently skip — nothing gets linked
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, otherId, treeRoot, 0n);

			expect(() => storage.nonceTree(TEST_GROUP.groupId, 0n)).toThrow();
		});
	});

	describe("createNonceCommitments", () => {
		it("returns nonce commitments and merkle proof", () => {
			const { client, storage } = createTestClient(0);
			const ownId = storage.participantId(TEST_GROUP.groupId);
			const signers = TEST_GROUP.participants.map((p) => p.id);

			// Generate and link a real nonce tree
			const root = client.generateNonceTree(TEST_GROUP.groupId);
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, ownId, root, 0n);

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
			const signers = [1n, 2n];

			expect(() => client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers)).toThrow(
				"Not enough signers to start signing process",
			);
		});

		it("throws when signer ID is not a valid participant", () => {
			const { client } = createTestClientWithNonces(0);
			const signers = [1n, 2n, 3n, 4n, 999n]; // 999n is not a participant

			expect(() => client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers)).toThrow(
				"Invalid signer id provided: 999",
			);
		});

		it("stores the signature request for later retrieval", () => {
			const { client } = createTestClientWithNonces(0);
			const signers = TEST_GROUP.participants.map((p) => p.id);

			client.createNonceCommitments(TEST_GROUP.groupId, SIGNATURE_ID, MESSAGE, 0n, signers);

			// After creating nonce commitments, we should be able to query signers and signing group
			expect(client.signers(SIGNATURE_ID)).toEqual(signers);
			expect(client.signingGroup(SIGNATURE_ID)).toBe(TEST_GROUP.groupId);
		});
	});

	describe("handleNonceCommitments", () => {
		it("returns false for own nonce commitments", () => {
			const { client, storage } = createTestClientWithNonces(0);
			const ownId = storage.participantId(TEST_GROUP.groupId);
			const signers = TEST_GROUP.participants.map((p) => p.id);
			const { nonceCommitments } = client.createNonceCommitments(
				TEST_GROUP.groupId,
				SIGNATURE_ID,
				MESSAGE,
				0n,
				signers,
			);

			const result = client.handleNonceCommitments(SIGNATURE_ID, ownId, nonceCommitments);
			expect(result).toBe(false);
		});

		it("returns false when not all nonces have been received", () => {
			const { client: client0 } = createTestClientWithNonces(0);
			const { client: client1 } = createTestClientWithNonces(1);
			const signers = TEST_GROUP.participants.map((p) => p.id);

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
			const result = client0.handleNonceCommitments(SIGNATURE_ID, 2n, nonces1);
			expect(result).toBe(false);
		});

		it("returns true when all nonces have been received", () => {
			const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));
			const signers = TEST_GROUP.participants.map((p) => p.id);

			// All clients create nonce commitments
			const allNonces: { signerId: ParticipantId; nonces: PublicNonceCommitments }[] = [];
			for (const { client, storage } of allClients) {
				const { nonceCommitments } = client.createNonceCommitments(
					TEST_GROUP.groupId,
					SIGNATURE_ID,
					MESSAGE,
					0n,
					signers,
				);
				allNonces.push({
					signerId: storage.participantId(TEST_GROUP.groupId),
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
		function setupFullCeremony() {
			const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));
			const signers = TEST_GROUP.participants.map((p) => p.id);

			// All clients create nonce commitments
			const allNonces: { signerId: ParticipantId; nonces: PublicNonceCommitments }[] = [];
			for (const { client, storage } of allClients) {
				const { nonceCommitments } = client.createNonceCommitments(
					TEST_GROUP.groupId,
					SIGNATURE_ID,
					MESSAGE,
					0n,
					signers,
				);
				allNonces.push({
					signerId: storage.participantId(TEST_GROUP.groupId),
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
		}

		it("returns a valid signature share with required fields", () => {
			const allClients = setupFullCeremony();
			const { client, storage } = allClients[0];
			const signerId = storage.participantId(TEST_GROUP.groupId);

			const result = client.createSignatureShare(SIGNATURE_ID);

			// Verify the signers merkle proof is valid for this signer's leaf
			const leaf = keccak256(
				encodePacked(
					["uint256", "uint256", "uint256", "uint256", "uint256", "uint256"],
					[
						signerId,
						result.commitmentShare.x,
						result.commitmentShare.y,
						result.lagrangeCoefficient,
						result.groupCommitment.x,
						result.groupCommitment.y,
					],
				),
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
			for (let i = 1; i < groupCommitments.length; i++) {
				expect(groupCommitments[i].x).toBe(groupCommitments[0].x);
				expect(groupCommitments[i].y).toBe(groupCommitments[0].y);
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
			const signers = TEST_GROUP.participants.map((p) => p.id);
			const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));

			const allNonces = allClients.map(({ client, storage }) => ({
				signerId: storage.participantId(TEST_GROUP.groupId),
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
			const ownId = ownStorage.participantId(TEST_GROUP.groupId);

			client.createSignatureShare(SIGNATURE_ID);

			// Attempting to use the same nonces again should throw
			// We need to set up a new signature request with the same sequence
			const signers = TEST_GROUP.participants.map((p) => p.id);
			const newSigId = "0x0000000000000000000000027fa9385be102ac3eac297483dd6233d62b3e1496";
			client.createNonceCommitments(TEST_GROUP.groupId, newSigId, MESSAGE, 0n, signers);

			// Feed nonces for new signature
			for (const { client: peerClient, storage } of allClients) {
				const peerId = storage.participantId(TEST_GROUP.groupId);
				if (peerId === ownId) continue;
				const { nonceCommitments } = peerClient.createNonceCommitments(
					TEST_GROUP.groupId,
					newSigId,
					MESSAGE,
					0n,
					signers,
				);
				client.handleNonceCommitments(newSigId, peerId, nonceCommitments);
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
			const ownId = storage.participantId(TEST_GROUP.groupId);
			const nonceTree = createNonceTree(TEST_SIGNERS[0].signingShare, 4n);
			storage.registerNonceTree(TEST_GROUP.groupId, nonceTree);
			client.handleNonceCommitmentsHash(TEST_GROUP.groupId, ownId, nonceTree.root, 0n);
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
			const subsetClients = TEST_SIGNERS.slice(0, threshold).map((_, i) => {
				const { client, storage } = createTestClient(i, threshold);
				const ownId = storage.participantId(TEST_GROUP.groupId);
				const root = client.generateNonceTree(TEST_GROUP.groupId);
				client.handleNonceCommitmentsHash(TEST_GROUP.groupId, ownId, root, 0n);
				return { client, storage };
			});

			const signers = TEST_SIGNERS.slice(0, threshold).map((s) => s.participantId);
			const allNonces: { signerId: ParticipantId; nonces: PublicNonceCommitments }[] = [];

			for (const { client, storage } of subsetClients) {
				const { nonceCommitments } = client.createNonceCommitments(
					TEST_GROUP.groupId,
					SIGNATURE_ID,
					MESSAGE,
					0n,
					signers,
				);
				allNonces.push({
					signerId: storage.participantId(TEST_GROUP.groupId),
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
			function signMessage(message: Hex) {
				const allClients = TEST_SIGNERS.map((_, i) => createTestClientWithNonces(i));
				const signers = TEST_GROUP.participants.map((p) => p.id);
				const sigId = keccak256(stringToBytes(`sig-${message}`));

				const allNonces: { signerId: ParticipantId; nonces: PublicNonceCommitments }[] = [];
				for (const { client, storage } of allClients) {
					const { nonceCommitments } = client.createNonceCommitments(TEST_GROUP.groupId, sigId, message, 0n, signers);
					allNonces.push({
						signerId: storage.participantId(TEST_GROUP.groupId),
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
			}

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
