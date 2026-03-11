// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { StatusBadge } from "./StatusBadge";

afterEach(cleanup);

describe("StatusBadge", () => {
	it("renders a different badge for attested vs proposed", () => {
		const { container: attested } = render(<StatusBadge attested={true} />);
		const { container: proposed } = render(<StatusBadge attested={false} />);
		expect(attested.innerHTML).not.toBe(proposed.innerHTML);
	});
});
