import { type Address, type Hex, keccak256, stringToBytes, zeroHash } from "viem";
import { describe, expect, it } from "vitest";
import { log } from "../../__tests__/config.js";
import { createClientStorage } from "../../__tests__/utils.js";
import { addmod, g, toPoint } from "../../frost/math.js";
import type { FrostPoint, SignatureId } from "../../frost/types.js";
import { SigningClient } from "./client.js";
import type { NonceCommitments, PublicNonceCommitments } from "./nonces.js";
import { verifySignature } from "./verify.js";

const TEST_GROUP = {
	groupId: "0x93df36aea8e8fc3d254282cf738cd4171a2675e12ae725680000000000000000",
	participants: [
		"0x17dA3E04a30e9Dec247FddDCbFb7B0497Cd2AF95",
		"0x690f083b2968f6cB0Ab6d8885d563b7977cff43B",
		"0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
		"0xbF4e298652F7e39d9062A4e7ec5C48Bf76e48e10",
		"0xf22BE54C085Dc0621ad076D881de8251c5a25fF1",
	],
	publicKey: toPoint({
		x: 84170342083342046397084658286143833881385705429382200330874980718209595271985n,
		y: 111565381103637648897565888500643513470463855578292478313367060330928451504515n,
	}),
} as const;
const TEST_SIGNERS = [
	{
		account: "0x17dA3E04a30e9Dec247FddDCbFb7B0497Cd2AF95",
		signingShare: 92616603195930045475330214960755134594574429097855230829573477440534317429993n,
		verificationShare: toPoint({
			x: 64261139819204851855244563172704531594599903129651325303989702961646113765865n,
			y: 84617880903058707597561846243195427190847825954421979576005455488635048167951n,
		}),
	},
	{
		account: "0x690f083b2968f6cB0Ab6d8885d563b7977cff43B",
		signingShare: 65840621212864096956224136028940543657870580760596441578126625855708404229905n,
		verificationShare: toPoint({
			x: 37213650586135434554301508857716015703663624186131749523514676647747801089936n,
			y: 6236367474068391151235483003042845133507117409844064500737987761560520755049n,
		}),
	},
	{
		account: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
		signingShare: 68722891159576630907435858937774180708777996115765358145405774196995605039429n,
		verificationShare: toPoint({
			x: 41741098108972902426831876015445239529523912969640162546161086083710807385070n,
			y: 48947717088249892923702441418821968428786331634365135200513906456433783689879n,
		}),
	},
	{
		account: "0xbF4e298652F7e39d9062A4e7ec5C48Bf76e48e10",
		signingShare: 44833201630411931922855141261524414673023026320673166473864650125267998209599n,
		verificationShare: toPoint({
			x: 84877201132516405020234240153119389112001594610742699913395786424745518339279n,
			y: 73082033105851640734385047250007221059892527096280794788900747213539676720136n,
		}),
	},
	{
		account: "0xf22BE54C085Dc0621ad076D881de8251c5a25fF1",
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

// --- Tests ---
describe("signing", () => {
	it("e2e signing flow", async () => {
		const nonceRevealEvent: {
			signatureId: SignatureId;
			signer: Address;
			nonces: PublicNonceCommitments;
		}[] = [];
		const signatureShareEvents: {
			signatureId: SignatureId;
			signer: Address;
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
				account: a.account,
				storage,
				client,
			};
		});
		const groupId = TEST_GROUP.groupId;
		log("------------------------ Inject Nonce Commitments ------------------------");
		for (const { client, storage, account } of clients) {
			const participantIndex = TEST_GROUP.participants.findIndex((p) => p === account);
			const treeInfo = NONCE_TREES[participantIndex];
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
			client.handleNonceCommitmentsHash(groupId, nonceTree.root, 0n);
		}
		log("------------------------ Trigger Signing Request ------------------------");
		const signatureId = "0x0000000000000000000000017fa9385be102ac3eac297483dd6233d62b3e1496";
		const message = keccak256(stringToBytes("Hello, Safenet!"));
		for (const { client, account } of clients) {
			log(`>>>> Signing request to ${account} >>>>`);
			const commitments = client.createNonceCommitments(
				groupId,
				signatureId,
				message,
				0n,
				TEST_GROUP.participants,
				account,
			);
			nonceRevealEvent.push({
				signatureId,
				signer: account,
				nonces: commitments.nonceCommitments,
			});
		}
		log("------------------------ Reveal Nonces ------------------------");
		for (const e of nonceRevealEvent) {
			for (const { client, account } of clients) {
				log(`>>>> Nonce reveal from ${e.signer} to ${account} >>>>`);
				const readyToSubmit = client.handleNonceCommitments(e.signatureId, e.signer, e.nonces, account);
				if (!readyToSubmit) continue;

				const { commitmentShare, signatureShare } = client.createSignatureShare(e.signatureId, account);

				signatureShareEvents.push({
					signatureId: e.signatureId,
					signer: e.signer,
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
