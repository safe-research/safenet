// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Address } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "@/lib/settings";
import { ConsensusSettingsForm } from "./ConsensusSettingsForm";

const DEFAULT_SETTINGS: Settings = {
	consensus: "0x49Db717Adec0D22235A73C3a9c2ea57AB0bC2353" as Address,
	rpc: "https://ethereum-sepolia-rpc.publicnode.com",
	decoder: "https://calldata.swiss-knife.xyz/decoder?calldata=",
	maxBlockRange: 10000,
	validatorInfo:
		"https://raw.githubusercontent.com/safe-fndn/safenet-beta-data/refs/heads/main/assets/validator-info.json",
	refetchInterval: 10000,
	blocksPerEpoch: 1440,
	signingTimeout: 12,
};

const mockUpdateSettings = vi.hoisted(() => vi.fn());

vi.mock("@/hooks/useSettings", () => ({
	useSettings: vi.fn(() => [DEFAULT_SETTINGS]),
}));

vi.mock("@/lib/settings", () => ({
	updateSettings: mockUpdateSettings,
}));

afterEach(cleanup);
beforeEach(() => {
	mockUpdateSettings.mockClear();
});

describe("ConsensusSettingsForm", () => {
	it("renders all form fields", () => {
		render(<ConsensusSettingsForm />);
		expect(screen.getByLabelText("RPC Url")).toBeTruthy();
		expect(screen.getByLabelText("Max Block Range")).toBeTruthy();
		expect(screen.getByLabelText("Decoder Url")).toBeTruthy();
		expect(screen.getByLabelText("Relayer Url")).toBeTruthy();
		expect(screen.getByLabelText("Consensus Address")).toBeTruthy();
		expect(screen.getByLabelText("Validator Info Url")).toBeTruthy();
		expect(screen.getByLabelText("Refetch Interval (0 to disable refetching)")).toBeTruthy();
		expect(screen.getByLabelText("Signing Timeout (blocks)")).toBeTruthy();
	});

	it("displays default values from settings", () => {
		render(<ConsensusSettingsForm />);
		const rpcInput = screen.getByLabelText("RPC Url") as HTMLInputElement;
		expect(rpcInput.value).toBe(DEFAULT_SETTINGS.rpc);
		const maxBlockRangeInput = screen.getByLabelText("Max Block Range") as HTMLInputElement;
		expect(maxBlockRangeInput.value).toBe(String(DEFAULT_SETTINGS.maxBlockRange));
		const refetchIntervalInput = screen.getByLabelText(
			"Refetch Interval (0 to disable refetching)",
		) as HTMLInputElement;
		expect(refetchIntervalInput.value).toBe(String(DEFAULT_SETTINGS.refetchInterval));
	});

	it("save button is disabled when form is not dirty", () => {
		render(<ConsensusSettingsForm />);
		const button = screen.getByRole("button", { name: "Save" }) as HTMLButtonElement;
		expect(button.disabled).toBe(true);
	});

	it("calls updateSettings with updated data on valid submit", async () => {
		render(<ConsensusSettingsForm />);
		const input = screen.getByLabelText("Max Block Range") as HTMLInputElement;
		fireEvent.change(input, { target: { value: "5000" } });
		fireEvent.submit(input.closest("form") as HTMLFormElement);
		await waitFor(() => {
			expect(mockUpdateSettings).toHaveBeenCalledWith(expect.objectContaining({ maxBlockRange: 5000 }));
		});
	});

	it("calls onSubmitted callback after successful save", async () => {
		const onSubmitted = vi.fn();
		render(<ConsensusSettingsForm onSubmitted={onSubmitted} />);
		const input = screen.getByLabelText("Max Block Range") as HTMLInputElement;
		fireEvent.change(input, { target: { value: "5000" } });
		fireEvent.submit(input.closest("form") as HTMLFormElement);
		await waitFor(() => {
			expect(onSubmitted).toHaveBeenCalled();
		});
	});

	it("shows a validation error for an invalid max block range value", async () => {
		render(<ConsensusSettingsForm />);
		const input = screen.getByLabelText("Max Block Range") as HTMLInputElement;
		fireEvent.change(input, { target: { value: "-1" } });
		fireEvent.submit(input.closest("form") as HTMLFormElement);
		await waitFor(() => {
			expect(screen.getByText(/too small/i)).toBeTruthy();
		});
	});

	it("displays the default signingTimeout value from settings", () => {
		render(<ConsensusSettingsForm />);
		const input = screen.getByLabelText("Signing Timeout (blocks)") as HTMLInputElement;
		expect(input.value).toBe(String(DEFAULT_SETTINGS.signingTimeout));
	});

	it("calls updateSettings with parsed signingTimeout when a string number is entered", async () => {
		render(<ConsensusSettingsForm />);
		const input = screen.getByLabelText("Signing Timeout (blocks)") as HTMLInputElement;
		fireEvent.change(input, { target: { value: "24" } });
		fireEvent.submit(input.closest("form") as HTMLFormElement);
		await waitFor(() => {
			expect(mockUpdateSettings).toHaveBeenCalledWith(expect.objectContaining({ signingTimeout: 24 }));
		});
	});

	it("shows a validation error for an invalid signing timeout value", async () => {
		render(<ConsensusSettingsForm />);
		const input = screen.getByLabelText("Signing Timeout (blocks)") as HTMLInputElement;
		fireEvent.change(input, { target: { value: "0" } });
		fireEvent.submit(input.closest("form") as HTMLFormElement);
		await waitFor(() => {
			expect(screen.getByText(/too small/i)).toBeTruthy();
		});
	});
});
