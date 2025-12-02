import type { KeyGenClient } from "../consensus/keyGen/client.js";
import type { ProtocolAction, ShieldnetProtocol } from "../consensus/protocol/types.js";
import type { SigningClient } from "../consensus/signing/client.js";
import type { Participant } from "../consensus/storage/types.js";
import type { VerificationEngine } from "../consensus/verify/engine.js";
import { handleEpochStaged } from "../machine/consensus/epochStaged.js";
import { checkEpochRollover } from "../machine/consensus/rollover.js";
import { handleTransactionAttested } from "../machine/consensus/transactionAttested.js";
import { handleTransactionProposed } from "../machine/consensus/transactionProposed.js";
import { checkKeyGenAbort } from "../machine/keygen/abort.js";
import { handleKeyGenCommitted } from "../machine/keygen/committed.js";
import { checkGenesis } from "../machine/keygen/genesis.js";
import { handleKeyGenSecretShared } from "../machine/keygen/secretShares.js";
import { checkKeyGenTimeouts } from "../machine/keygen/timeouts.js";
import { handleSigningCompleted } from "../machine/signing/completed.js";
import { handleRevealedNonces } from "../machine/signing/nonces.js";
import { handlePreprocess } from "../machine/signing/preprocess.js";
import { handleSigningShares } from "../machine/signing/shares.js";
import { handleSign } from "../machine/signing/sign.js";
import { checkSigningTimeouts } from "../machine/signing/timeouts.js";
import { applyConsensus, applyMachines } from "../machine/state/diff.js";
import { TransitionState } from "../machine/state/local.js";
import type {
	ConsensusState,
	MachineConfig,
	MachineStates,
	MutableConsensusState,
	MutableMachineStates,
	StateDiff,
	StateTransition,
} from "../machine/types.js";
import { Queue } from "../utils/queue.js";

const BLOCKS_PER_EPOCH = (24n * 60n * 60n) / 5n; // ~ blocks for 1 day
const DEFAULT_TIMEOUT = (10n * 60n) / 5n; // ~ blocks for 10 minutes

export class ShieldnetStateMachine {
	// Injected logic
	#protocol: ShieldnetProtocol;
	#verificationEngine: VerificationEngine;
	#keyGenClient: KeyGenClient;
	#signingClient: SigningClient;
	#logger?: (msg: unknown) => void;
	// Config Parameters
	#machineConfig: MachineConfig;
	// Event queue state
	#lastProcessedBlock = 0n;
	#lastProcessedIndex = 0;
	#transitionQueue = new Queue<StateTransition>();
	#currentTransition?: StateTransition;
	// Consensus state
	#consensusState: MutableConsensusState = {
		epochGroups: {},
		activeEpoch: 0n,
		stagedEpoch: 0n,
		groupPendingNonces: {},
		signatureIdToMessage: {},
	};
	// Sub machine state
	#machineStates: MutableMachineStates = {
		rollover: { id: "waiting_for_rollover" },
		signing: {},
	};

	constructor({
		participants,
		protocol,
		keyGenClient,
		signingClient,
		verificationEngine,
		initialEpoch,
		logger,
		blocksPerEpoch,
		keyGenTimeout,
		signingTimeout,
	}: {
		participants: Participant[];
		protocol: ShieldnetProtocol;
		keyGenClient: KeyGenClient;
		signingClient: SigningClient;
		verificationEngine: VerificationEngine;
		initialEpoch?: bigint;
		logger?: (msg: unknown) => void;
		blocksPerEpoch?: bigint;
		keyGenTimeout?: bigint;
		signingTimeout?: bigint;
	}) {
		this.#machineConfig = {
			defaultParticipants: participants,
			blocksPerEpoch: blocksPerEpoch ?? BLOCKS_PER_EPOCH,
			keyGenTimeout: keyGenTimeout ?? DEFAULT_TIMEOUT,
			signingTimeout: signingTimeout ?? DEFAULT_TIMEOUT,
		};
		this.#protocol = protocol;
		this.#keyGenClient = keyGenClient;
		this.#signingClient = signingClient;
		this.#verificationEngine = verificationEngine;
		this.#consensusState.activeEpoch = initialEpoch ?? 0n;
		this.#logger = logger;
	}

	transition(transition: StateTransition) {
		this.#logger?.(`Enqueue ${transition.type} at ${transition.block}`);
		this.#transitionQueue.push(transition);
		this.checkNextTransition();
	}

	private checkNextTransition() {
		// Still processing
		if (this.#currentTransition !== undefined) return;
		const transition = this.#transitionQueue.pop();
		// Nothing queued
		if (transition === undefined) return;
		this.#currentTransition = transition;
		this.performTransition(transition)
			.then((diffs) => {
				const actions: ProtocolAction[] = [];
				for (const diff of diffs) {
					actions.push(...this.applyDiff(diff));
				}
				for (const action of actions) {
					this.#protocol.process(action);
				}
			})
			.catch(this.#logger)
			.finally(() => {
				this.#currentTransition = undefined;
				this.checkNextTransition();
			});
	}

	private async performTransition(transition: StateTransition): Promise<StateDiff[]> {
		switch (transition.type) {
			case "block":
				return this.progressToBlock(transition.block);
			case "event":
				return this.processBlockEvent(transition.block, transition.index, transition.eventName, transition.eventArgs);
		}
	}

	private progressToBlock(
		block: bigint,
		state: TransitionState = new TransitionState(this.#machineStates, this.#consensusState),
	): StateDiff[] {
		// Check if we are already up to date
		if (block <= this.#lastProcessedBlock) {
			return [];
		}
		this.#lastProcessedBlock = block;
		state.apply(checkKeyGenAbort(this.#machineConfig, state.consensus, state.machines, block, this.#logger));
		state.apply(
			checkKeyGenTimeouts(this.#machineConfig, this.#protocol, this.#keyGenClient, state.machines, block, this.#logger),
		);

		for (const diff of checkSigningTimeouts(
			this.#machineConfig,
			this.#signingClient,
			state.consensus,
			state.machines,
			block,
		)) {
			state.apply(diff);
		}
		state.apply(checkGenesis(this.#machineConfig, this.#keyGenClient, state.consensus, state.machines, this.#logger));

		state.apply(
			checkEpochRollover(
				this.#machineConfig,
				this.#protocol,
				this.#keyGenClient,
				state.consensus,
				state.machines,
				block,
				this.#logger,
			),
		);

		return state.diffs;
	}

	private async processBlockEvent(
		block: bigint,
		index: number,
		eventName: string,
		eventArgs: unknown,
	): Promise<StateDiff[]> {
		if (block < this.#lastProcessedBlock || (block === this.#lastProcessedBlock && index <= this.#lastProcessedIndex)) {
			throw new Error(
				`Invalid block number (${block}) and index ${index} (currently at block ${this.#lastProcessedBlock} and index ${this.#lastProcessedIndex})`,
			);
		}
		this.#lastProcessedIndex = index;
		const state = new TransitionState(this.#machineStates, this.#consensusState);
		this.progressToBlock(block, state);
		state.apply(await this.handleEvent(block, eventName, eventArgs, state.consensus, state.machines));
		// Check after every event if we could do a epoch rollover
		state.apply(
			checkEpochRollover(
				this.#machineConfig,
				this.#protocol,
				this.#keyGenClient,
				state.consensus,
				state.machines,
				block,
				this.#logger,
			),
		);
		return state.diffs;
	}

	private applyDiff(
		diff: StateDiff,
		machineStates: MutableMachineStates = this.#machineStates,
		consensusState: MutableConsensusState = this.#consensusState,
	): ProtocolAction[] {
		applyMachines(diff, machineStates);
		applyConsensus(diff, consensusState);
		return diff.actions ?? [];
	}

	private async handleEvent(
		block: bigint,
		eventName: string,
		eventArgs: unknown,
		consensusState: ConsensusState,
		machineStates: MachineStates,
	): Promise<StateDiff> {
		this.#logger?.(`Handle event ${eventName}`);
		switch (eventName) {
			case "KeyGenCommitted": {
				return await handleKeyGenCommitted(
					this.#machineConfig,
					this.#keyGenClient,
					consensusState,
					machineStates,
					block,
					eventArgs,
				);
			}
			case "KeyGenSecretShared": {
				return await handleKeyGenSecretShared(
					this.#machineConfig,
					this.#protocol,
					this.#verificationEngine,
					this.#keyGenClient,
					this.#signingClient,
					consensusState,
					machineStates,
					block,
					eventArgs,
					this.#logger,
				);
			}
			case "Preprocess": {
				return await handlePreprocess(this.#signingClient, consensusState, eventArgs, this.#logger);
			}
			case "Sign": {
				return await handleSign(
					this.#machineConfig,
					this.#verificationEngine,
					this.#signingClient,
					consensusState,
					machineStates,
					block,
					eventArgs,
					this.#logger,
				);
			}
			case "SignRevealedNonces": {
				return await handleRevealedNonces(
					this.#machineConfig,
					this.#signingClient,
					consensusState,
					machineStates,
					block,
					eventArgs,
				);
			}
			case "SignShared": {
				return await handleSigningShares(consensusState, machineStates, eventArgs);
			}
			case "SignCompleted": {
				return await handleSigningCompleted(this.#machineConfig, consensusState, machineStates, block, eventArgs);
			}
			case "EpochStaged": {
				return await handleEpochStaged(machineStates, eventArgs);
			}
			case "TransactionProposed": {
				return await handleTransactionProposed(
					this.#machineConfig,
					this.#protocol,
					this.#verificationEngine,
					consensusState,
					block,
					eventArgs,
					this.#logger,
				);
			}
			case "TransactionAttested": {
				return await handleTransactionAttested(machineStates, eventArgs);
			}
			default: {
				return {};
			}
		}
	}
}
