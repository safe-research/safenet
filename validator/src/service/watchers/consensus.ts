import type { Address, PublicClient } from "viem";
import { CONSENSUS_EVENTS } from "../../types/abis.js";
import { AbiPoint } from "../../types/interfaces.js";

export const watchConsensusEvents = ({
    client,
    target,
    onEpochProposed,
    onError,
}: {
    client: PublicClient;
    target: Address;
    onEpochProposed: (args: {
        activeEpoch?: bigint;
        proposedEpoch?: bigint;
        timestamp?: bigint;
        groupKey?: AbiPoint;
    }) => Promise<void>;
    onError: (error: Error) => void;
}): (() => void) => {
    // TODO: Allow to specify "fromBlock" to pick up on past events
    // TODO: Provide callback for "last block processed" for service recovery
    return client.watchContractEvent({
        address: target,
        abi: CONSENSUS_EVENTS,
        onLogs: (logs) => {
            logs.forEach((log) => {
                switch (log.eventName) {
                    case "EpochProposed":
                        onEpochProposed(log.args).catch(onError);
                        return;
                    default:
                        // Event unrelated to signing flow
                        return;
                }
            });
        },
        onError,
    });
};
