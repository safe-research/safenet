import type { ProtocolAction } from "../../consensus/protocol/types.js";
import { applyConsensus, applyMachines } from "../state/diff.js";
import type {
	ConsensusState,
	MachineStates,
	MutableConsensusState,
	MutableMachineStates,
	StateDiff,
} from "../types.js";
import type { StateStorage } from "./types.js";

export class InMemoryStateStorage implements StateStorage {
	// States
	#consensusState: MutableConsensusState;
	#machineStates: MutableMachineStates;

	constructor(consensus?: Partial<MutableConsensusState>, machines?: Partial<MutableMachineStates>) {
		this.#consensusState = {
			epochGroups: {},
			activeEpoch: 0n,
			stagedEpoch: 0n,
			groupPendingNonces: {},
			signatureIdToMessage: {},
			...consensus,
		};
		this.#machineStates = {
			rollover: machines?.rollover ?? { id: "waiting_for_rollover" },
			signing: machines?.rollover ?? {},
		};
	}

	applyDiff(diff: StateDiff): ProtocolAction[] {
		applyMachines(diff, this.#machineStates);
		applyConsensus(diff, this.#consensusState);
		return diff.actions ?? [];
	}
	consensusState(): ConsensusState {
		return this.#consensusState;
	}
	machineStates(): MachineStates {
		return this.#machineStates;
	}
}
