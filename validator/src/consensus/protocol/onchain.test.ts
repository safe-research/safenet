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
		const pending = vi.fn();
		const txStorage = {
			pending,
		} as unknown as TransactionStorage;
		pending.mockReturnValue([]);
		const protocol = new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		expect(protocol.chainId()).toBe(100n);
		expect(protocol.consensus()).toBe(TEST_CONSENSUS);
		expect(protocol.coordinator()).toBe(TEST_COORDINATOR);
	});

	it("should check pending on setup, mark as executed and setTimeout if polling is enabled", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const setExecuted = vi.fn();
		const pending = vi.fn();
		const txStorage = {
			pending,
			setExecuted,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const hash = keccak256("0x5afe5afe");
		const [, , tx1] = TEST_ACTIONS[0];
		const [, , tx2] = TEST_ACTIONS[1];
		const [, , tx3] = TEST_ACTIONS[2];
		pending.mockReturnValue([
			{
				...tx1,
				nonce: 10,
				hash,
			},
			{
				...tx2,
				nonce: 11,
				hash,
			},
			{
				...tx3,
				nonce: 12,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(12);
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(setExecuted).toBeCalledTimes(2);
		expect(setExecuted).toHaveBeenNthCalledWith(1, 10);
		expect(setExecuted).toHaveBeenNthCalledWith(2, 11);
	});

	it("should check pending on setup and mark as executed but not setTimeout when polling is not enabled", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const setExecuted = vi.fn();
		const pending = vi.fn();
		const txStorage = {
			pending,
			setExecuted,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const hash = keccak256("0x5afe5afe");
		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(11);
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
		});
		await vi.waitFor(() => {
			expect(getTransactionCount).toHaveBeenCalled();
		});
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(timeoutSpy).toBeCalledTimes(0);
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
		const pending = vi.fn();
		const txStorage = {
			pending,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const hash = keccak256("0x5afe5afe");
		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockRejectedValueOnce(new Error("Test unexpected!"));
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
	});

	it("should do nothing on fetching pending tx error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const pending = vi.fn();
		const txStorage = {
			pending,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");
		getTransactionCount.mockResolvedValueOnce(10);
		pending.mockImplementationOnce(() => {
			throw new Error("Test unexpected!");
		});
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(pending).toBeCalledTimes(1);
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
		const pending = vi.fn();
		const setHash = vi.fn();
		const setExecuted = vi.fn();
		const txStorage = {
			pending,
			setHash,
			setExecuted,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(10);
		sendTransaction.mockRejectedValueOnce(new NonceTooLowError());
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
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
		const pending = vi.fn();
		const setHash = vi.fn();
		const setExecuted = vi.fn();
		const txStorage = {
			pending,
			setHash,
			setExecuted,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
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
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
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
		const pending = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			pending,
			setHash,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(10);
		sendTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
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

	it("should resubmit pending tx", async () => {
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
		const pending = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			pending,
			setHash,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const hash = keccak256("0x5afe5afe01");
		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(10);

		const retryHash = keccak256("0x5afe5afe02");
		sendTransaction.mockResolvedValueOnce(retryHash);
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
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

	it("should submit pending tx without hash", async () => {
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
		const pending = vi.fn();
		const setHash = vi.fn();
		const txStorage = {
			pending,
			setHash,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");

		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash: null,
			},
		]);

		const hash = keccak256("0x5afe5afe");
		sendTransaction.mockResolvedValueOnce(hash);
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
		expect(sendTransaction).toBeCalledTimes(1);
		expect(sendTransaction).toBeCalledWith({
			...tx,
			nonce: 10,
			hash: null,
			account,
			chain,
		});
		expect(setHash).toBeCalledTimes(1);
		expect(setHash).toBeCalledWith(10, hash);
	});

	it("should check pending to be called after polling timeout mark as executed", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionCount = vi.fn();
		const publicClient = {
			getTransactionCount,
		} as unknown as PublicClient;
		const signingClient = {
			account: { address: entryPoint09Address },
			chain: { id: 100 },
		} as unknown as WalletClient<Transport, Chain, Account>;
		const setExecuted = vi.fn();
		const pending = vi.fn();
		const txStorage = {
			pending,
			setExecuted,
		} as unknown as TransactionStorage;
		// No pending tx on startup
		pending.mockReturnValue([]);
		getTransactionCount.mockResolvedValueOnce(11);
		new OnchainProtocol({
			publicClient,
			signingClient,
			consensus: TEST_CONSENSUS,
			coordinator: TEST_COORDINATOR,
			queue,
			txStorage,
			logger: testLogger,
			txStatusPollingSeconds: 5,
		});
		await vi.waitFor(() => {
			expect(getTransactionCount).toHaveBeenCalled();
		});
		expect(getTransactionCount).toBeCalledTimes(1);
		expect(pending).toBeCalledTimes(1);

		// No pending tx to check
		const hash = keccak256("0x5afe5afe");
		const [, , tx] = TEST_ACTIONS[0];
		pending.mockReturnValue([
			{
				...tx,
				nonce: 10,
				hash,
			},
		]);
		getTransactionCount.mockResolvedValueOnce(11);
		vi.advanceTimersByTime(5000);
		await vi.waitFor(() => {
			expect(setExecuted).toHaveBeenCalled();
		});
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(getTransactionCount).toBeCalledTimes(2);
		expect(getTransactionCount).toBeCalledWith({
			address: entryPoint09Address,
			blockTag: "latest",
		});
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
			const pending = vi.fn();
			pending.mockReturnValue([]);
			const txStorage = {
				pending,
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
				txStatusPollingSeconds: 5,
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
			expect(getTransactionCount).toBeCalledTimes(2);
			expect(getTransactionCount).toHaveBeenNthCalledWith(1, {
				address: entryPoint09Address,
				blockTag: "latest",
			});
			expect(getTransactionCount).toHaveBeenNthCalledWith(2, {
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
