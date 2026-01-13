import { zeroHash } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { testLogger } from "../../__tests__/config.js";
import { TEST_ACTIONS, TestProtocol } from "../../__tests__/data/protocol.js";
import { InMemoryQueue } from "../../utils/queue.js";
import type { ActionWithTimeout } from "./types.js";

describe("BaseProtocol", () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("should retry actions that errored", async () => {
		const timeoutSpy = vi.spyOn(global, "setTimeout");
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const pushSpy = vi.spyOn(queue, "push");
		const peekSpy = vi.spyOn(queue, "peek");
		const popSpy = vi.spyOn(queue, "pop");
		const protocol = new TestProtocol(queue, testLogger);
		const protocolSpy = vi.spyOn(protocol, "requestSignature");
		expect(queue.peek()).toBeUndefined();
		const action = TEST_ACTIONS[0][0];
		protocol.process(action, 10000);
		const actionWithTimeout = {
			...action,
			validUntil: Date.now() + 10000,
		};
		expect(queue.peek()).toStrictEqual(actionWithTimeout);
		expect(pushSpy).toBeCalledTimes(1);
		expect(pushSpy).toBeCalledWith(actionWithTimeout);
		// Called 3 times: 2 times in the test, 1 time in the implementation
		expect(peekSpy).toBeCalledTimes(3);
		// Check if retry was scheduled via setTimeout
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		expect(protocolSpy).toBeCalledTimes(1);
		// Test successful retry
		protocolSpy.mockResolvedValueOnce(zeroHash);
		vi.advanceTimersByTime(1000);
		// Wait for retry to be successful
		await vi.waitFor(() => {
			expect(popSpy).toHaveBeenCalled();
		});
		expect(popSpy).toBeCalledTimes(1);
		// Called 3 times before, 1 additional time by retry, 1 time after retry
		expect(peekSpy).toBeCalledTimes(5);
		expect(protocolSpy).toBeCalledTimes(2);
		expect(protocolSpy).toBeCalledWith(actionWithTimeout);
		// Queue should be empty now
		expect(queue.peek()).toBeUndefined();
	});

	it("should drop actions after timeout", async () => {
		const timeoutSpy = vi.spyOn(global, "setTimeout");
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const pushSpy = vi.spyOn(queue, "push");
		const peekSpy = vi.spyOn(queue, "peek");
		const popSpy = vi.spyOn(queue, "pop");
		const protocol = new TestProtocol(queue, testLogger);
		const protocolSpy = vi.spyOn(protocol, "requestSignature");
		expect(queue.peek()).toBeUndefined();
		const action = TEST_ACTIONS[0][0];
		protocol.process(action, 0);
		const actionWithTimeout = {
			...action,
			validUntil: Date.now(),
		};
		expect(queue.peek()).toStrictEqual(actionWithTimeout);
		expect(pushSpy).toBeCalledTimes(1);
		expect(pushSpy).toBeCalledWith(actionWithTimeout);
		// Called 3 times: 2 times in the test, 1 time in the implementation
		expect(peekSpy).toBeCalledTimes(3);
		// Check if retry was scheduled via setTimeout
		await vi.waitFor(() => {
			expect(timeoutSpy).toHaveBeenCalled();
		});
		// Initial try to executed the action
		expect(protocolSpy).toBeCalledTimes(1);

		// Test action timeout
		vi.advanceTimersByTime(1000);
		// Wait for retry to be successful
		await vi.waitFor(() => {
			expect(popSpy).toHaveBeenCalled();
		});
		expect(popSpy).toBeCalledTimes(1);
		// No additional function should be triggered as the action was dropped
		expect(protocolSpy).toBeCalledTimes(1);
		// Called 3 times before, 1 additional time by retry, 1 time after retry
		expect(peekSpy).toBeCalledTimes(5);
		// Queue should be empty now
		expect(queue.peek()).toBeUndefined();
	});

	it("should be able to enqueue multiple actions", async () => {
		const timeoutSpy = vi.spyOn(global, "setTimeout");
		const queue = new InMemoryQueue<ActionWithTimeout>();
		const popSpy = vi.spyOn(queue, "pop");
		const protocol = new TestProtocol(queue, testLogger);
		const protocolSpy = vi.spyOn(protocol, "requestSignature");
		// Do not resolve promise
		protocolSpy.mockReturnValueOnce(new Promise(() => {}));
		const validUntil = Date.now();
		for (const [action] of TEST_ACTIONS) {
			protocol.process(action, 0);
		}
		// No action should be completed
		expect(popSpy).toBeCalledTimes(0);
		expect(timeoutSpy).toBeCalledTimes(0);
		for (const [action] of TEST_ACTIONS) {
			const actionWithTimeout = {
				...action,
				validUntil,
			};
			expect(queue.pop()).toStrictEqual(actionWithTimeout);
		}
	});

	describe.each(
		TEST_ACTIONS.map(([action, functionName]) => {
			return {
				description: action.id,
				functionName,
				action,
			};
		}),
	)("for $description", ({ action, functionName }) => {
		it(`should call ${functionName}`, async () => {
			const queue = new InMemoryQueue<ActionWithTimeout>();
			const pushSpy = vi.spyOn(queue, "push");
			const peekSpy = vi.spyOn(queue, "peek");
			const popSpy = vi.spyOn(queue, "pop");
			const protocol = new TestProtocol(queue, testLogger);
			const protocolSpy = vi.spyOn(protocol, functionName);
			protocolSpy.mockResolvedValueOnce(zeroHash);
			protocol.process(action, 0);
			const actionWithTimeout = {
				...action,
				validUntil: Date.now(),
			};
			// Wait for the pop that is triggered after successful processing
			await vi.waitFor(() => {
				expect(popSpy).toHaveBeenCalled();
			});
			expect(popSpy).toBeCalledTimes(1);
			expect(pushSpy).toBeCalledTimes(1);
			expect(pushSpy).toBeCalledWith(actionWithTimeout);
			expect(protocolSpy).toBeCalledTimes(1);
			expect(protocolSpy).toBeCalledWith(actionWithTimeout);
			// Called 1 to process, 1 time after process
			expect(peekSpy).toBeCalledTimes(2);
			// Queue should be empty now
			expect(queue.peek()).toBeUndefined();
		});
	});
});
