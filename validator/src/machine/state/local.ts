import type { Hex } from "viem";
import type { SignatureId } from "../../frost/types.js";
import type {
	ConsensusState,
	GroupInfo,
	MachineStates,
	RolloverState,
	SigningState,
	StateDiff,
} from "../types.js";
import {
	applyConsensus,
	applyMachines,
	type UpdatableConsensusState,
	type UpdatableMachineState,
} from "./diff.js";

const proxy = <K extends string, V, T extends Record<K, V>>(
	target: T,
	temp: Record<K, V | undefined>,
) =>
	new Proxy(target, {
		get(target, key, receiver): V | undefined {
			const keyString = String(key) as K;
			if (keyString in temp) {
				return temp[keyString];
			}
			return Reflect.get(target, key, receiver);
		},
		has(target, key) {
			const keyString = String(key) as K;
			if (keyString in temp) {
				return temp[keyString] !== undefined;
			}
			return Reflect.has(target, key);
		},
		deleteProperty(_target, key) {
			const keyString = String(key) as K;
			temp[keyString] = undefined;
			return true;
		},
	});

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

	public get signing(): Record<`0x${string}`, SigningState> {
		return proxy(this.immutableState.signing, this.tempState.signing);
	}

	apply(diff: StateDiff) {
		applyMachines(diff, this.tempState);
	}
}

export class LocalConsensusStates implements ConsensusState {
	protected tempState: UpdatableConsensusState = {
		groupPendingNonces: {},
		epochGroups: {},
		signatureIdToMessage: {},
	};

	constructor(private immutableState: ConsensusState) {}

	public get genesisGroupId(): `0x${string}` | undefined {
		if (this.tempState.genesisGroupId !== undefined) {
			return this.tempState.genesisGroupId;
		}
		return this.immutableState.genesisGroupId;
	}

	public get activeEpoch(): bigint {
		if (this.tempState.activeEpoch !== undefined) {
			return this.tempState.activeEpoch;
		}
		return this.immutableState.activeEpoch;
	}

	public get stagedEpoch(): bigint {
		if (this.tempState.stagedEpoch !== undefined) {
			return this.tempState.stagedEpoch;
		}
		return this.immutableState.stagedEpoch;
	}

	public get groupPendingNonces(): Record<Hex, boolean> {
		return proxy(
			this.immutableState.groupPendingNonces,
			this.tempState.groupPendingNonces ?? {},
		);
	}

	public get epochGroups(): Record<string, GroupInfo> {
		return proxy(
			this.immutableState.epochGroups,
			this.tempState.epochGroups ?? {},
		);
	}

	public get signatureIdToMessage(): Record<SignatureId, Hex> {
		return proxy(
			this.immutableState.signatureIdToMessage,
			this.tempState.signatureIdToMessage ?? {},
		);
	}

	apply(diff: StateDiff) {
		applyConsensus(diff, this.tempState);
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
