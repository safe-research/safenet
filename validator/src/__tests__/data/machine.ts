import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";

export const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});

export const makeGroupSetup = () => ({
	groupId: "0x5afe02",
	participantsRoot: "0x5afe5afe5afe",
	commitments: [TEST_POINT],
	encryptionPublicKey: TEST_POINT,
	pok: {
		r: TEST_POINT,
		mu: 100n,
	},
	poap: ["0x5afe5afe5afe01"],
});
