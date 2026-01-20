import {
	type Account,
	type Chain,
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
import { OnchainProtocol, type TransactionStorage } from "./onchain.js";
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
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			setSubmittedForPending,
		} as unknown as TransactionStorage;
		setSubmittedForPending.mockRejectedValueOnce(new Error("Test unexpected!"));
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
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
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			setSubmittedForPending,
		} as unknown as TransactionStorage;
		setSubmittedForPending.mockResolvedValueOnce(10);
		getTransactionCount.mockRejectedValueOnce(new Error("Test unexpected!"));
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
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
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		protocol.checkPendingActions(10n);
		await vi.waitFor(() => {
			expect(getTransactionCount).toHaveBeenCalled();
		});
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
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const setExecuted = vi.fn();
		const txStorage = {
			submittedUpTo,
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
		sendTransaction.mockRejectedValueOnce(new NonceTooLowError());
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
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
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			hash,
			account,
			chain,
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
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const setExecuted = vi.fn();
		const txStorage = {
			submittedUpTo,
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
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			hash,
			account,
			chain,
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
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			submittedUpTo,
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
		sendTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
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
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			hash,
			account,
			chain,
		});
		expect(setHash).toBeCalledTimes(0);
	});

	it("should resubmit submittedUpTo tx", async () => {
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
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			submittedUpTo,
			setHash,
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
			},
		]);
		setSubmittedForPending.mockReturnValueOnce(0);
		setAllBeforeAsExecuted.mockReturnValueOnce(0);
		getTransactionCount.mockResolvedValueOnce(10);

		const retryHash = keccak256("0x5afe5afe02");
		sendTransaction.mockResolvedValueOnce(retryHash);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
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
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			hash,
			account,
			chain,
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
		const submittedUpTo = vi.fn();
		const setSubmittedForPending = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			submittedUpTo,
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

		const hash = keccak256("0x5afe5afe");
		sendTransaction.mockResolvedValueOnce(hash);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
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
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 11,
			hash: null,
			account,
			chain,
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
		const setHash = vi.fn();
		const submittedUpTo = vi.fn();
		const setAllBeforeAsExecuted = vi.fn();
		const setSubmittedForPending = vi.fn();
		const txStorage = {
			submittedUpTo,
			setHash,
			setAllBeforeAsExecuted,
			setSubmittedForPending,
		} as unknown as TransactionStorage;

		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
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
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 11,
			hash,
			account,
			chain,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(11, hash);
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
			const register = vi.fn();
			const setHash = vi.fn();
			const txStorage = {
				register,
				setHash,
			} as unknown as TransactionStorage;
			const protocol = new OnchainProtocol({
				publicClient,
				signingClient,
				consensus: TEST_CONSENSUS,
				coordinator: TEST_COORDINATOR,
				queue,
				txStorage,
				logger: testLogger,
			});

			getTransactionCount.mockResolvedValueOnce(2);
			// Mock high nonce to ensure overwrite works
			register.mockReturnValueOnce(10);
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
				blockTag: "pending",
			});
			expect(register).toBeCalledTimes(1);
			expect(register).toBeCalledWith(tx, 2);
			expect(sendTransaction).toBeCalledTimes(1);
			expect(sendTransaction).toBeCalledWith({
				...tx,
				nonce: 10,
				account,
				chain,
			});
			expect(setHash).toBeCalledTimes(1);
			expect(setHash).toBeCalledWith(10, txHash);
		});
	});
});
