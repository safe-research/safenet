/**
 * Timer.
 *
 * This allows tests to mock timing-related calls.
 */

export type Timer = {
	sleep(ms: number): Promise<void>;
	sleepUntil(unixTimestampMs: number): Promise<void>;
};

const sleep = (ms: number): Promise<void> => {
	return new Promise((resolve) => setTimeout(resolve, ms));
};

export const DEFAULT_TIMER = {
	sleep,
	sleepUntil(unixTimestampMs: number): Promise<void> {
		const delay = unixTimestampMs - Date.now();
		return delay > 0 ? sleep(delay) : Promise.resolve();
	},
};
