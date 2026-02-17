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
	type WalletClient,
} from "viem";
import { entryPoint09Address } from "viem/account-abstraction";
import { gnosisChiado } from "viem/chains";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { testLogger } from "../../__tests__/config.js";
import { TEST_ACTIONS, TEST_CONSENSUS, TEST_COORDINATOR } from "../../__tests__/data/protocol.js";
import { InMemoryQueue } from "../../utils/queue.js";
import { GasFeeEstimator, OnchainProtocol, type TransactionStorage } from "./onchain.js";
import type { ActionWithTimeout } from "./types.js";

describe("OnchainProtocol", () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("should return correct config params", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const publicClient = {} as unknown as PublicClient;
		const signingClient = {
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const gasFeeEstimator = {} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const txStorage = {
			setAllBeforeAsExecuted,
			submittedUpTo,
		} as unknown as TransactionStorage;
		setAllBeforeAsExecuted.mockReturnValue(0);
		submittedUpTo.mockReturnValue([]);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		expect(protocol.chainId()).toBe(100n);
		expect(protocol.consensus()).toBe(TEST_CONSENSUS);
		expect(protocol.coordinator()).toBe(TEST_COORDINATOR);
	});

	it("should not check pending on setup (in constructor)", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {} as unknown as PublicClient;
		const signingClient = {} as unknown as WalletClient<Transport, Chain, Account>;
		const gasFeeEstimator = {} as unknown as GasFeeEstimator;
		const setExecuted = vi.fn();
		const submittedUpTo = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			setSubmittedForPending,
			setAllBeforeAsExecuted,
			submittedUpTo,
			setExecuted,
		} as unknown as TransactionStorage;
		setSubmittedForPending.mockReturnValue(0);
		setAllBeforeAsExecuted.mockReturnValue(2);
		submittedUpTo.mockReturnValue([]);
		getTransactionCount.mockResolvedValueOnce(12);
		new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		expect(setSubmittedForPending).toBeCalledTimes(0);
		expect(setAllBeforeAsExecuted).toBeCalledTimes(0);
		expect(submittedUpTo).toBeCalledTimes(0);
		expect(setExecuted).toBeCalledTimes(0);
	});

	it("should use bulk mark as executed", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const gasFeeEstimator = {} as unknown as GasFeeEstimator;
		const setExecuted = vi.fn();
		const submittedUpTo = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			setSubmittedForPending,
			setAllBeforeAsExecuted,
			submittedUpTo,
			setExecuted,
		} as unknown as TransactionStorage;
		const loggerSpy = vi.spyOn(testLogger, "debug");
		setSubmittedForPending.mockReturnValue(0);
		setAllBeforeAsExecuted.mockReturnValue(2);
		submittedUpTo.mockReturnValue([]);
		getTransactionCount.mockResolvedValueOnce(12);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(12);
		expect(loggerSpy).toBeCalledTimes(1);
		expect(loggerSpy).toBeCalledWith("Marked 2 transactions as executed");
		expect(setExecuted).toBeCalledTimes(0);
	});

	it("should do nothing on setSubmittedForPending error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const publicClient = {} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const gasFeeEstimator = {} as unknown as GasFeeEstimator;
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			setSubmittedForPending,
		} as unknown as TransactionStorage;
		setSubmittedForPending.mockRejectedValueOnce(new Error("Test unexpected!"));
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
	});

	it("should do nothing on rpc error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const gasFeeEstimator = {} as unknown as GasFeeEstimator;
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			setSubmittedForPending,
		} as unknown as TransactionStorage;
		setSubmittedForPending.mockResolvedValueOnce(10);
		getTransactionCount.mockRejectedValueOnce(new Error("Test unexpected!"));
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
	});

	it("should do nothing on mark all tx as executed error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const gasFeeEstimator = {} as unknown as GasFeeEstimator;
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const txStorage = {
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;
		setSubmittedForPending.mockResolvedValueOnce(10);
		getTransactionCount.mockResolvedValueOnce(10);
		setAllBeforeAsExecuted.mockImplementationOnce(() => {
			throw new Error("Test unexpected!");
		});
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
	});

	it("should do nothing on fetching submittedUpTo tx error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const gasFeeEstimator = {} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const txStorage = {
			submittedUpTo,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;
		getTransactionCount.mockResolvedValueOnce(10);
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
		submittedUpTo.mockImplementationOnce(() => {
			throw new Error("Test unexpected!");
		});
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(submittedUpTo).toBeCalledTimes(1);
	});

	it("should do nothing on fetching gas fees error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const txStorage = {
			submittedUpTo,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;
		getTransactionCount.mockResolvedValueOnce(10);
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
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
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(submittedUpTo).toBeCalledTimes(1);
		expect(estimateFees).toBeCalledTimes(2);
	});

	it("should mark as completed if nonce too low error on submission", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setFees = vi.fn();
		const setHash = vi.fn();
		const setExecuted = vi.fn();
		const txStorage = {
			submittedUpTo,
			setFees,
			setHash,
			setExecuted,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
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
		sendTransaction.mockRejectedValueOnce(new NonceTooLowError());
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(0);
	});

	it("should mark as completed if nested nonce too low error on submission", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setFees = vi.fn();
		const setHash = vi.fn();
		const setExecuted = vi.fn();
		const txStorage = {
			submittedUpTo,
			setFees,
			setHash,
			setExecuted,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
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
		sendTransaction.mockRejectedValueOnce(
			new TransactionExecutionError(
				new NonceTooLowError(),
				{} as unknown as Omit<SendTransactionParameters, "account" | "chain"> & {
					account: Account | null;
					chain?: Chain | undefined;
					docsPath?: string | undefined;
				},
			),
		);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(0);
	});

	it("should do nothing on unexpected error on submission", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setFees = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			submittedUpTo,
			setFees,
			setHash,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
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
		sendTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(0);
	});

	it("should resubmit submittedUpTo tx without stored gas fees", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const setFees = vi.fn();
		const txStorage = {
			submittedUpTo,
			setHash,
			setFees,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;

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
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);

		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		const retryHash = keccak256("0x5afe5afe02");
		sendTransaction.mockResolvedValueOnce(retryHash);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
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
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const setFees = vi.fn();
		const txStorage = {
			submittedUpTo,
			setHash,
			setFees,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;

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
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);

		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		const retryHash = keccak256("0x5afe5afe02");
		sendTransaction.mockResolvedValueOnce(retryHash);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(10, retryHash);
	});

	it("should resubmit submittedUpTo tx with higher stored gas fees", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const setFees = vi.fn();
		const txStorage = {
			submittedUpTo,
			setHash,
			setFees,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;

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
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);

		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		const retryHash = keccak256("0x5afe5afe02");
		sendTransaction.mockResolvedValueOnce(retryHash);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			account,
			chain,
			maxFeePerGas: 209n,
			maxPriorityFeePerGas: 108n,
		});
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(10, {
			maxFeePerGas: 209n,
			maxPriorityFeePerGas: 108n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(10, retryHash);
	});

	it("should submit submittedUpTo tx without hash", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setFees = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			submittedUpTo,
			setFees,
			setHash,
			setSubmittedForPending,
			setAllBeforeAsExecuted,
		} as unknown as TransactionStorage;

		setSubmittedForPending.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
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
		const hash = keccak256("0x5afe5afe");
		sendTransaction.mockResolvedValueOnce(hash);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(10);
		expect(estimateFees).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 11,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
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
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const setHash = vi.fn();
		const setFees = vi.fn();
		const submittedUpTo = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			submittedUpTo,
			setFees,
			setHash,
			setAllBeforeAsExecuted,
			setSubmittedForPending,
		} as unknown as TransactionStorage;

		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});

		setSubmittedForPending.mockReturnValue(0);
		setAllBeforeAsExecuted.mockReturnValue(0);
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
		sendTransaction.mockResolvedValueOnce(hash);
		await protocol.checkPendingActions(10n);
		expect(setSubmittedForPending).toBeCalledTimes(1);
		expect(setSubmittedForPending).toBeCalledWith(10n);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setAllBeforeAsExecuted).toBeCalledTimes(1);
		expect(setAllBeforeAsExecuted).toBeCalledWith(11);
		expect(estimateFees).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 11,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setFees).toBeCalledTimes(1);
		expect(setFees).toBeCalledWith(11, {
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(11, hash);
	});

	it("should not delete tx on error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const sendTransaction = vi.fn();
		const chain = gnosisChiado;
		const account = { address: entryPoint09Address };
		const signingClient = {
			account,
			chain,
			sendTransaction,
		} as unknown as WalletClient<Transport, Chain, Account>;
		const estimateFees = vi.fn();
		const gasFeeEstimator = {
			estimateFees,
		} as unknown as GasFeeEstimator;
		const register = vi.fn();
		const setFees = vi.fn();
		const maxNonce = vi.fn();
		const deleteTx = vi.fn();
		const txStorage = {
			register,
			setFees,
			maxNonce,
			delete: deleteTx,
		} as unknown as TransactionStorage;

		const [action, , tx] = TEST_ACTIONS[0];
		getTransactionCount.mockResolvedValueOnce(10);
		estimateFees.mockResolvedValueOnce({
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
		});
		sendTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		register.mockReturnValueOnce(10);
		maxNonce.mockReturnValueOnce(10);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			gasFeeEstimator,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		protocol.process(action);
		// Action was submitted and should be in the queue
		expect(queue.peek()).toBeDefined();
		await vi.waitFor(() => {
			expect(sendTransaction).toHaveBeenCalled();
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
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			account,
			chain,
			maxFeePerGas: 200n,
			maxPriorityFeePerGas: 100n,
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
			const queue = new InMemoryQueue<ActionWithTimeout>();
			const getTransactionCount = vi.fn();
			const publicClient = {
				getTransactionCount,
			} as unknown as PublicClient;
			const sendTransaction = vi.fn();
			const chain = gnosisChiado;
			const account = { address: entryPoint09Address };
			const signingClient = {
				account,
				chain,
				sendTransaction,
			} as unknown as WalletClient<Transport, Chain, Account>;
			const estimateFees = vi.fn();
			const gasFeeEstimator = {
				estimateFees,
			} as unknown as GasFeeEstimator;
			const register = vi.fn();
			const setHash = vi.fn();
			const setFees = vi.fn();
			const txStorage = {
				register,
				setFees,
				setHash,
			} as unknown as TransactionStorage;
			const protocol = new OnchainProtocol({
				publicClient,
				signingClient,
				gasFeeEstimator,
				consensus: TEST_CONSENSUS,
				coordinator: TEST_COORDINATOR,
				queue,
				txStorage,
				logger: testLogger,
			});

			getTransactionCount.mockResolvedValueOnce(2);
			// Mock high nonce to ensure overwrite works
			register.mockReturnValueOnce(10);
			estimateFees.mockResolvedValueOnce({
				maxFeePerGas: 200n,
				maxPriorityFeePerGas: 100n,
			});
			const txHash = keccak256("0x5afe5afe");
			sendTransaction.mockResolvedValueOnce(txHash);

			protocol.process(action, 0);
			// Wait for the setHash that is triggered after successful submission
			await vi.waitFor(() => {
				expect(setHash).toHaveBeenCalled();
			});
			expect(getTransactionCount).toBeCalledTimes(1);
			expect(getTransactionCount).toBeCalledWith({
				address: entryPoint09Address,
				blockTag: "latest",
			});
			expect(register).toBeCalledTimes(1);
			expect(register).toBeCalledWith(tx, 2);
			expect(estimateFees).toBeCalledTimes(1);
			expect(setFees).toBeCalledTimes(1);
			expect(setFees).toBeCalledWith(10, {
				maxFeePerGas: 200n,
				maxPriorityFeePerGas: 100n,
			});
			expect(sendTransaction).toBeCalledTimes(1);
			expect(sendTransaction).toBeCalledWith({
				...tx,
				nonce: 10,
				account,
				chain,
				maxFeePerGas: 200n,
				maxPriorityFeePerGas: 100n,
			});
			expect(setHash).toBeCalledTimes(1);
			expect(setHash).toBeCalledWith(10, txHash);
		});
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
