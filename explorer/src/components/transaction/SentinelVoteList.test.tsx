// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Address } from "viem";
import { afterEach, describe, expect, it } from "vitest";
import type { SentinelVote } from "@/lib/oracle";
import { SentinelVoteList } from "./SentinelVoteList";

afterEach(cleanup);

const SENTINEL_A = "0x0000000000000000000000000000000000000011" as Address;
const SENTINEL_B = "0x0000000000000000000000000000000000000022" as Address;
const SENTINEL_C = "0x0000000000000000000000000000000000000033" as Address;

describe("SentinelVoteList", () => {
	it("renders nothing when no one has voted", () => {
		const { container } = render(<SentinelVoteList votes={[]} />);
		expect(container.textContent).toBe("");
	});

	it("lists every vote, using the emoji suffix to indicate each one's own state", () => {
		const votes: SentinelVote[] = [
			{ sentinel: SENTINEL_A, state: "committed" },
			{ sentinel: SENTINEL_B, state: "approved", reason: "looks fine" },
			{ sentinel: SENTINEL_C, state: "denied", reason: "suspicious" },
		];
		render(<SentinelVoteList votes={votes} />);

		expect(screen.getByText("Votes:")).toBeTruthy();
		expect(screen.getByText("0x0000…0011 ⏳")).toBeTruthy();
		expect(screen.getByText("0x0000…0022 ✅")).toBeTruthy();
		expect(screen.getByText("0x0000…0033 ❌")).toBeTruthy();

		fireEvent.click(screen.getByText("0x0000…0011 ⏳"));
		expect(screen.getByText(SENTINEL_A)).toBeTruthy();
		fireEvent.click(screen.getByText("0x0000…0022 ✅"));
		expect(screen.getByText(SENTINEL_B)).toBeTruthy();
		expect(screen.getByText("looks fine")).toBeTruthy();
		fireEvent.click(screen.getByText("0x0000…0033 ❌"));
		expect(screen.getByText(SENTINEL_C)).toBeTruthy();
		expect(screen.getByText("suspicious")).toBeTruthy();
	});
});
