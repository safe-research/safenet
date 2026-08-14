import type { Address, Hex } from "viem";
import { describe, expect, it } from "vitest";
import { oracleRequestId } from "./hashing";

describe("oracleRequestId", () => {
	it("matches the Rust validator's transaction_proposal_hash reference vector", () => {
		// Vector derived from crates/validator/src/consensus/hashing.rs's
		// `sample_transaction_packet_hash` test.
		const hash = oracleRequestId({
			chainId: 1n,
			consensus: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as Address,
			epoch: 1n,
			oracle: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as Address,
			safeTxHash: "0xfe8b85e8d090b16fe8f142d3c9292dc1fc77daf9eb4af8f7cf4a7707d95f4028" as Hex,
			oracleData: "0x",
		});

		expect(hash).toBe("0x5ac4a916cb21b51c5b61c6d4479f7c7477856abecb96e439fb53d5462fb5b3ed");
	});
});
