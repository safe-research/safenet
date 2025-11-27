import {
	type Address,
	encodePacked,
	type Hex,
	maxUint64,
	zeroAddress,
} from "viem";
import type { KeyGenClient } from "../consensus/keyGen/client.js";
import type {
	ProtocolAction,
	ShieldnetProtocol,
} from "../consensus/protocol/types.js";
import type { SigningClient } from "../consensus/signing/client.js";
import type { Participant } from "../consensus/storage/types.js";
import type { VerificationEngine } from "../consensus/verify/engine.js";
import type { GroupId, SignatureId } from "../frost/types.js";
import { handleEpochStaged } from "../machine/consensus/epochStaged.js";
import { handleTransactionAttested } from "../machine/consensus/transactionAttested.js";
import { handleTransactionProposed } from "../machine/consensus/transactionProposed.js";
import { handleKeyGenCommitted } from "../machine/keygen/committed.js";
import { handleKeyGenSecretShared } from "../machine/keygen/secretShares.js";
import { handleSigningCompleted } from "../machine/signing/completed.js";
import { handleRevealedNonces } from "../machine/signing/nonces.js";
import { handlePreprocess } from "../machine/signing/preprocess.js";
import { handleSigningShares } from "../machine/signing/shares.js";
import { handleSign } from "../machine/signing/sign.js";
import type {
	ConsensusState,
	MachineConfig,
	MachineStates,
	SigningState,
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
	#consensusState: ConsensusState = {
		epochGroups: new Map<bigint, GroupId>(),
		activeEpoch: 0n,
		stagedEpoch: 0n,
		groupPendingNonces: new Set<GroupId>(),
		messageSignatureRequests: new Map<Hex, SignatureId>(),
		transactionProposalInfo: new Map<
			Hex,
			{ epoch: bigint; transactionHash: Hex }
		>(),
	};
	// Sub machine state
	#machineStates: MachineStates = {
		rollover: { id: "waiting_for_rollover" },
		signing: new Map<SignatureId, SigningState>(),
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
			.then((actions) => {
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

	private async performTransition(
		transition: StateTransition,
	): Promise<ProtocolAction[]> {
		switch (transition.type) {
			case "block":
				return this.progressToBlock(transition.block);
			case "event":
				return this.processBlockEvent(
					transition.block,
					transition.index,
					transition.eventName,
					transition.eventArgs,
				);
		}
	}

	private progressToBlock(block: bigint): ProtocolAction[] {
		// Check if we are already up to date
		if (block <= this.#lastProcessedBlock) {
			return [];
		}
		this.#lastProcessedBlock = block;
		const actions: ProtocolAction[] = [];

		actions.push(...this.checkKeyGenAbort(block));
		actions.push(...this.checkKeyGenTimeouts(block));
		actions.push(...this.checkSigningTimeouts(block));

		actions.push(...this.checkGenesis());
		actions.push(...this.checkEpochRollover(block));

		return actions;
	}

	private async processBlockEvent(
		block: bigint,
		index: number,
		eventName: string,
		eventArgs: unknown,
	): Promise<ProtocolAction[]> {
		if (
			block < this.#lastProcessedBlock ||
			(block === this.#lastProcessedBlock && index <= this.#lastProcessedIndex)
		) {
			throw Error(
				`Invalid block number (${block}) and index ${index} (currently at block ${this.#lastProcessedBlock} and index ${this.#lastProcessedIndex})`,
			);
		}
		const actions: ProtocolAction[] = [];
		this.#lastProcessedIndex = index;
		actions.push(...this.progressToBlock(block));
		actions.push(...(await this.handleEvent(block, eventName, eventArgs)));
		// Check after every event if we could do a epoch rollover
		actions.push(...this.checkEpochRollover(block));
		return actions;
	}

	private applyDiff(diff: StateDiff): ProtocolAction[] {
		if (diff.signing !== undefined) {
			const [signatureId, state] = diff.signing;
			if (state === undefined) {
				this.#machineStates.signing.delete(signatureId);
			} else {
				this.#machineStates.signing.set(signatureId, state);
			}
		}
		if (diff.rollover !== undefined) {
			this.#machineStates.rollover = diff.rollover;
		}
		return diff.actions ?? [];
	}
	private async handleEvent(
		block: bigint,
		eventName: string,
		eventArgs: unknown,
	): Promise<ProtocolAction[]> {
		this.#logger?.(`Handle event ${eventName}`);
		switch (eventName) {
			case "KeyGenCommitted": {
				const diff = await handleKeyGenCommitted(
					this.#machineConfig,
					this.#keyGenClient,
					this.#consensusState,
					this.#machineStates,
					block,
					eventArgs,
				);
				return this.applyDiff(diff);
			}
			case "KeyGenSecretShared": {
				const diff = await handleKeyGenSecretShared(
					this.#machineConfig,
					this.#protocol,
					this.#verificationEngine,
					this.#keyGenClient,
					this.#signingClient,
					this.#consensusState,
					this.#machineStates,
					eventArgs,
					this.#logger,
				);
				return this.applyDiff(diff);
			}
			case "Preprocess": {
				const diff = await handlePreprocess(
					this.#signingClient,
					this.#consensusState,
					eventArgs,
					this.#logger,
				);
				return this.applyDiff(diff);
			}
			case "Sign": {
				const diff = await handleSign(
					this.#machineConfig,
					this.#verificationEngine,
					this.#signingClient,
					this.#consensusState,
					this.#machineStates,
					block,
					eventArgs,
					this.#logger,
				);
				return this.applyDiff(diff);
			}
			case "SignRevealedNonces": {
				const diff = await handleRevealedNonces(
					this.#machineConfig,
					this.#signingClient,
					this.#consensusState,
					this.#machineStates,
					block,
					eventArgs,
					this.#logger,
				);
				return this.applyDiff(diff);
			}
			case "SignShared": {
				const diff = await handleSigningShares(this.#machineStates, eventArgs);
				return this.applyDiff(diff);
			}
			case "SignCompleted": {
				const diff = await handleSigningCompleted(
					this.#machineConfig,
					this.#machineStates,
					block,
					eventArgs,
				);
				return this.applyDiff(diff);
			}
			case "EpochStaged": {
				const diff = await handleEpochStaged(
					this.#consensusState,
					this.#machineStates,
					eventArgs,
				);
				return this.applyDiff(diff);
			}
			case "TransactionProposed": {
				const diff = await handleTransactionProposed(
					this.#protocol,
					this.#verificationEngine,
					this.#consensusState,
					eventArgs,
					this.#logger,
				);
				return this.applyDiff(diff);
			}
			case "TransactionAttested": {
				const diff = await handleTransactionAttested(
					this.#machineStates,
					this.#consensusState,
					eventArgs,
				);
				return this.applyDiff(diff);
			}
			default: {
				return [];
			}
		}
	}

	private checkGenesis(): ProtocolAction[] {
		if (
			this.#machineStates.rollover.id === "waiting_for_rollover" &&
			this.#consensusState.activeEpoch === 0n &&
			this.#consensusState.stagedEpoch === 0n
		) {
			this.#logger?.("Trigger Genesis Group Generation");
			// We set no timeout for the genesis group generation
			const { groupId, actions } = this.triggerKeyGen(
				0n,
				maxUint64,
				this.#machineConfig.defaultParticipants,
				zeroAddress,
			);
			this.#consensusState.genesisGroupId = groupId;
			this.#logger?.(
				`Genesis group id: ${this.#consensusState.genesisGroupId}`,
			);
			return actions;
		}
		return [];
	}

	private checkEpochRollover(block: bigint): ProtocolAction[] {
		const currentEpoch = block / this.#machineConfig.blocksPerEpoch;
		if (
			this.#consensusState.stagedEpoch > 0n &&
			this.#consensusState.stagedEpoch <= currentEpoch
		) {
			this.#logger?.(
				`Update active epoch from ${this.#consensusState.activeEpoch} to ${this.#consensusState.stagedEpoch}`,
			);
			// Update active epoch
			this.#consensusState.activeEpoch = this.#consensusState.stagedEpoch;
			this.#consensusState.stagedEpoch = 0n;
		}
		// If no rollover is staged and new key gen was not triggered do it now
		if (
			this.#machineStates.rollover.id === "waiting_for_rollover" &&
			this.#consensusState.stagedEpoch === 0n
		) {
			// Trigger key gen for next epoch
			const nextEpoch = currentEpoch + 1n;
			this.#logger?.(`Trigger key gen for epoch ${nextEpoch}`);
			const { actions } = this.triggerKeyGen(
				nextEpoch,
				block + this.#machineConfig.keyGenTimeout,
				this.#machineConfig.defaultParticipants,
			);
			return actions;
		}
		return [];
	}

	private triggerKeyGen(
		epoch: bigint,
		deadline: bigint,
		participants: Participant[],
		consensus: Address = this.#protocol.consensus(),
	): { groupId: GroupId; actions: ProtocolAction[] } {
		if (participants.length < 2) {
			throw Error("Not enough participatns!");
		}
		// 4 bytes version, 20 bytes address, 8 bytes epoch number
		const context = encodePacked(
			["uint32", "address", "uint64"],
			[0, consensus, epoch],
		);
		const count = BigInt(participants.length);
		const threshold = count / 2n + 1n;
		const { groupId, participantsRoot, participantId, commitments, pok, poap } =
			this.#keyGenClient.setupGroup(participants, count, threshold, context);

		const actions: ProtocolAction[] = [
			{
				id: "key_gen_start",
				participants: participantsRoot,
				count,
				threshold,
				context,
				participantId,
				commitments,
				pok,
				poap,
			},
		];

		this.#logger?.(`Triggered key gen for epoch ${epoch} with ${groupId}`);
		this.#consensusState.epochGroups.set(epoch, groupId);
		this.#machineStates.rollover = {
			id: "collecting_commitments",
			nextEpoch: epoch,
			groupId,
			deadline: deadline,
		};
		return {
			groupId,
			actions,
		};
	}

	private checkKeyGenAbort(block: bigint): ProtocolAction[] {
		if (
			this.#machineStates.rollover.id === "waiting_for_rollover" ||
			this.#machineStates.rollover.groupId ===
				this.#consensusState.genesisGroupId
		) {
			return [];
		}
		const currentEpoch = block / this.#machineConfig.blocksPerEpoch;
		if (currentEpoch < this.#machineStates.rollover.nextEpoch) {
			// Still valid epoch
			return [];
		}
		this.#logger?.(
			`Abort keygen for ${this.#machineStates.rollover.nextEpoch}`,
		);
		this.#machineStates.rollover = { id: "waiting_for_rollover" };
		return [];
	}

	private checkKeyGenTimeouts(block: bigint): ProtocolAction[] {
		// No timeout in waiting state
		if (
			this.#machineStates.rollover.id !== "collecting_commitments" &&
			this.#machineStates.rollover.id !== "collecting_shares"
		)
			return [];
		// Still within deadline
		if (this.#machineStates.rollover.deadline > block) return [];
		const groupId = this.#machineStates.rollover.groupId;
		// Get participants that did not participate
		const missingParticipants =
			this.#machineStates.rollover.id === "collecting_commitments"
				? this.#keyGenClient.missingCommitments(groupId)
				: this.#keyGenClient.missingSecretShares(groupId);
		// For next key gen only consider active participants
		const participants = this.#machineConfig.defaultParticipants.filter(
			(p) => missingParticipants.indexOf(p.id) < 0,
		);
		const { actions } = this.triggerKeyGen(
			this.#machineStates.rollover.nextEpoch,
			block + this.#machineConfig.keyGenTimeout,
			participants,
		);
		return actions;
	}

	private checkSigningRequestTimeout(
		block: bigint,
		signatureId: SignatureId,
		status: SigningState,
	): ProtocolAction[] {
		// Still within deadline
		if (status.deadline > block) return [];
		this.#consensusState.messageSignatureRequests.delete(signatureId);
		switch (status.id) {
			case "waiting_for_attestation": {
				const everyoneResponsible = status.responsible === undefined;
				if (everyoneResponsible) {
					// Everyone is responsible
					// Signature request will be readded once it is submitted
					// and no more state needs to be tracked
					// if the deadline is hit again this would be a critical failure
					this.#machineStates.signing.delete(signatureId);
				} else {
					// Make everyone responsible for next retry
					this.#machineStates.signing.set(signatureId, {
						...status,
						responsible: undefined,
						deadline: block + this.#machineConfig.signingTimeout,
					});
				}
				const act =
					everyoneResponsible ||
					status.responsible === this.#signingClient.participantId(signatureId);
				if (!act) {
					return [];
				}
				const message = this.#signingClient.message(signatureId);
				if (
					this.#machineStates.rollover.id === "sign_rollover" &&
					message === this.#machineStates.rollover.message
				) {
					return [
						{
							id: "consensus_stage_epoch",
							proposedEpoch: this.#machineStates.rollover.nextEpoch,
							rolloverBlock:
								this.#machineStates.rollover.nextEpoch *
								this.#machineConfig.blocksPerEpoch,
							groupId: this.#machineStates.rollover.groupId,
							signatureId,
						},
					];
				}
				const transactionInfo =
					this.#consensusState.transactionProposalInfo.get(message);
				if (transactionInfo !== undefined) {
					return [
						{
							id: "consensus_attest_transaction",
							...transactionInfo,
							signatureId,
						},
					];
				}
				return [];
			}
			case "waiting_for_request": {
				const everyoneResponsible = status.responsible === undefined;
				if (everyoneResponsible) {
					// Everyone is responsible
					// Signature request will be readded once it is submitted
					// and no more state needs to be tracked
					// if the deadline is hit again this would be a critical failure
					this.#machineStates.signing.delete(signatureId);
				} else {
					// Make everyone responsible for next retry
					this.#machineStates.signing.set(signatureId, {
						...status,
						signers: status.signers.filter((id) => id !== status.responsible),
						responsible: undefined,
						deadline: block + this.#machineConfig.signingTimeout,
					});
				}
				const act =
					everyoneResponsible ||
					status.responsible === this.#signingClient.participantId(signatureId);
				if (!act) {
					return [];
				}
				const message = this.#signingClient.message(signatureId);
				const groupId = this.#signingClient.signingGroup(signatureId);
				return [
					{
						id: "sign_request",
						groupId,
						message,
					},
				];
			}
			case "collect_nonce_commitments":
			case "collect_signing_shares": {
				// Still within deadline
				if (status.deadline <= block) return [];
				// Get participants that did not participate
				const missingParticipants =
					status.id === "collect_nonce_commitments"
						? this.#signingClient.missingNonces(signatureId)
						: this.#signingClient
								.signers(signatureId)
								.filter((s) => status.sharesFrom.indexOf(s) < 0);
				// For next key gen only consider active participants
				const signers = this.#machineConfig.defaultParticipants
					.filter((p) => missingParticipants.indexOf(p.id) < 0)
					.map((p) => p.id);
				this.#machineStates.signing.set(signatureId, {
					id: "waiting_for_request",
					responsible: status.lastSigner,
					signers,
					deadline: block + this.#machineConfig.signingTimeout,
				});
				const groupId = this.#signingClient.signingGroup(signatureId);
				const message = this.#signingClient.message(signatureId);
				return [
					{
						id: "sign_request",
						groupId,
						message,
					},
				];
			}
		}
	}

	private checkSigningTimeouts(block: bigint): ProtocolAction[] {
		// No timeout in waiting state
		const statesToProcess = Array.from(this.#machineStates.signing.entries());
		const actions: ProtocolAction[] = [];
		for (const [signatureId, status] of statesToProcess) {
			actions.push(
				...this.checkSigningRequestTimeout(block, signatureId, status),
			);
		}
		return [];
	}
}
