import http, { type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { Counter, collectDefaultMetrics, Gauge, Registry } from "prom-client";
import type { Logger } from "./logging.js";

export type Metrics = {
	blockNumber: Gauge;
	eventIndex: Gauge;
	transitions: Counter;
};

export type MetricsServiceOptions = {
	logger: Logger;
	host?: string;
	port?: number;
};

export class MetricsService {
	#logger: Logger;
	#register: Registry;
	#metrics: Metrics;
	#server: Server;
	#listenOptions: { host: string; port: number };

	constructor({ logger, host, port }: MetricsServiceOptions) {
		this.#logger = logger;
		this.#register = new Registry();
		this.#metrics = {
			blockNumber: new Gauge({
				name: "validator_block_number",
				help: "The last processed block number by the validator",
				registers: [this.#register],
			}),
			eventIndex: new Gauge({
				name: "validator_event_index",
				help: "The last processed event index by the validator",
				registers: [this.#register],
			}),
			transitions: new Counter({
				name: "validator_transitions",
				help: "Validator state transitions",
				labelNames: ["result"],
				registers: [this.#register],
			}),
		};
		collectDefaultMetrics({
			prefix: "validator_",
			register: this.#register,
		});
		this.#server = http.createServer((req, res) => this.handler(req, res));
		this.#listenOptions = {
			host: host ?? "localhost",
			port: port ?? 3555,
		};
	}

	get metrics(): Readonly<Metrics> {
		return this.#metrics;
	}

	async start(): Promise<void> {
		await new Promise((resolve) => {
			this.#server.listen(this.#listenOptions, () => resolve(undefined));
		});

		// In order to support `port = 0` for assigning a random port for
		// for serving the metrics, make sure to read it back from the server
		// address.
		if (this.#listenOptions.port === 0) {
			const address = this.#server.address();
			if (address === null || typeof address === "string") {
				throw new Error("unexpected null or string server address after start");
			}
			this.#listenOptions.port = address.port;
		}

		this.#logger.info(`serving metrics on :${this.#listenOptions.port}`);
	}

	stop(): Promise<void> {
		return new Promise((resolve) => {
			this.#server.on("close", () => resolve(undefined));
			this.#server.close();
		});
	}

	private async handler(req: IncomingMessage, res: ServerResponse): Promise<void> {
		if (req.url === "/metrics") {
			res.writeHead(200, { "Content-Type": this.#register.contentType });
			res.end(await this.#register.metrics());
		} else {
			res.writeHead(404, { "Content-Type": "text/plain" });
			res.end("Not Found\n");
		}
	}
}

export const createMetricsService = (options: MetricsServiceOptions): MetricsService => {
	return new MetricsService(options);
};
