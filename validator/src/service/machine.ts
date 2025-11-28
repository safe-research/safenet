import {
	type Address,
	encodeFunctionData,
	encodePacked,
	type Hex,
	maxUint64,
	zeroAddress,
	zeroHash,
} from "viem";
import type { KeyGenClient } from "../consensus/keyGen/client.js";
import type {
	ProtocolAction,
	ShieldnetProtocol,
} from "../consensus/protocol/types.js";
import {
	epochStagedEventSchema,
	keyGenCommittedEventSchema,
	keyGenSecretSharedEventSchema,
	nonceCommitmentsEventSchema,
	nonceCommitmentsHashEventSchema,
	signatureShareEventSchema,
	signedEventSchema,
	signRequestEventSchema,
	transactionAttestedEventSchema,
	transactionProposedEventSchema,
} from "../consensus/schemas.js";
import type { SigningClient } from "../consensus/signing/client.js";
import { decodeSequence } from "../consensus/signing/nonces.js";
import type { Participant } from "../consensus/storage/types.js";
import type { VerificationEngine } from "../consensus/verify/engine.js";
import type { EpochRolloverPacket } from "../consensus/verify/rollover/schemas.js";
import type { SafeTransactionPacket } from "../consensus/verify/safeTx/schemas.js";
import { toPoint } from "../frost/math.js";
import type { GroupId, ParticipantId, SignatureId } from "../frost/types.js";
import { CONSENSUS_FUNCTIONS } from "../types/abis.js";
import { Queue } from "../utils/queue.js";

const BLOCKS_PER_EPOCH = (24n * 60n * 60n) / 5n; // ~ blocks for 1 day
const DEFAULT_TIMEOUT = (10n * 60n) / 5n; // ~ blocks for 10 minutes
const NONCE_THRESHOLD = 100n;

type RolloverState =
	| {
			id: "waiting_for_rollover";
	  }
	| {
			id: "collecting_commitments";
			groupId: GroupId;
			nextEpoch: bigint;
			deadline: bigint;
	  }
	| {
			id: "collecting_shares";
			groupId: GroupId;
			nextEpoch: bigint;
			deadline: bigint;
			lastParticipant?: ParticipantId;
	  }
	| {
			id: "sign_rollover";
			groupId: GroupId;
			nextEpoch: bigint;
			message: Hex;
			responsible: ParticipantId;
	  };

type SigningState =
	| {
			id: "waiting_for_request";
			responsible: ParticipantId | undefined;
			signers: ParticipantId[];
			deadline: bigint;
	  }
	| {
			id: "collect_nonce_commitments";
			lastSigner: ParticipantId | undefined;
			deadline: bigint;
	  }
	| {
			id: "collect_signing_shares";
			sharesFrom: ParticipantId[];
			lastSigner: ParticipantId | undefined;
			deadline: bigint;
	  }
	| {
			id: "waiting_for_attestation";
			responsible: ParticipantId | undefined;
			deadline: bigint;
	  };

export type StateTransition =
	| {
			type: "block";
			block: bigint;
	  }
	| {
			type: "event";
			block: bigint;
			index: number;
			eventName: string;
			eventArgs: unknown;
	  };

export class ShieldnetStateMachine {
	// Injected logic
	#protocol: ShieldnetProtocol;
	#verificationEngine: VerificationEngine;
	#keyGenClient: KeyGenClient;
	#signingClient: SigningClient;
	#logger?: (msg: unknown) => void;
	// Config Parameters
	#defaultParticipants: Participant[];
	#blocksPerEpoch: bigint;
	#keyGenTimeout: bigint;
	#signingTimeout: bigint;
	// Global state (i.e. Blockchain state)
	#lastProcessedBlock = 0n;
	#lastProcessedIndex = 0;
	// Consensus state (i.e. Blockchain state)
	#genesisGroupId?: GroupId;
	#epochGroups = new Map<bigint, GroupId>();
	#activeEpoch;
	#stagedEpoch = 0n;
	#groupPendingNonces = new Set<GroupId>();
	#messageSignatureRequests = new Map<Hex, SignatureId>();
	#transactionProposalInfo = new Map<
		Hex,
		{ epoch: bigint; transactionHash: Hex }
	>();
	// Event queue state
	#transitionQueue = new Queue<StateTransition>();
	#currentTransition?: StateTransition;
	// Sub machine state
	#rolloverState: RolloverState = { id: "waiting_for_rollover" };
	#signingState = new Map<SignatureId, SigningState>();

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
		this.#defaultParticipants = participants;
		this.#protocol = protocol;
		this.#keyGenClient = keyGenClient;
		this.#signingClient = signingClient;
		this.#verificationEngine = verificationEngine;
		this.#activeEpoch = initialEpoch ?? 0n;
		this.#logger = logger;
		this.#blocksPerEpoch = blocksPerEpoch ?? BLOCKS_PER_EPOCH;
		this.#keyGenTimeout = keyGenTimeout ?? DEFAULT_TIMEOUT;
		this.#signingTimeout = signingTimeout ?? DEFAULT_TIMEOUT;
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

	private async handleEvent(
		block: bigint,
		eventName: string,
		eventArgs: unknown,
	): Promise<ProtocolAction[]> {
		this.#logger?.(`Handle event ${eventName}`);
		switch (eventName) {
			case "KeyGenCommitted": {
				// A participant has committed to the new key gen
				// Ignore if not in "collecting_commitments" state
				if (this.#rolloverState.id !== "collecting_commitments") return [];
				// Parse event from raw data
				const event = keyGenCommittedEventSchema.parse(eventArgs);
				// Verify that the group corresponds to the next epoch
				if (this.#rolloverState.groupId !== event.gid) return [];
				const nextEpoch = this.#rolloverState.nextEpoch;
				// TODO: handle bad commitments -> Remove participant
				this.#keyGenClient.handleKeygenCommitment(
					event.gid,
					event.identifier,
					event.commitment.c.map((c) => toPoint(c)),
					{
						r: toPoint(event.commitment.r),
						mu: event.commitment.mu,
					},
				);
				// If all participants have committed update state to "collecting_shares"
				if (event.committed) {
					const { verificationShare, shares } =
						this.#keyGenClient.createSecretShares(event.gid);

					this.#rolloverState = {
						id: "collecting_shares",
						groupId: event.gid,
						nextEpoch,
						deadline: block + this.#keyGenTimeout,
					};

					const callbackContext =
						this.#genesisGroupId === event.gid
							? undefined
							: encodePacked(
									["uint256", "uint256"],
									[nextEpoch, nextEpoch * this.#blocksPerEpoch],
								);
					return [
						{
							id: "key_gen_publish_secret_shares",
							groupId: event.gid,
							verificationShare,
							shares,
							callbackContext,
						},
					];
				}
				return [];
			}
			case "KeyGenSecretShared": {
				// A participant has submitted secret share for new group
				// Ignore if not in "collecting_shares" state
				if (this.#rolloverState.id !== "collecting_shares") return [];
				// Parse event from raw data
				const event = keyGenSecretSharedEventSchema.parse(eventArgs);
				// Verify that the group corresponds to the next epoch
				if (this.#rolloverState.groupId !== event.gid) return [];
				this.#rolloverState.lastParticipant = event.identifier;
				// Track identity that has submitted last share
				await this.#keyGenClient.handleKeygenSecrets(
					event.gid,
					event.identifier,
					event.share.f,
				);
				if (event.completed) {
					return await this.onGroupSetup(block, event.gid);
				}
				return [];
			}
			case "Preprocess": {
				// The commited nonces need to be linked to a specific chunk
				// This can happen in any state
				// This will be handled by the signingClient
				const event = nonceCommitmentsHashEventSchema.parse(eventArgs);
				// Clear pending nonce commitments for group
				if (this.#groupPendingNonces.has(event.gid)) {
					this.#groupPendingNonces.delete(event.gid);
				}
				this.#signingClient.handleNonceCommitmentsHash(
					event.gid,
					event.identifier,
					event.commitment,
					event.chunk,
				);
				return [];
			}
			case "Sign": {
				// The signature process has been started
				// Parse event from raw data
				const event = signRequestEventSchema.parse(eventArgs);
				const actions = this.checkAvailableNonces(event.sequence);
				const status = this.#signingState.get(event.sid);
				// Check that state for signature id is "not_started"
				if (status !== undefined) {
					this.#logger?.(`Alreay started signing ${event.sid}!`);
					return actions;
				}
				// Check that signing was initiated via consensus contract
				// TODO: filter by group id
				// Check that message is verified
				if (!this.#verificationEngine.isVerified(event.message)) {
					this.#logger?.(`Message ${event.message} not verified!`);
					return actions;
				}
				// Check if there is already a request for this message
				const signatureRequest = this.#messageSignatureRequests.get(
					event.message,
				);
				// Only allow one concurrent signing process per message
				if (signatureRequest !== undefined) {
					this.#logger?.(`Message ${event.message} is already being signed!`);
					return actions;
				}
				this.#messageSignatureRequests.set(event.message, event.sid);

				const { nonceCommitments, nonceProof } =
					this.#signingClient.createNonceCommitments(
						event.gid,
						event.sid,
						event.message,
						event.sequence,
					);
				// Update state for signature id to "collect_nonce_commitments"
				this.#signingState.set(event.sid, {
					id: "collect_nonce_commitments",
					deadline: block + this.#signingTimeout,
					lastSigner: undefined,
				});

				actions.push({
					id: "sign_reveal_nonce_commitments",
					signatureId: event.sid,
					nonceCommitments,
					nonceProof,
				});
				return actions;
			}
			case "SignRevealedNonces": {
				// A participant has submitted nonces for a signature id
				// Parse event from raw data
				const event = nonceCommitmentsEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_nonce_commitments"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_nonce_commitments") return [];
				this.#signingState.set(event.sid, {
					...status,
					lastSigner: event.identifier,
				});
				const message = this.#signingClient.message(event.sid);
				const readyToSubmit = this.#signingClient.handleNonceCommitments(
					event.sid,
					event.identifier,
					{
						hidingNonceCommitment: toPoint(event.nonces.d),
						bindingNonceCommitment: toPoint(event.nonces.e),
					},
				);
				// If all participants have committed update state for request id to "collect_signing_shares"
				if (readyToSubmit) {
					this.#signingState.set(event.sid, {
						id: "collect_signing_shares",
						sharesFrom: [],
						deadline: block + this.#signingTimeout,
						lastSigner: event.identifier,
					});

					const {
						signersRoot,
						signersProof,
						groupCommitment,
						commitmentShare,
						signatureShare,
						lagrangeCoefficient,
					} = this.#signingClient.createSignatureShare(event.sid);

					const callbackContext =
						this.#rolloverState.id === "sign_rollover" &&
						this.#rolloverState.message === message
							? encodeFunctionData({
									abi: CONSENSUS_FUNCTIONS,
									functionName: "stageEpoch",
									args: [
										this.#rolloverState.nextEpoch,
										this.#rolloverState.nextEpoch * this.#blocksPerEpoch,
										this.#rolloverState.groupId,
										zeroHash,
									],
								})
							: this.buildTransactionAttestationCallback(message);
					return [
						{
							id: "sign_publish_signature_share",
							signatureId: event.sid,
							signersRoot,
							signersProof,
							groupCommitment,
							commitmentShare,
							signatureShare,
							lagrangeCoefficient,
							callbackContext,
						},
					];
				}
				return [];
			}
			case "SignShared": {
				// A participant has submitted a singature share for a signature id
				// Parse event from raw data
				const event = signatureShareEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_signing_shares"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_signing_shares") return [];
				// Track identity that has submitted last share
				status.sharesFrom.push(event.identifier);
				status.lastSigner = event.identifier;
				return [];
			}
			case "SignCompleted": {
				// The message was completely signed
				// Parse event from raw data
				const event = signedEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_signing_shares"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_signing_shares") return [];
				if (status.lastSigner === undefined) throw Error("Invalid state");

				this.#signingState.set(event.sid, {
					id: "waiting_for_attestation",
					deadline: block + this.#signingTimeout,
					responsible: status.lastSigner,
				});
				return [];
			}
			case "EpochStaged": {
				// An epoch was staged
				const event = epochStagedEventSchema.parse(eventArgs);
				this.#stagedEpoch = event.proposedEpoch;
				// Ignore if not in "request_rollover_data" state
				if (this.#rolloverState.id !== "sign_rollover") {
					throw Error(
						`Not expecting epoch staging during ${this.#rolloverState.id}!`,
					);
				}
				// Get current signature id for message
				const signatureRequest = this.#messageSignatureRequests.get(
					this.#rolloverState.message,
				);
				if (signatureRequest === undefined) return [];
				// Check that state for signature id is "collect_signing_shares"
				const status = this.#signingState.get(signatureRequest);
				if (status?.id !== "waiting_for_attestation") return [];

				// Clean up internal state
				this.#signingState.delete(signatureRequest);
				this.#messageSignatureRequests.delete(this.#rolloverState.message);
				this.#rolloverState = { id: "waiting_for_rollover" };
				return [];
			}
			case "TransactionProposed": {
				// Parse event from raw data
				this.#logger?.(eventArgs);
				const event = transactionProposedEventSchema.parse(eventArgs);
				const group = this.#epochGroups.get(event.epoch);
				if (group === undefined) {
					this.#logger?.(`Unknown epoch ${event.epoch}!`);
					return [];
				}
				const packet: SafeTransactionPacket = {
					type: "safe_transaction_packet",
					domain: {
						chain: this.#protocol.chainId(),
						consensus: this.#protocol.consensus(),
					},
					proposal: {
						epoch: event.epoch,
						transaction: event.transaction,
					},
				};
				const message = await this.#verificationEngine.verify(packet);
				this.#transactionProposalInfo.set(message, {
					epoch: event.epoch,
					transactionHash: event.transactionHash,
				});
				this.#logger?.(`Verified message ${message}`);
				// The signing will be triggered in a separate event
				return [];
			}
			case "TransactionAttested": {
				// The transaction attestation was submitted
				// Parse event from raw data
				const event = transactionAttestedEventSchema.parse(eventArgs);
				// Get current signature id for message
				const signatureRequest = this.#messageSignatureRequests.get(
					event.message,
				);
				if (signatureRequest === undefined) return [];
				// Check that state for signature id is "collect_signing_shares"
				const status = this.#signingState.get(signatureRequest);
				if (status?.id !== "waiting_for_attestation") return [];

				// Clean up internal state
				this.#signingState.delete(signatureRequest);
				this.#messageSignatureRequests.delete(event.message);
				this.#transactionProposalInfo.delete(event.message);
				return [];
			}
			default: {
				return [];
			}
		}
	}

	private buildTransactionAttestationCallback(message: Hex): Hex | undefined {
		const info = this.#transactionProposalInfo.get(message);
		if (info === undefined) {
			this.#logger?.(`Warn: Unknown proposal info for ${message}`);
			return undefined;
		}
		return encodeFunctionData({
			abi: CONSENSUS_FUNCTIONS,
			functionName: "attestTransaction",
			args: [info.epoch, info.transactionHash, zeroHash],
		});
	}

	private checkGenesis(): ProtocolAction[] {
		if (
			this.#rolloverState.id === "waiting_for_rollover" &&
			this.#activeEpoch === 0n &&
			this.#stagedEpoch === 0n
		) {
			this.#logger?.("Trigger Genesis Group Generation");
			// We set no timeout for the genesis group generation
			const { groupId, actions } = this.triggerKeyGen(
				0n,
				maxUint64,
				this.#defaultParticipants,
				zeroAddress,
			);
			this.#genesisGroupId = groupId;
			this.#logger?.(`Genesis group id: ${this.#genesisGroupId}`);
			return actions;
		}
		return [];
	}

	private checkEpochRollover(block: bigint): ProtocolAction[] {
		const currentEpoch = block / this.#blocksPerEpoch;
		if (this.#stagedEpoch > 0n && this.#stagedEpoch <= currentEpoch) {
			this.#logger?.(
				`Update active epoch from ${this.#activeEpoch} to ${this.#stagedEpoch}`,
			);
			// Update active epoch
			this.#activeEpoch = this.#stagedEpoch;
			this.#stagedEpoch = 0n;
		}
		// If no rollover is staged and new key gen was not triggered do it now
		if (
			this.#rolloverState.id === "waiting_for_rollover" &&
			this.#stagedEpoch === 0n
		) {
			// Trigger key gen for next epoch
			const nextEpoch = currentEpoch + 1n;
			this.#logger?.(`Trigger key gen for epoch ${nextEpoch}`);
			const { actions } = this.triggerKeyGen(
				nextEpoch,
				block + this.#keyGenTimeout,
				this.#defaultParticipants,
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
		this.#epochGroups.set(epoch, groupId);
		this.#rolloverState = {
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

	private async onGroupSetup(
		_block: bigint,
		groupId: GroupId,
	): Promise<ProtocolAction[]> {
		const status = this.#rolloverState;
		if (status.id !== "collecting_shares" || status.groupId !== groupId) {
			return [];
		}

		// If a group is setup start preprocess (aka nonce commitment)
		this.#groupPendingNonces.add(groupId);
		const nonceTreeRoot = this.#signingClient.generateNonceTree(groupId);
		const actions: ProtocolAction[] = [
			{
				id: "sign_register_nonce_commitments",
				groupId,
				nonceCommitmentsHash: nonceTreeRoot,
			},
		];

		if (this.#genesisGroupId === groupId) {
			this.#logger?.("Genesis group ready!");
			// Don't propose rollover for genesis groups
			this.#rolloverState = { id: "waiting_for_rollover" };
			return actions;
		}
		if (status.lastParticipant === undefined) {
			throw Error("Invalid state");
		}
		const nextEpoch = status.nextEpoch;
		const groupKey = this.#keyGenClient.groupPublicKey(groupId);
		if (groupKey === undefined) {
			throw Error("Invalid state");
		}
		// The deadline is either the timeout or when the epoch should start
		const packet: EpochRolloverPacket = {
			type: "epoch_rollover_packet",
			domain: {
				chain: this.#protocol.chainId(),
				consensus: this.#protocol.consensus(),
			},
			rollover: {
				activeEpoch: this.#activeEpoch,
				proposedEpoch: nextEpoch,
				rolloverBlock: nextEpoch * this.#blocksPerEpoch,
				groupKeyX: groupKey.x,
				groupKeyY: groupKey.y,
			},
		};
		const message = await this.#verificationEngine.verify(packet);
		this.#logger?.(`Verified message ${message}`);
		this.#rolloverState = {
			id: "sign_rollover",
			groupId,
			nextEpoch,
			message,
			responsible: status.lastParticipant,
		};
		return actions;
	}

	private checkKeyGenAbort(block: bigint): ProtocolAction[] {
		if (
			this.#rolloverState.id === "waiting_for_rollover" ||
			this.#rolloverState.groupId === this.#genesisGroupId
		) {
			return [];
		}
		const currentEpoch = block / this.#blocksPerEpoch;
		if (currentEpoch < this.#rolloverState.nextEpoch) {
			// Still valid epoch
			return [];
		}
		this.#logger?.(`Abort keygen for ${this.#rolloverState.nextEpoch}`);
		this.#rolloverState = { id: "waiting_for_rollover" };
		return [];
	}

	private checkKeyGenTimeouts(block: bigint): ProtocolAction[] {
		// No timeout in waiting state
		if (
			this.#rolloverState.id !== "collecting_commitments" &&
			this.#rolloverState.id !== "collecting_shares"
		)
			return [];
		// Still within deadline
		if (this.#rolloverState.deadline > block) return [];
		const groupId = this.#rolloverState.groupId;
		// Get participants that did not participate
		const missingParticipants =
			this.#rolloverState.id === "collecting_commitments"
				? this.#keyGenClient.missingCommitments(groupId)
				: this.#keyGenClient.missingSecretShares(groupId);
		// For next key gen only consider active participants
		const participants = this.#defaultParticipants.filter(
			(p) => missingParticipants.indexOf(p.id) < 0,
		);
		const { actions } = this.triggerKeyGen(
			this.#rolloverState.nextEpoch,
			block + this.#keyGenTimeout,
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
		this.#messageSignatureRequests.delete(signatureId);
		switch (status.id) {
			case "waiting_for_attestation": {
				const everyoneResponsible = status.responsible === undefined;
				if (everyoneResponsible) {
					// Everyone is responsible
					// Signature request will be readded once it is submitted
					// and no more state needs to be tracked
					// if the deadline is hit again this would be a critical failure
					this.#signingState.delete(signatureId);
				} else {
					// Make everyone responsible for next retry
					this.#signingState.set(signatureId, {
						...status,
						responsible: undefined,
						deadline: block + this.#signingTimeout,
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
					this.#rolloverState.id === "sign_rollover" &&
					message === this.#rolloverState.message
				) {
					return [
						{
							id: "consensus_stage_epoch",
							proposedEpoch: this.#rolloverState.nextEpoch,
							rolloverBlock:
								this.#rolloverState.nextEpoch * this.#blocksPerEpoch,
							groupId: this.#rolloverState.groupId,
							signatureId,
						},
					];
				}
				const transactionInfo = this.#transactionProposalInfo.get(message);
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
					this.#signingState.delete(signatureId);
				} else {
					// Make everyone responsible for next retry
					this.#signingState.set(signatureId, {
						...status,
						signers: status.signers.filter((id) => id !== status.responsible),
						responsible: undefined,
						deadline: block + this.#signingTimeout,
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
				const signers = this.#defaultParticipants
					.filter((p) => missingParticipants.indexOf(p.id) < 0)
					.map((p) => p.id);
				this.#signingState.set(signatureId, {
					id: "waiting_for_request",
					responsible: status.lastSigner,
					signers,
					deadline: block + this.#signingTimeout,
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
		const statesToProcess = Array.from(this.#signingState.entries());
		const actions: ProtocolAction[] = [];
		for (const [signatureId, status] of statesToProcess) {
			actions.push(
				...this.checkSigningRequestTimeout(block, signatureId, status),
			);
		}
		return [];
	}

	private checkAvailableNonces(sequence: bigint): ProtocolAction[] {
		if (
			this.#activeEpoch === 0n &&
			this.#rolloverState.id !== "waiting_for_rollover"
		) {
			// We are in the genesis setup
			return [];
		}
		const activeGroup = this.#epochGroups.get(this.#activeEpoch);
		if (
			activeGroup !== undefined &&
			!this.#groupPendingNonces.has(activeGroup)
		) {
			let { chunk, offset } = decodeSequence(sequence);
			let availableNonces = 0n;
			while (true) {
				const noncesInChunk = this.#signingClient.availableNoncesCount(
					activeGroup,
					chunk,
				);
				availableNonces += noncesInChunk - offset;
				// Chunk has no nonces, meaning the chunk was not initialized yet.
				if (noncesInChunk === 0n) break;
				// Offset for next chunk should be 0 as it was not used yet
				chunk++;
				offset = 0n;
			}
			if (availableNonces < NONCE_THRESHOLD) {
				this.#groupPendingNonces.add(activeGroup);
				this.#logger?.(`Commit nonces for ${activeGroup}!`);
				const nonceTreeRoot =
					this.#signingClient.generateNonceTree(activeGroup);

				return [
					{
						id: "sign_register_nonce_commitments",
						groupId: activeGroup,
						nonceCommitmentsHash: nonceTreeRoot,
					},
				];
			}
		}
		return [];
	}
}
