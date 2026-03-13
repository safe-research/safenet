// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import type { Hex } from "viem";
import { afterEach, describe, expect, it } from "vitest";
import { InlineHash } from "./InlineHash";

afterEach(cleanup);

const HASH = "0x9f1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab7a" as Hex;

describe("InlineHash", () => {
	it("renders the formatted hash using formatHashShort", () => {
		render(<InlineHash hash={HASH} />);
		// formatHashShort returns "0x" + first 8 hex chars + "…" + last 8 hex chars
		expect(screen.getByText("0x9f123456…7890ab7a")).toBeTruthy();
	});

	it("applies font-mono class", () => {
		const { container } = render(<InlineHash hash={HASH} />);
		expect(container.firstChild).toBeTruthy();
		expect((container.firstChild as HTMLElement).className).toContain("font-mono");
	});
});
