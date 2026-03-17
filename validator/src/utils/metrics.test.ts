import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createLogger } from "./logging.js";
import { MetricsService } from "./metrics.js";

describe("MetricsService HTTP routes", () => {
	let service: MetricsService;

	beforeEach(async () => {
		const logger = createLogger({ level: "silent" });
		service = new MetricsService({ logger, host: "127.0.0.1", port: 0 });
		await service.start();
	});

	afterEach(async () => {
		await service.stop();
	});

	it("GET /health returns 200 with { status: 'ok' }", async () => {
		const res = await fetch(`http://127.0.0.1:${service.port}/health`);
		expect(res.status).toBe(200);
		expect(res.headers.get("content-type")).toBe("application/json");
		const body = await res.json();
		expect(body).toEqual({ status: "ok" });
	});

	it("GET /metrics returns 200 with Prometheus output", async () => {
		const res = await fetch(`http://127.0.0.1:${service.port}/metrics`);
		expect(res.status).toBe(200);
		const body = await res.text();
		expect(body).toContain("validator_");
	});

	it("GET /unknown returns 404", async () => {
		const res = await fetch(`http://127.0.0.1:${service.port}/unknown`);
		expect(res.status).toBe(404);
	});
});
