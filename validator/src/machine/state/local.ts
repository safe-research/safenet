import type { Hex } from "viem";
import type { SignatureId } from "../../frost/types.js";
import type { ConsensusState, GroupInfo, MachineStates, RolloverState, SigningState, StateDiff } from "../types.js";
import { applyConsensus, applyMachines, type UpdatableConsensusState, type UpdatableMachineState } from "./diff.js";

const proxy = <K extends string, V, T extends Record<K, V>>(source: T, temp: Record<K, V | undefined>) =>
	new Proxy(temp, {
		get(temp, key, receiver): V | undefined {
			const keyString = String(key) as K;
			if (keyString in temp) {
				return temp[keyString];
			}
			return Reflect.get(source, key, receiver);
		},
		has(temp, key) {
			const keyString = String(key) as K;
			if (keyString in temp) {
				return temp[keyString] !== undefined;
			}
			return Reflect.has(source, key);
		},
		deleteProperty(_temp, key) {
			const keyString = String(key) as K;
			temp[keyString] = undefined;
			return true;
		},
	}) as Record<K, V>;

export class LocalMachineStates implements MachineStates {
	protected tempState: UpdatableMachineState = {
		signing: {},
	};

	constructor(private immutableState: MachineStates) {}

	public get rollover(): RolloverState {
		if (this.tempState.rollover !== undefined) {
			return this.tempState.rollover;
		}
		return this.immutableState.rollover;
	}

	public set rollover(value: RolloverState) {
		this.tempState.rollover = value;
	}

	public get signing(): Record<`0x${string}`, SigningState> {
		return proxy(this.immutableState.signing, this.tempState.signing);
	}

	apply(diff: StateDiff) {
		applyMachines(diff, this);
	}
}

export class LocalConsensusStates implements ConsensusState {
	protected tempState: UpdatableConsensusState = {
		groupPendingNonces: {},
		epochGroups: {},
		signatureIdToMessage: {},
	};

	constructor(private immutableState: ConsensusState) {}

	public get genesisGroupId(): Hex | undefined {
		if (this.tempState.genesisGroupId !== undefined) {
			return this.tempState.genesisGroupId;
		}
		return this.immutableState.genesisGroupId;
	}

	public set genesisGroupId(value: Hex | undefined) {
		this.tempState.genesisGroupId = value;
	}

	public get activeEpoch(): bigint {
		if (this.tempState.activeEpoch !== undefined) {
			return this.tempState.activeEpoch;
		}
		return this.immutableState.activeEpoch;
	}

	public set activeEpoch(value: bigint) {
		this.tempState.activeEpoch = value;
	}

	public get stagedEpoch(): bigint {
		if (this.tempState.stagedEpoch !== undefined) {
			return this.tempState.stagedEpoch;
		}
		return this.immutableState.stagedEpoch;
	}

	public set stagedEpoch(value: bigint) {
		this.tempState.stagedEpoch = value;
	}

	public get groupPendingNonces(): Record<Hex, boolean> {
		return proxy(this.immutableState.groupPendingNonces, this.tempState.groupPendingNonces ?? {});
	}

	public get epochGroups(): Record<string, GroupInfo> {
		return proxy(this.immutableState.epochGroups, this.tempState.epochGroups ?? {});
	}

	public get signatureIdToMessage(): Record<SignatureId, Hex> {
		return proxy(this.immutableState.signatureIdToMessage, this.tempState.signatureIdToMessage ?? {});
	}

	apply(diff: StateDiff) {
		applyConsensus(diff, this);
	}
}

export class TransitionState {
	machines: LocalMachineStates;
	consensus: LocalConsensusStates;
	diffs: StateDiff[] = [];
	constructor(machineState: MachineStates, consensusState: ConsensusState) {
		this.machines = new LocalMachineStates(machineState);
		this.consensus = new LocalConsensusStates(consensusState);
	}

	apply(diff: StateDiff) {
		this.diffs.push(diff);
		this.machines.apply(diff);
		this.consensus.apply(diff);
	}
}
