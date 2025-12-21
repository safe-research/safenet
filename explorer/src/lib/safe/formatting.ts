import { formatUnits, type Hex, size } from "viem";

export type Currency = {
	name: string;
	/** 2-6 characters long */
	symbol: string;
	decimals: number;
};

export const opString = (operation: 0 | 1) => (operation === 0 ? "CALL" : "DELEGATECALL");
export const valueString = (value: bigint, currency?: Currency) =>
	`${formatUnits(value, currency?.decimals ?? 18)} ${currency?.symbol ?? "ETH"}`;
export const dataString = (data: Hex) => `${size(data)} bytes of data`;
