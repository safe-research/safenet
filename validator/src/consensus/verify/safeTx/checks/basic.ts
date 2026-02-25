import {
	type ContractFunctionArgs,
	decodeFunctionData,
	type Hex,
	type ParseAbiItem,
	parseAbiItem,
	size,
	toFunctionSelector,
} from "viem";
import type { TransactionCheck } from "../handler.js";
import type { SafeTransaction } from "../schemas.js";
import { TransactionCheckError } from "./errors.js";

export const buildNoDelegateCallCheck = () => (tx: SafeTransaction) => {
	if (tx.operation !== 0) throw new TransactionCheckError("no_delegatecall", "Delegatecall not allowed");
};

export const buildSelfCheck = (check: TransactionCheck) => (tx: SafeTransaction) => {
	// Checks should only be applied to self transactions
	if (tx.to !== tx.safe) return;
	check(tx);
};

export const buildFixedParamsCheck =
	(params: Partial<Pick<SafeTransaction, "to" | "value" | "data" | "operation">>): TransactionCheck =>
	(tx: SafeTransaction) => {
		for (const field of ["to", "value", "data", "operation"] as const) {
			if (params[field] !== undefined && tx[field] !== params[field]) {
				throw new Error(`Expected ${field} ${params[field]} got ${tx[field]}`);
			}
		}
	};

export const buildSupportedSelectorCheck =
	(selectors: readonly Hex[], allowEmpty: boolean): TransactionCheck =>
	(tx: SafeTransaction) => {
		const dataSize = size(tx.data);
		if (dataSize === 0 && allowEmpty) return;
		if (dataSize < 4) {
			throw new Error(`${tx.data} is not a valid selector`);
		}
		const selector = tx.data.slice(0, 10) as Hex;
		if (!selectors.includes(selector)) {
			throw new Error(`${selector} not supported`);
		}
	};

export const buildSupportedSignaturesCheck = (signatures: readonly string[], allowEmpty = true): TransactionCheck =>
	buildSupportedSelectorCheck(
		signatures.map((s) => toFunctionSelector(s)),
		allowEmpty,
	);

type SelectorChecks = Record<string, TransactionCheck>;

export function buildSelectorCheck<S extends string>(
	signature: S,
	handler: (args: ContractFunctionArgs<[ParseAbiItem<S>], "nonpayable">) => void,
): SelectorChecks {
	const selectorCheck: SelectorChecks = {};
	const abi = parseAbiItem(signature as string) as ParseAbiItem<S>;
	const selector = toFunctionSelector(signature);
	selectorCheck[selector] = (tx: SafeTransaction): void => {
		const parsedData = decodeFunctionData<ParseAbiItem<S>[]>({ abi: [abi], data: tx.data });
		handler(parsedData.args);
	};
	return selectorCheck;
}

export const buildSelectorChecks =
	(selectorChecks: Readonly<SelectorChecks>, allowEmpty: boolean, fallbackCheck?: TransactionCheck): TransactionCheck =>
	(tx: SafeTransaction): void => {
		const dataSize = size(tx.data);
		if (dataSize === 0 && allowEmpty) return;
		if (dataSize < 4) {
			throw new Error(`${tx.data} is not a valid selector`);
		}
		const selector = tx.data.slice(0, 10) as Hex;
		const selectorCheck = selectorChecks[selector] ?? fallbackCheck;
		if (selectorCheck === undefined) {
			throw new Error(`${selector} not supported`);
		}
		selectorCheck(tx);
	};
