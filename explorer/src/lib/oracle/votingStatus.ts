import { type Address, getAbiItem, type Hex, type PublicClient } from "viem";
import { getBlockRange, loadChainId, mostRecentFirst } from "@/lib/utils";
import { oracleAbi, sentinelOracleAbi } from "./abi";
import { oracleRequestId } from "./hashing";

// `NONE` (ordinal 0) is a zero-value sentinel `SentinelOracleRequest.State` uses to mean "request
// was never created" — a real `getRequest` response never reports it as a `VotingStatus.state`
// (see the `progress.state === 0` check below), so it's excluded from the public type.
const sentinelRequestStates = [
	"NONE",
	"PENDING",
	"FROZEN",
	"RESOLVED_APPROVED",
	"RESOLVED_DENIED",
	"TIMED_OUT",
] as const;

export type SentinelRequestState = Exclude<(typeof sentinelRequestStates)[number], "NONE">;

export type VotingStatus =
	| { kind: "sentinel"; state: SentinelRequestState; approveCount: bigint; denyCount: bigint }
	| { kind: "generic"; approved: boolean }
	| null;

export const loadVotingStatus = async ({
	provider,
	oracle,
	consensus,
	epoch,
	safeTxHash,
	oracleData,
	maxBlockRange,
}: {
	provider: PublicClient;
	oracle: Address;
	consensus: Address;
	epoch: bigint;
	safeTxHash: Hex;
	oracleData: Hex;
	maxBlockRange: bigint;
}): Promise<VotingStatus> => {
	const chainId = await loadChainId(provider);
	const requestId = oracleRequestId({ chainId, consensus, epoch, oracle, safeTxHash, oracleData });

	try {
		const { progress } = await provider.readContract({
			address: oracle,
			abi: sentinelOracleAbi,
			functionName: "getRequest",
			args: [requestId],
		});
		if (progress.state === 0) return null;
		return {
			kind: "sentinel",
			state: sentinelRequestStates[progress.state] as SentinelRequestState,
			approveCount: BigInt(progress.approveSentinelCount),
			denyCount: BigInt(progress.denySentinelCount),
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
