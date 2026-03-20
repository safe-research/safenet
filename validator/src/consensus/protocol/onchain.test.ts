import {
	type Account,
	type Chain,
	type FeeValuesEIP1559,
	keccak256,
	NonceTooLowError,
	type PublicClient,
	type SendTransactionParameters,
	TransactionExecutionError,
	type Transport,
} from "viem";
import { entryPoint09Address } from "viem/account-abstraction";
import { gnosisChiado } from "viem/chains";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { testLogger } from "../../__tests__/config.js";
import { TEST_ACTIONS, TEST_CONSENSUS, TEST_COORDINATOR } from "../../__tests__/data/protocol.js";
import { createActionQueue } from "../../__tests__/utils.js";
import type { ValidatorAccount } from "../../types/account.js";
import { GasFeeEstimator, OnchainProtocol, type TransactionStorage } from "./onchain.js";

describe("OnchainProtocol", () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	function createTestContext(overrides?: { chain?: Chain; blocksBeforeResubmit?: bigint }) {
		const queue = createActionQueue();

		const chain = overrides?.chain ?? gnosisChiado;

		const publicClient = {
			chain,
			getTransactionCount: vi.fn(),
			sendRawTransaction: vi.fn(),
		};
		const account = {
			address: entryPoint09Address,
			signTransaction: vi.fn(),
		};
		const gasFeeEstimator = {
			estimateFees: vi.fn(),
		};
		const txStorage = {
			countPending: vi.fn(),
			submittedUpTo: vi.fn(),
			setSubmittedForPending: vi.fn(),
			setExecutedUpTo: vi.fn(),
			setPending: vi.fn(),
			setFees: vi.fn(),
			setHash: vi.fn(),
			register: vi.fn(),
			delete: vi.fn(),
			maxNonce: vi.fn(),
		};

		const protocol = new OnchainProtocol({
			publicClient: publicClient as unknown as PublicClient<Transport, Chain>,
			account: account as unknown as ValidatorAccount,
			gasFeeEstimator: gasFeeEstimator as unknown as GasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage: txStorage as unknown as TransactionStorage,
			logger: testLogger,
			blocksBeforeResubmit: overrides?.blocksBeforeResubmit,
		});

		return {
			protocol,
			queue,
			publicClient,
			account,
			gasFeeEstimator,
			txStorage,
		};
	}

	it("should return correct config params", async () => {
		const { protocol } = createTestContext({ chain: { id: 100 } as Chain });
		expect(protocol.chainId()).toBe(100n);
		expect(protocol.consensus()).toBe(TEST_CONSENSUS);
		expect(protocol.coordinator()).toBe(TEST_COORDINATOR);
	});

	it("should not check pending on setup (in constructor)", async () => {
		const {
			txStorage: { setSubmittedForPending, setExecutedUpTo, submittedUpTo },
		} = createTestContext();
		expect(setSubmittedForPending).toBeCalledTimes(0);
		expect(setExecutedUpTo).toBeCalledTimes(0);
		expect(submittedUpTo).toBeCalledTimes(0);
	});

	it("should use bulk mark as executed", async () => {
		const {
			protocol,
			publicClient: { getTransactionCount },
			txStorage: { countPending, submittedUpTo, setExecutedUpTo, setSubmittedForPending },
		} = createTestContext();
		const loggerSpy = vi.spyOn(testLogger, "debug");
		countPending.mockReturnValue(2);
		setSubmittedForPending.mockReturnValue(0);
		setExecutedUpTo.mockReturnValue(2);
		submittedUpTo.mockReturnValue([]);
		getTransactionCount.mockResolvedValueOnce(12);
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(11);
		expect(loggerSpy).toBeCalledTimes(1);
		expect(loggerSpy).toBeCalledWith("Marked 2 transactions as executed");
	});

	it("should do nothing on setSubmittedForPending error", async () => {
		const {
			protocol,
			txStorage: { countPending, setSubmittedForPending },
		} = createTestContext();
		const loggerSpy = vi.spyOn(testLogger, "error");
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockImplementationOnce(() => {
			throw new Error("Test unexpected!");
		});
		protocol.triggerPendingCheck(10n);
		await vi.waitFor(() => expect(loggerSpy).toHaveBeenCalled());
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
	});

	it("should do nothing on rpc error", async () => {
		const {
			protocol,
			publicClient: { getTransactionCount },
			txStorage: { countPending, setSubmittedForPending },
		} = createTestContext();
		const loggerSpy = vi.spyOn(testLogger, "error");
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		getTransactionCount.mockRejectedValueOnce(new Error("Test unexpected!"));
		protocol.triggerPendingCheck(10n);
		await vi.waitFor(() => expect(loggerSpy).toHaveBeenCalled());
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
	});

	it("should do nothing on mark all tx as executed error", async () => {
		const {
			protocol,
			publicClient: { getTransactionCount },
			txStorage: { countPending, setSubmittedForPending, setExecutedUpTo },
		} = createTestContext();
		const loggerSpy = vi.spyOn(testLogger, "error");
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);
		setExecutedUpTo.mockImplementationOnce(() => {
			throw new Error("Test unexpected!");
		});
		protocol.triggerPendingCheck(10n);
		await vi.waitFor(() => expect(loggerSpy).toHaveBeenCalled());
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(1);
	});

	it("should do nothing on fetching submittedUpTo tx error", async () => {
		const {
			protocol,
			publicClient: { getTransactionCount },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo },
		} = createTestContext();
		const loggerSpy = vi.spyOn(testLogger, "error");
		countPending.mockReturnValue(1);
		getTransactionCount.mockResolvedValueOnce(10);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		submittedUpTo.mockImplementationOnce(() => {
			throw new Error("Test unexpected!");
		});
		protocol.triggerPendingCheck(10n);
		await vi.waitFor(() => expect(loggerSpy).toHaveBeenCalled());
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(9);
		expect(submittedUpTo).toBeCalledTimes(1);
	});

	it("should do nothing on fetching gas fees error", async () => {
		const {
			protocol,
			publicClient: { getTransactionCount },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo },
		} = createTestContext();
		countPending.mockReturnValue(1);
		getTransactionCount.mockResolvedValueOnce(10);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		const hash = keccak256("0x5afe5afe01");
		const [, , tx1] = TEST_ACTIONS[0];
		const [, , tx2] = TEST_ACTIONS[0];
		submittedUpTo.mockReturnValue([
			{
				...tx1,
				nonce: 10,
				hash,
			},
			{
				...tx2,
				nonce: 11,
				hash: null,
			},
		]);
		estimateFees.mockRejectedValueOnce("Test unexpected!");
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(9);
		expect(submittedUpTo).toBeCalledTimes(1);
		// Only 1 call is expected, as the rest of the pending txs are skipped on error
		expect(estimateFees).toBeCalledTimes(1);
	});

	it("should mark as completed if nonce too low error on submission", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setFees, setHash },
		} = createTestContext();

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(10);
		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		sendRawTransaction.mockRejectedValueOnce(new NonceTooLowError());
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(2);
		expect(setExecutedUpTo).toHaveBeenNthCalledWith(1, 9);
		expect(setExecutedUpTo).toHaveBeenNthCalledWith(2, 10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		const txHash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		expect(setHash).toBeCalledWith(10, txHash);
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({
			serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
		});
	});

	it("should mark as completed if nested nonce too low error on submission", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setPending, setFees, setHash },
		} = createTestContext();

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(10);
		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		sendRawTransaction.mockRejectedValueOnce(
			new TransactionExecutionError(
				new NonceTooLowError(),
				{} as unknown as Omit<SendTransactionParameters, "account" | "chain"> & {
					account: Account | null;
					chain?: Chain | undefined;
					docsPath?: string | undefined;
				},
			),
		);
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(2);
		expect(setExecutedUpTo).toHaveBeenNthCalledWith(1, 9);
		expect(setExecutedUpTo).toHaveBeenNthCalledWith(2, 10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(setPending).toBeCalledTimes(1);
		expect(setPending).toBeCalledWith(10);
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		const txHash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		expect(setHash).toBeCalledWith(10, txHash);
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({
			serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
		});
	});

	it("should set tx hash on unexpected error on submission", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setHash },
		} = createTestContext();

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(10);
		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		sendRawTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(9);
		expect(estimateFees).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		const txHash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		expect(setHash).toBeCalledWith(10, txHash);
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({
			serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
		});
	});

	it("should resubmit submittedUpTo tx without stored gas fees", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setFees, setHash },
		} = createTestContext();

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
				fees: null,
			},
		]);
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);

		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		const retryHash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		sendRawTransaction.mockResolvedValueOnce(retryHash);
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(9);
		expect(estimateFees).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({
			serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
		});
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(10, retryHash);
	});

	it("should resubmit submittedUpTo tx with lower stored gas fees", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setFees, setHash },
		} = createTestContext();

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
				fees: {
					maxFeePerGas: 100n,
					maxPriorityFeePerGas: 50n,
				},
			},
		]);
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);

		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe02");
		const retryHash = keccak256("0x5afe5afe02");
		sendRawTransaction.mockResolvedValueOnce(retryHash);
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(9);
		expect(estimateFees).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({ serializedTransaction: "0x5afe5afe02" });
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(10, retryHash);
	});

	it("should resubmit submittedUpTo tx with higher stored gas fees", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setFees, setHash },
		} = createTestContext();

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
				fees: {
					maxFeePerGas: 190n,
					maxPriorityFeePerGas: 99n,
				},
			},
		]);
		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValueOnce(0);
		setExecutedUpTo.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);

		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe02");
		const retryHash = keccak256("0x5afe5afe02");
		sendRawTransaction.mockResolvedValueOnce(retryHash);
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(9);
		expect(estimateFees).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 209n,
			maxPriorityFeePerGas: 108n,
		});
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({ serializedTransaction: "0x5afe5afe02" });
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 209n,
			maxPriorityFeePerGas: 108n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(10, retryHash);
	});

	it("should submit submittedUpTo tx without hash", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setFees, setHash },
		} = createTestContext();

		setSubmittedForPending.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);
		setExecutedUpTo.mockReturnValueOnce(0);
		countPending.mockReturnValue(1);
		const [, , tx] = TEST_ACTIONS[0];
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 11,
				hash: null,
			},
		]);

		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		const hash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		sendRawTransaction.mockResolvedValueOnce(hash);
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(9);
		expect(estimateFees).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 11,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({
			serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
		});
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(11, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(11, hash);
	});

	it("should check pending when checkPendingActions is called", async () => {
		const {
			protocol,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo, setPending, setFees, setHash },
		} = createTestContext();

		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValue(0);
		setExecutedUpTo.mockReturnValue(0);
		getTransactionCount.mockResolvedValue(11);
		const hash = keccak256("0x5afe5afe");
		const [, , tx] = TEST_ACTIONS[0];
		submittedUpTo.mockReturnValue([
			{
				...tx,
				nonce: 11,
				hash,
			},
		]);
		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe");
		sendRawTransaction.mockResolvedValueOnce(hash);
		await protocol.checkPendingActions(10n);
		expect(countPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecutedUpTo).toBeCalledTimes(1);
		expect(setExecutedUpTo).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 11,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setPending).toBeCalledTimes(1);
		expect(setPending).toBeCalledWith(11);
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(11, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(11, hash);
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({ serializedTransaction: "0x5afe5afe" });
	});

	it("should delete tx on error if no additional tx was submitted", async () => {
		const {
			protocol,
			queue,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { register, maxNonce, delete: deleteTx, setHash },
		} = createTestContext();

		const [action, , tx] = TEST_ACTIONS[0];
		getTransactionCount.mockResolvedValueOnce(10);
		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		sendRawTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		register.mockReturnValueOnce(10);
		maxNonce.mockReturnValueOnce(10);
		protocol.process(action);
		// Action was submitted and should be in the queue
		expect(queue.peek()).toBeDefined();
		await vi.waitFor(() => {
			expect(deleteTx).toHaveBeenCalled();
		});
		// Tx was not stored, action should be kept
		expect(queue.peek()).toBeDefined();
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(register).toBeCalledTimes(1);
		expect(register).toBeCalledWith(tx, 10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		const txHash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		expect(setHash).toBeCalledWith(10, txHash);
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({
			serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
		});
		expect(deleteTx).toBeCalledTimes(1);
		expect(deleteTx).toBeCalledWith(10);
	});

	it("should not delete tx on error if additional tx was submitted", async () => {
		const {
			protocol,
			queue,
			publicClient: { chain, getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { register, setPending, setFees, maxNonce, delete: deleteTx, setHash },
		} = createTestContext();

		const [action, , tx] = TEST_ACTIONS[0];
		getTransactionCount.mockResolvedValueOnce(10);
		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		signTransaction.mockResolvedValueOnce("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		sendRawTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		register.mockReturnValueOnce(10);
		maxNonce.mockReturnValueOnce(11);
		protocol.process(action);
		// Action was submitted and should be in the queue
		expect(queue.peek()).toBeDefined();
		await vi.waitFor(() => {
			expect(sendRawTransaction).toHaveBeenCalled();
		});
		// Tx was stored, action should be popped
		expect(queue.peek()).toBeUndefined();
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(register).toBeCalledTimes(1);
		expect(register).toBeCalledWith(tx, 10);
		expect(estimateFees).toBeCalledTimes(1);
		// Check that pending state and fees are also set in an error case
		expect(setPending).toBeCalledTimes(1);
		expect(setPending).toBeCalledWith(10);
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(signTransaction).toBeCalledTimes(1);
		expect(signTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			chainId: chain.id,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		const txHash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
		expect(setHash).toBeCalledWith(10, txHash);
		expect(sendRawTransaction).toBeCalledTimes(1);
		expect(sendRawTransaction).toBeCalledWith({
			serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
		});
		expect(deleteTx).toBeCalledTimes(0);
	});

	describe.each(
		TEST_ACTIONS.map(([action, functionName, tx]) => {
			return {
				description: action.id,
				functionName,
				tx,
				action,
			};
		}),
	)("for $description", ({ action, functionName, tx }) => {
		it(`should call ${functionName}`, async () => {
			const {
				protocol,
				publicClient: { chain, getTransactionCount, sendRawTransaction },
				account: { signTransaction },
				gasFeeEstimator: { estimateFees },
				txStorage: { register, setPending, setFees, setHash },
			} = createTestContext();

			getTransactionCount.mockResolvedValueOnce(2);
			// Mock high nonce to ensure overwrite works
			register.mockReturnValueOnce(10);
			estimateFees.mockResolvedValueOnce({
				maxFeePerGas: 200n,
				maxPriorityFeePerGas: 100n,
			});
			signTransaction.mockResolvedValueOnce(
				"0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
			);
			const txHash = keccak256("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");
			sendRawTransaction.mockResolvedValueOnce(txHash);

			protocol.process(action, 0);
			// Wait for the setHash that is triggered after successful submission
			await vi.waitFor(() => {
				expect(sendRawTransaction).toHaveBeenCalled();
			});
			expect(getTransactionCount).toBeCalledTimes(1);
			expect(getTransactionCount).toBeCalledWith({
				address: entryPoint09Address,
				blockTag: "latest",
			});
			expect(register).toBeCalledTimes(1);
			expect(register).toBeCalledWith(tx, 2);
			expect(estimateFees).toBeCalledTimes(1);
			expect(setPending).toBeCalledTimes(1);
			expect(setPending).toBeCalledWith(10);
			expect(setFees).toBeCalledTimes(1);
			expect(setFees).toBeCalledWith(10, {
				maxFeePerGas: 200n,
				maxPriorityFeePerGas: 100n,
			});
			expect(signTransaction).toBeCalledTimes(1);
			expect(signTransaction).toBeCalledWith({
				...tx,
				nonce: 10,
				chainId: chain.id,
				maxFeePerGas: 200n,
				maxPriorityFeePerGas: 100n,
			});
			expect(sendRawTransaction).toBeCalledTimes(1);
			expect(sendRawTransaction).toBeCalledWith({
				serializedTransaction: "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe",
			});
			expect(setHash).toBeCalledTimes(1);
			expect(setHash).toBeCalledWith(10, txHash);
		});
	});

	it("should not run concurrent pending checks", async () => {
		const {
			protocol,
			publicClient: { getTransactionCount, sendRawTransaction },
			account: { signTransaction },
			gasFeeEstimator: { estimateFees },
			txStorage: { countPending, submittedUpTo, setSubmittedForPending, setExecutedUpTo },
		} = createTestContext();

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		const pendingTx = { ...tx, nonce: 10, hash };

		countPending.mockReturnValue(1);
		setSubmittedForPending.mockReturnValue(0);
		setExecutedUpTo.mockReturnValue(0);
		submittedUpTo.mockReturnValue([pendingTx]);
		getTransactionCount.mockResolvedValue(10);
		estimateFees.mockResolvedValue({ maxFeePerGas: 200n, maxPriorityFeePerGas: 100n });
		signTransaction.mockResolvedValue("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe");

		// sendRawTransaction will be held pending until we resolve it manually
		let resolveSendRaw!: (value: `0x${string}`) => void;
		const sendRawPending = new Promise<`0x${string}`>((resolve) => {
			resolveSendRaw = resolve;
		});
		sendRawTransaction.mockReturnValueOnce(sendRawPending);

		// Start first check — it will hang inside sendRawTransaction
		protocol.triggerPendingCheck(10n);

		// Wait until sendRawTransaction has been called (first check has entered in-flight state)
		await vi.waitFor(() => expect(sendRawTransaction).toHaveBeenCalledTimes(1));

		// Attempt second check while first is still in-flight — should be skipped (lock held)
		protocol.triggerPendingCheck(11n);

		// Resolve the first sendRawTransaction and wait for the lock to be released
		resolveSendRaw("0xdeadbeef");
		await vi.waitFor(() => expect(protocol.isRunningPendingCheck()).toBe(false));

		// sendRawTransaction must have been called exactly once despite two concurrent checks
		expect(sendRawTransaction).toBeCalledTimes(1);

		// After the in-flight work is done, a subsequent check can resubmit the tx (guard is not sticky)
		sendRawTransaction.mockReturnValueOnce(Promise.resolve("0xdeadbeef2"));
		protocol.triggerPendingCheck(12n);
		await vi.waitFor(() => expect(sendRawTransaction).toBeCalledTimes(2));
	});
});

describe("GasFeeEstimator", () => {
	it("should cache prices", async () => {
		const estimateFeesPerGas = vi.fn();
		const publicClient = {
			estimateFeesPerGas,
		} as unknown as PublicClient;
		let priceCallback: ((values: FeeValuesEIP1559) => void) | undefined;
		const promise = new Promise<FeeValuesEIP1559>((callback) => {
			priceCallback = callback;
		});
		estimateFeesPerGas.mockReturnValueOnce(promise);
		const estimator = new GasFeeEstimator(publicClient);
		const price1 = estimator.estimateFees();
		const price2 = estimator.estimateFees();
		expect(estimateFeesPerGas).toHaveBeenCalledTimes(1);
		priceCallback?.({
			maxFeePerGas: 100n,
			maxPriorityFeePerGas: 200n,
		});
		expect(price1).toBe(price2);
		expect(await price1).toStrictEqual({
			maxFeePerGas: 100n,
			maxPriorityFeePerGas: 200n,
		});
		expect(await price2).toStrictEqual({
			maxFeePerGas: 100n,
			maxPriorityFeePerGas: 200n,
		});
	});

	it("should cache errors", async () => {
		const estimateFeesPerGas = vi.fn();
		const publicClient = {
			estimateFeesPerGas,
		} as unknown as PublicClient;
		let rejectedCallback: ((reason: unknown) => void) | undefined;
		const promise = new Promise<FeeValuesEIP1559>((_, reject) => {
			rejectedCallback = reject;
		});
		estimateFeesPerGas.mockReturnValueOnce(promise);
		const estimator = new GasFeeEstimator(publicClient);
		const price1 = estimator.estimateFees();
		const price2 = estimator.estimateFees();
		expect(estimateFeesPerGas).toHaveBeenCalledTimes(1);
		rejectedCallback?.("Some error");
		expect(price1).toBe(price2);
		await expect(price1).rejects.toThrow("Some error");
		await expect(price2).rejects.toThrow("Some error");
	});

	it("should invalidate cache", async () => {
		const estimateFeesPerGas = vi.fn();
		const publicClient = {
			estimateFeesPerGas,
		} as unknown as PublicClient;
		estimateFeesPerGas.mockReturnValueOnce(new Promise(() => {}));
		const estimator = new GasFeeEstimator(publicClient);
		const original = estimator.estimateFees();
		expect(original).toBe(estimator.estimateFees());
		expect(estimateFeesPerGas).toHaveBeenCalledTimes(1);
		estimator.invalidate();
		const next = estimator.estimateFees();
		expect(original).not.toBe(next);
		expect(estimateFeesPerGas).toHaveBeenCalledTimes(2);
	});
});
