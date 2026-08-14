import { type Address, getAbiItem, type Hex, type PublicClient, zeroAddress } from "viem";
import { getBlockRange, loadChainId, mostRecentFirst } from "@/lib/utils";
import { oracleAbi, sentinelOracleAbi } from "./abi";
import { oracleRequestId } from "./hashing";

const sentinelRequestStates = ["PENDING", "FROZEN", "RESOLVED_APPROVED", "RESOLVED_DENIED", "TIMED_OUT"] as const;

export type SentinelRequestState = (typeof sentinelRequestStates)[number];

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

	// `getRequest` is a bare mapping read on `SentinelOracle` — it never reverts, even for an
	// unknown requestId, instead returning a zero-initialized struct. The catch below only fires
	// for oracles that don't implement `getRequest` at all (a plain `IOracle` like
	// `AlwaysApproveOracle`).
	try {
		const request = await provider.readContract({
			address: oracle,
			abi: sentinelOracleAbi,
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
