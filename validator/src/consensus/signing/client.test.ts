import { type Address, type Hex, keccak256, stringToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { createStorage, log } from "../../__tests__/config.js";
import { addmod, g, toPoint } from "../../frost/math.js";
import type { FrostPoint, ParticipantId, SignatureId } from "../../frost/types.js";
import { SigningClient } from "./client.js";
import type { NonceCommitments, PublicNonceCommitments } from "./nonces.js";
import { verifySignature } from "./verify.js";

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

// --- Tests ---
describe("signing", () => {
	it("e2e signing flow", async () => {
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
			const storage = createStorage(a.account);
			storage.registerGroup(TEST_GROUP.groupId, TEST_GROUP.participants, BigInt(TEST_GROUP.participants.length));
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
				leaves: ["0x" as Hex],
				root: treeInfo.root as Hex,
			};
			storage.registerNonceTree(groupId, nonceTree);
			client.handleNonceCommitmentsHash(groupId, participantId, nonceTree.root, 0n);
		}
		log("------------------------ Trigger Signing Request ------------------------");
		const signatureId = "0x0000000000000000000000017fa9385be102ac3eac297483dd6233d62b3e1496";
		const message = keccak256(stringToBytes("Hello, Shieldnet!"));
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
					signerId: e.signerId,
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
