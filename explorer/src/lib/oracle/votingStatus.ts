import { type Address, getAbiItem, type Hex, type PublicClient, zeroAddress } from "viem";
import { getBlockRange, mostRecentFirst } from "@/lib/utils";
import { oracleAbi, sentinelOracleV2Abi } from "./abi";

const sentinelRequestStates = ["PENDING", "FROZEN", "RESOLVED_APPROVED", "RESOLVED_DENIED", "TIMED_OUT"] as const;

export type SentinelRequestState = (typeof sentinelRequestStates)[number];

export type VotingStatus =
	| { kind: "sentinel"; state: SentinelRequestState; approveCount: bigint; denyCount: bigint }
	| { kind: "generic"; approved: boolean }
	| null;

export const loadVotingStatus = async ({
	provider,
	oracle,
	requestId,
	maxBlockRange,
}: {
	provider: PublicClient;
	oracle: Address;
	requestId: Hex;
	maxBlockRange: bigint;
}): Promise<VotingStatus> => {
	// `getRequest` is a bare mapping read on `SentinelOracleV2` — it never reverts, even for an
	// unknown requestId, instead returning a zero-initialized struct. The catch below only fires
	// for oracles that don't implement `getRequest` at all (a plain `IOracle` like
	// `AlwaysApproveOracle`, or a still-running deprecated V1 deployment).
	try {
		const request = await provider.readContract({
			address: oracle,
			abi: sentinelOracleV2Abi,
			functionName: "getRequest",
			args: [requestId],
		});
		if (request.proposer === zeroAddress) return null;
		return {
			kind: "sentinel",
			state: sentinelRequestStates[request.state],
			approveCount: request.approveSentinelCount,
			denyCount: request.denySentinelCount,
		};
	} catch {
		const { fromBlock, toBlock } = await getBlockRange(provider, maxBlockRange);
		const logs = mostRecentFirst(
			await provider.getLogs({
				address: oracle,
				event: getAbiItem({ abi: oracleAbi, name: "OracleResult" }),
				args: { requestId },
				fromBlock,
				toBlock,
				strict: true,
			}),
		);
		const result = logs.at(0);
		return result ? { kind: "generic", approved: result.args.approved } : null;
	}
};
