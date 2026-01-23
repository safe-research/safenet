/**
 * Configurable backoff for throttleing requests.
 */

import type { RequestErrorType } from "viem/utils";
import { withDefaults } from "../utils/config.js";

/**
 * Configuration options for the backoff.
 */
export type Config = {
	backoffDelays: number[];
};

export const DEFAULT_CONFIG = {
	backoffDelays: [1, 2, 4, 8, 16, 32, 64].map((seconds) => seconds * 1000),
};

/**
 * Request backoff throttling.
 */
export class Backoff {
	#config: Config;
	#delay: number | null;

	constructor(config: Partial<Config>) {
		this.#config = withDefaults(config, DEFAULT_CONFIG);
		this.#delay = null;
	}

	#isRateLimitError(error: unknown) {
		const e = error as RequestErrorType | { name: undefined } | undefined | null;

		// Is it an EIP-1474 standard error code indicating that requests are being exceed the
		// allowed rate limit.
		if (e?.name === "LimitExceededRpcError") {
			return true;
		}

		// An HTTP error code indicating too many requests to the server.
		if (e?.name === "HttpRequestError" && e.status === 429) {
			return true;
		}

		return false;
	}

	/**
	 * Classifies an error and determines whether or not to increase the backoff delay.
	 */
	classify(error: unknown): void {
		if (this.#isRateLimitError(error)) {
			this.#delay = Math.min((this.#delay ?? -1) + 1, this.#config.backoffDelays.length - 1);
		} else {
			this.reset();
		}
	}

	/**
	 * Resets throttling.
	 */
	reset(): void {
		this.#delay = null;
	}

	/**
	 * Throttle a request.
	 */
	throttle(): Promise<void> {
		if (this.#delay !== null) {
			const delay = this.#config.backoffDelays[this.#delay];
			return new Promise((resolve) => setTimeout(resolve, delay));
		}
		return Promise.resolve();
	}

	/**
	 * Executes the specified request with throttling.
	 */
	async throttled<T>(request: () => Promise<T>): Promise<T> {
		try {
			await this.throttle();
			const result = await request();
			this.reset();
			return result;
		} catch (error) {
			this.classify(error);
			throw error;
		}
	}
}
