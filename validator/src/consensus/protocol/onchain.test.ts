import {
	type Account,
	type Chain,
	keccak256,
	type PublicClient,
	TransactionReceiptNotFoundError,
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
		const protocol = new OnchainProtocol(
			publicClient,
			signingClient,
			TEST_CONSENSUS,
			TEST_COORDINATOR,
			queue,
			txStorage,
			testLogger,
		);
		expect(protocol.chainId()).toBe(100n);
		expect(protocol.consensus()).toBe(TEST_CONSENSUS);
		expect(protocol.coordinator()).toBe(TEST_COORDINATOR);
	});

	it("should check pending on setup and mark as executed", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionReceipt = vi.fn();
		const publicClient = {
			getTransactionReceipt,
		} as unknown as PublicClient;
		const signingClient = {} as unknown as WalletClient<Transport, Chain, Account>;
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
		getTransactionReceipt.mockResolvedValueOnce({});
		new OnchainProtocol(publicClient, signingClient, TEST_CONSENSUS, TEST_COORDINATOR, queue, txStorage, testLogger);
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(getTransactionReceipt).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledWith({ hash });
	});

	it("should do nothing on rpc error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionReceipt = vi.fn();
		const publicClient = {
			getTransactionReceipt,
		} as unknown as PublicClient;
		const signingClient = {} as unknown as WalletClient<Transport, Chain, Account>;
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
		getTransactionReceipt.mockRejectedValueOnce(new Error("Test unexpected!"));
		new OnchainProtocol(publicClient, signingClient, TEST_CONSENSUS, TEST_COORDINATOR, queue, txStorage, testLogger);
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledWith({ hash });
	});

	it("should do nothing on fetching pending tx error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionReceipt = vi.fn();
		const publicClient = {
			getTransactionReceipt,
		} as unknown as PublicClient;
		const signingClient = {} as unknown as WalletClient<Transport, Chain, Account>;
		const pending = vi.fn();
		const txStorage = {
			pending,
		} as unknown as TransactionStorage;
		const timeoutSpy = vi.spyOn(global, "setTimeout");
		pending.mockImplementationOnce(() => {
			throw new Error("Test unexpected!");
		});
		new OnchainProtocol(publicClient, signingClient, TEST_CONSENSUS, TEST_COORDINATOR, queue, txStorage, testLogger);
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(pending).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledTimes(0);
	});

	it("should do nothing on submission error", async () => {
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const getTransactionReceipt = vi.fn();
		const publicClient = {
			getTransactionReceipt,
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
		getTransactionReceipt.mockRejectedValueOnce(new TransactionReceiptNotFoundError({ hash }));
		sendTransaction.mockRejectedValueOnce(new Error("Test unexpected!"));
		new OnchainProtocol(publicClient, signingClient, TEST_CONSENSUS, TEST_COORDINATOR, queue, txStorage, testLogger);
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledWith({ hash });
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
		const getTransactionReceipt = vi.fn();
		const publicClient = {
			getTransactionReceipt,
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
		getTransactionReceipt.mockRejectedValueOnce(new TransactionReceiptNotFoundError({ hash }));

		const retryHash = keccak256("0x5afe5afe02");
		sendTransaction.mockResolvedValueOnce(retryHash);
		new OnchainProtocol(publicClient, signingClient, TEST_CONSENSUS, TEST_COORDINATOR, queue, txStorage, testLogger);
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledWith({ hash });
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
		const publicClient = {} as unknown as PublicClient;
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
		new OnchainProtocol(publicClient, signingClient, TEST_CONSENSUS, TEST_COORDINATOR, queue, txStorage, testLogger);
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(timeoutSpy).toBeCalledTimes(1);
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
		const getTransactionReceipt = vi.fn();
		const publicClient = {
			getTransactionReceipt,
		} as unknown as PublicClient;
		const signingClient = {} as unknown as WalletClient<Transport, Chain, Account>;
		const setExecuted = vi.fn();
		const pending = vi.fn();
		const txStorage = {
			pending,
			setExecuted,
		} as unknown as TransactionStorage;
		// No pending tx on startup
		pending.mockReturnValue([]);
		new OnchainProtocol(publicClient, signingClient, TEST_CONSENSUS, TEST_COORDINATOR, queue, txStorage, testLogger);
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
		getTransactionReceipt.mockResolvedValueOnce({});
		vi.advanceTimersByTime(5000);
		await vi.waitFor(() => {
			expect(setExecuted).toHaveBeenCalled();
		});
		expect(setExecuted).toBeCalledTimes(1);
		expect(setExecuted).toBeCalledWith(10);
		expect(getTransactionReceipt).toBeCalledTimes(1);
		expect(getTransactionReceipt).toBeCalledWith({ hash });
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
			const protocol = new OnchainProtocol(
				publicClient,
				signingClient,
				TEST_CONSENSUS,
				TEST_COORDINATOR,
				queue,
				txStorage,
				testLogger,
			);
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
