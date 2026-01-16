import { describe, expect, it } from "vitest";
import { TEST_EVENTS } from "../../__tests__/data/protocol.js";
import { logToTransition, type ProtocolLog } from "./onchain.js";
import type { EventTransition } from "./types.js";

describe.each(
	TEST_EVENTS.map(([log, transition]) => {
		return {
			log,
			transition,
		};
	}).filter((test) => test.log !== null) as { log: ProtocolLog; transition: EventTransition }[],
)("logToTransition $log.eventName", ({ log, transition }) => {
	it(`should map to ${transition.id}`, () => {
		expect(logToTransition(log)).toStrictEqual(transition);
	});
});
