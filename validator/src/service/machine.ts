import type { Hex } from "viem";
import type { KeyGenClient } from "../consensus/keyGen/client.js";
import type { ProtocolAction, ShieldnetProtocol } from "../consensus/protocol/types.js";
import type { SigningClient } from "../consensus/signing/client.js";
import type { Participant } from "../consensus/storage/types.js";
import type { VerificationEngine } from "../consensus/verify/engine.js";
import { handleEpochProposed } from "../machine/consensus/epochProposed.js";
import { handleEpochStaged } from "../machine/consensus/epochStaged.js";
import { checkEpochRollover } from "../machine/consensus/rollover.js";
import { handleTransactionAttested } from "../machine/consensus/transactionAttested.js";
import { handleTransactionProposed } from "../machine/consensus/transactionProposed.js";
import { checkKeyGenAbort } from "../machine/keygen/abort.js";
import { handleKeyGenCommitted } from "../machine/keygen/committed.js";
import { handleKeyGenConfirmed } from "../machine/keygen/confirmed.js";
import { handleGenesisKeyGen } from "../machine/keygen/genesis.js";
import { handleKeyGenSecretShared } from "../machine/keygen/secretShares.js";
import { checkKeyGenTimeouts } from "../machine/keygen/timeouts.js";
import { handleSigningCompleted } from "../machine/signing/completed.js";
import { handleRevealedNonces } from "../machine/signing/nonces.js";
import { handlePreprocess } from "../machine/signing/preprocess.js";
import { handleSigningShares } from "../machine/signing/shares.js";
import { handleSign } from "../machine/signing/sign.js";
import { checkSigningTimeouts } from "../machine/signing/timeouts.js";
import { TransitionState } from "../machine/state/local.js";
import type { StateStorage } from "../machine/storage/types.js";
import type { EventTransition, StateTransition } from "../machine/transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../machine/types.js";
import { InMemoryQueue, type Queue } from "../utils/queue.js";

const BLOCKS_PER_EPOCH = (24n * 60n * 60n) / 5n; // ~ blocks for 1 day
const DEFAULT_TIMEOUT = (10n * 60n) / 5n; // ~ blocks for 10 minutes

export class ShieldnetStateMachine {
	// Injected logic
	#protocol: ShieldnetProtocol;
	#verificationEngine: VerificationEngine;
	#keyGenClient: KeyGenClient;
	#signingClient: SigningClient;
	#storage: StateStorage;
	#logger?: (msg: unknown) => void;
	// Config Parameters
	#machineConfig: MachineConfig;
	// Event queue state
	#lastProcessedBlock = 0n;
	#lastProcessedIndex = 0;
	#transitionQueue: Queue<StateTransition> = new InMemoryQueue<StateTransition>();
	#currentTransition?: StateTransition;

	constructor({
		participants,
		protocol,
		keyGenClient,
		signingClient,
		verificationEngine,
		logger,
		genesisSalt,
		blocksPerEpoch,
		keyGenTimeout,
		signingTimeout,
		storage,
	}: {
		participants: Participant[];
		genesisSalt: Hex;
		protocol: ShieldnetProtocol;
		keyGenClient: KeyGenClient;
		signingClient: SigningClient;
		verificationEngine: VerificationEngine;
		logger?: (msg: unknown) => void;
		blocksPerEpoch?: bigint;
		keyGenTimeout?: bigint;
		signingTimeout?: bigint;
		storage: StateStorage;
	}) {
		this.#machineConfig = {
			defaultParticipants: participants,
			genesisSalt: genesisSalt,
			blocksPerEpoch: blocksPerEpoch ?? BLOCKS_PER_EPOCH,
			keyGenTimeout: keyGenTimeout ?? DEFAULT_TIMEOUT,
			signingTimeout: signingTimeout ?? DEFAULT_TIMEOUT,
		};
		this.#protocol = protocol;
		this.#keyGenClient = keyGenClient;
		this.#signingClient = signingClient;
		this.#verificationEngine = verificationEngine;
		this.#storage = storage;
		this.#logger = logger;
	}

	transition(transition: StateTransition) {
		this.#logger?.(`Enqueue ${transition.id} at ${transition.block}`);
		this.#transitionQueue.push(transition);
		this.checkNextTransition();
	}

	private checkNextTransition() {
		// Still processing
		if (this.#currentTransition !== undefined) return;
		const transition = this.#transitionQueue.peek();
		// Nothing queued
		if (transition === undefined) return;
		this.#currentTransition = transition;
		this.performTransition(transition)
			.then((diffs) => {
				const actions: ProtocolAction[] = [];
				for (const diff of diffs) {
					actions.push(...this.#storage.applyDiff(diff));
				}
				for (const action of actions) {
					this.#protocol.process(action);
				}
			})
			.catch(this.#logger)
			.finally(() => {
				this.#transitionQueue.pop();
				this.#currentTransition = undefined;
				this.checkNextTransition();
			});
	}

	private async performTransition(transition: StateTransition): Promise<StateDiff[]> {
		switch (transition.id) {
			case "block_new":
				return this.progressToBlock(transition.block);
			default:
				return this.processEventTransition(transition);
		}
	}

	private progressToBlock(
		block: bigint,
		state: TransitionState = new TransitionState(this.#storage.machineStates(), this.#storage.consensusState()),
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

	private async processEventTransition(transition: EventTransition): Promise<StateDiff[]> {
		const { block, index } = transition;
		if (block < this.#lastProcessedBlock || (block === this.#lastProcessedBlock && index <= this.#lastProcessedIndex)) {
			throw new Error(
				`Invalid block number (${block}) and index ${index} (currently at block ${this.#lastProcessedBlock} and index ${this.#lastProcessedIndex})`,
			);
		}
		this.#lastProcessedIndex = index;
		const state = new TransitionState(this.#storage.machineStates(), this.#storage.consensusState());
		this.progressToBlock(block, state);
		state.apply(await this.handleEvent(block, transition, state.consensus, state.machines));
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

	private async handleEvent(
		_block: bigint,
		transition: EventTransition,
		consensusState: ConsensusState,
		machineStates: MachineStates,
	): Promise<StateDiff> {
		this.#logger?.(`Handle event ${transition.id}`);
		switch (transition.id) {
			case "event_key_gen": {
				return await handleGenesisKeyGen(
					this.#machineConfig,
					this.#keyGenClient,
					consensusState,
					machineStates,
					transition,
					this.#logger,
				);
			}
			case "event_key_gen_committed": {
				return await handleKeyGenCommitted(this.#machineConfig, this.#keyGenClient, machineStates, transition);
			}
			case "event_key_gen_secret_shared": {
				return await handleKeyGenSecretShared(
					this.#machineConfig,
					this.#keyGenClient,
					machineStates,
					transition,
					this.#logger,
				);
			}
			case "event_key_gen_confirmed": {
				return await handleKeyGenConfirmed(
					this.#keyGenClient,
					this.#signingClient,
					consensusState,
					machineStates,
					transition,
					this.#logger,
				);
			}
			// aka Preprocess
			case "event_nonce_commitments_hash": {
				return await handlePreprocess(this.#signingClient, consensusState, transition, this.#logger);
			}
			case "event_sign_request": {
				return await handleSign(
					this.#machineConfig,
					this.#verificationEngine,
					this.#signingClient,
					consensusState,
					machineStates,
					transition,
					this.#logger,
				);
			}
			case "event_nonce_commitments": {
				return await handleRevealedNonces(
					this.#machineConfig,
					this.#signingClient,
					consensusState,
					machineStates,
					transition,
				);
			}
			case "event_signature_share": {
				return await handleSigningShares(consensusState, machineStates, transition);
			}
			case "event_signed": {
				return await handleSigningCompleted(this.#machineConfig, consensusState, machineStates, transition);
			}
			case "event_epoch_proposed": {
				return await handleEpochProposed(
					this.#machineConfig,
					this.#protocol,
					this.#verificationEngine,
					machineStates,
					transition,
					this.#logger,
				);
			}
			case "event_epoch_staged": {
				return await handleEpochStaged(machineStates, transition);
			}
			case "event_transaction_proposed": {
				return await handleTransactionProposed(
					this.#machineConfig,
					this.#protocol,
					this.#verificationEngine,
					consensusState,
					transition,
					this.#logger,
				);
			}
			case "event_transaction_attested": {
				return await handleTransactionAttested(machineStates, transition);
			}
		}
	}
}
