import { parseAbi } from "viem";

// TODO: placeholder
export const CONSENSUS_CORE_ABI = parseAbi([
	"event Transfer(address indexed from, address indexed to, uint256 value)",
]);
