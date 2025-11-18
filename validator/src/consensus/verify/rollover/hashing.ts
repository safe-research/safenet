import { type Hex, hashTypedData } from "viem";
import type { EpochRolloverPacket } from "./schemas.js";

export const epochRolloverHash = (packet: EpochRolloverPacket): Hex =>
	hashTypedData({
		domain: {
			verifyingContract: packet.domain.consensus,
			chainId: packet.domain.chain,
		},
		types: {
			EpochRollover: [
				{ type: "uint64", name: "activeEpoch" },
				{ type: "uint64", name: "proposedEpoch" },
				{ type: "uint64", name: "rolloverAt" },
				{ type: "uint256", name: "groupKeyX" },
				{ type: "uint256", name: "groupKeyY" },
			],
		},
		primaryType: "EpochRollover",
		message: {
			...packet.rollover,
		},
	});
