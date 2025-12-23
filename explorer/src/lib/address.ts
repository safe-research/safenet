import { getAddress } from "viem";

export const shortAddress = (address: string): string => {
	const checksummedAddress = getAddress(address);
	return `${checksummedAddress.slice(0, 6)}…${checksummedAddress.slice(-4)}`;
};
