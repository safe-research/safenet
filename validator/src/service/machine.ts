import {
	type Address,
	encodeFunctionData,
	encodePacked,
	type Hex,
	zeroAddress,
	zeroHash,
} from "viem";
import type { KeyGenClient } from "../consensus/keyGen/client.js";
import type {
	ProtocolAction,
	ShieldnetProtocol,
} from "../consensus/protocol/types.js";
import {
	epochProposedEventSchema,
	keyGenCommittedEventSchema,
	keyGenSecretSharedEventSchema,
	nonceCommitmentsEventSchema,
	nonceCommitmentsHashEventSchema,
	signatureShareEventSchema,
	signedEventSchema,
	signRequestEventSchema,
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
const NONCE_THRESHOLD = 100n;

type KeyGenState =
	| {
			id: "waiting_for_rollover";
	  }
	| {
			id: "collecting_commitments";
			groupId: GroupId;
			nextEpoch: bigint;
	  }
	| {
			id: "collecting_shares";
			groupId: GroupId;
			nextEpoch: bigint;
			lastParticipant?: ParticipantId;
	  }
	| {
			id: "request_rollover_data";
			groupId: GroupId;
			nextEpoch: bigint;
	  }
	| {
			id: "sign_rollover_msg";
			groupId: GroupId;
			nextEpoch: bigint;
			msg: Hex;
	  };

type SigningState =
	| {
			id: "collect_nonce_commitments";
	  }
	| {
			id: "collect_signing_shares";
			lastSigner?: ParticipantId;
	  }
	| {
			id: "signed";
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
	#participants: Participant[];
	#protocol: ShieldnetProtocol;
	#verificationEngine: VerificationEngine;
	#keyGenClient: KeyGenClient;
	#signingClient: SigningClient;
	#activeEpoch;
	#genesisGroupId?: GroupId;
	#logger?: (msg: unknown) => void;
	#blocksPerEpoch: bigint;

	#lastProcessedBlock = 0n;
	#lastProcessedIndex = 0;
	#stagedEpoch = 0n;

	#keyGenState: KeyGenState = { id: "waiting_for_rollover" };
	#signingState = new Map<SignatureId, SigningState>();

	#epochGroups = new Map<bigint, GroupId>();
	#groupSequence = new Map<GroupId, bigint>();
	#groupPendingNonces = new Set<GroupId>();
	#transactionProposalInfo = new Map<
		Hex,
		{ epoch: bigint; transactionHash: Hex }
	>();

	#transitionQueue = new Queue<StateTransition>();
	#currentTransition?: StateTransition;

	constructor({
		participants,
		protocol,
		keyGenClient,
		signingClient,
		verificationEngine,
		initialEpoch,
		logger,
		blocksPerEpoch,
	}: {
		participants: Participant[];
		protocol: ShieldnetProtocol;
		keyGenClient: KeyGenClient;
		signingClient: SigningClient;
		verificationEngine: VerificationEngine;
		initialEpoch?: bigint;
		logger?: (msg: unknown) => void;
		blocksPerEpoch?: bigint;
	}) {
		this.#participants = participants;
		this.#protocol = protocol;
		this.#keyGenClient = keyGenClient;
		this.#signingClient = signingClient;
		this.#verificationEngine = verificationEngine;
		this.#activeEpoch = initialEpoch ?? 0n;
		this.#logger = logger;
		this.#blocksPerEpoch = blocksPerEpoch ?? BLOCKS_PER_EPOCH;
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

		actions.push(...this.checkGenesis());
		actions.push(...this.checkEpochRollover(block));
		actions.push(...this.checkAvailableNonces());

		// TODO: Check keyGen timeouts
		// TODO: Check signing timeouts

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
		actions.push(...(await this.handleEvent(eventName, eventArgs)));
		// Check after every event if we could do a epoch rollover
		actions.push(...this.checkEpochRollover(block));
		return actions;
	}

	private async handleEvent(
		eventName: string,
		eventArgs: unknown,
	): Promise<ProtocolAction[]> {
		this.#logger?.(`Handle event ${eventName}`);
		switch (eventName) {
			case "KeyGenCommitted": {
				// A participant has committed to the new key gen
				// Ignore if not in "collecting_commitments" state
				if (this.#keyGenState.id !== "collecting_commitments") return [];
				// Parse event from raw data
				const event = keyGenCommittedEventSchema.parse(eventArgs);
				// Verify that the group corresponds to the next epoch
				if (this.#keyGenState.groupId !== event.gid) return [];
				const nextEpoch = this.#keyGenState.nextEpoch;
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

					this.#keyGenState = {
						id: "collecting_shares",
						groupId: event.gid,
						nextEpoch,
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
				if (this.#keyGenState.id !== "collecting_shares") return [];
				// Parse event from raw data
				const event = keyGenSecretSharedEventSchema.parse(eventArgs);
				// Verify that the group corresponds to the next epoch
				if (this.#keyGenState.groupId !== event.gid) return [];
				this.#keyGenState.lastParticipant = event.identifier;
				// Track identity that has submitted last share
				this.#keyGenClient.handleKeygenSecrets(
					event.gid,
					event.identifier,
					event.share.f,
				);
				if (event.completed) {
					return this.onGroupSetup(event.gid);
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
				// Check that it has the expected sequence
				const currentSequence = this.#groupSequence.get(event.gid) ?? 0n;
				if (currentSequence !== event.sequence) {
					this.#logger?.(`Unexpected sequence ${event.sequence}!`);
					return [];
				}
				this.#groupSequence.set(event.gid, currentSequence + 1n);
				const status = this.#signingState.get(event.sid);
				// Check that state for signature id is "not_started"
				if (status !== undefined) {
					this.#logger?.(`Alreay started signing ${event.sid}!`);
					return [];
				}
				// Check that signing was initiated via consensus contract
				// TODO: switch to filter by group id
				if (event.initiator !== this.#protocol.consensus()) {
					this.#logger?.(`Unexpected initiator ${event.initiator}!`);
					return [];
				}
				// Check that message is verified
				if (!this.#verificationEngine.isVerified(event.message)) {
					this.#logger?.(`Message ${event.message} not verified!`);
					return [];
				}
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
				});

				return [
					{
						id: "sign_reveal_nonce_commitments",
						signatureId: event.sid,
						nonceCommitments,
						nonceProof,
					},
				];
			}
			case "SignRevealedNonces": {
				// A participant has submitted nonces for a signature id
				// Parse event from raw data
				const event = nonceCommitmentsEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_nonce_commitments"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_nonce_commitments") return [];
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
					this.#signingState.set(event.sid, { id: "collect_signing_shares" });
					const {
						signersRoot,
						signersProof,
						groupCommitment,
						commitmentShare,
						signatureShare,
						lagrangeCoefficient,
					} = this.#signingClient.createSignatureShare(event.sid);

					const callbackContext =
						this.#keyGenState.id === "sign_rollover_msg" &&
						this.#keyGenState.msg === message
							? encodeFunctionData({
									abi: CONSENSUS_FUNCTIONS,
									functionName: "stageEpoch",
									args: [
										this.#keyGenState.nextEpoch,
										this.#keyGenState.nextEpoch * this.#blocksPerEpoch,
										this.#keyGenState.groupId,
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
				status.lastSigner = event.identifier;
				return [];
			}
			case "SignCompleted": {
				// A participant has submitted a singature share for a signature id
				// Parse event from raw data
				const event = signedEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_signing_shares"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_signing_shares") return [];

				this.#signingState.set(event.sid, { id: "signed" });
				// If msg is rollover message check epoch update
				this.checkEpochStaging(event.sid);
				return [];
			}
			case "EpochProposed": {
				// This provides the data for the signing of the epoch rollover
				// Ignore if not in "request_rollover_data" state
				if (this.#keyGenState.id !== "request_rollover_data") {
					this.#logger?.(
						`Not expecting new epochf during ${this.#keyGenState.id}!`,
					);
					return [];
				}
				// Parse event from raw data
				const event = epochProposedEventSchema.parse(eventArgs);
				// TODO these checks should happen in the verified, but the current setup is not optimal for this
				if (event.activeEpoch !== this.#activeEpoch) {
					this.#logger?.(
						`Proposal for unexpected active epoch ${event.activeEpoch}!`,
					);
					return [];
				}
				if (event.proposedEpoch !== this.#keyGenState.nextEpoch) {
					this.#logger?.(
						`Proposal for unexpected next epoch ${event.proposedEpoch}!`,
					);
					return [];
				}
				if (
					event.rolloverBlock !==
					event.proposedEpoch * this.#blocksPerEpoch
				) {
					this.#logger?.(
						`Proposal for unexpected rollover block ${event.rolloverBlock}!`,
					);
					return [];
				}
				const groupKey = this.#keyGenClient.groupPublicKey(
					this.#keyGenState.groupId,
				);
				if (groupKey === undefined) {
					this.#logger?.("Missing group key!");
					return [];
				}
				if (
					groupKey.x !== event.groupKey.x ||
					groupKey.y !== event.groupKey.y
				) {
					this.#logger?.("Proposal with unexpected group key!");
					return [];
				}
				const packet: EpochRolloverPacket = {
					type: "epoch_rollover_packet",
					domain: {
						chain: this.#protocol.chainId(),
						consensus: this.#protocol.consensus(),
					},
					rollover: {
						activeEpoch: event.activeEpoch,
						proposedEpoch: event.proposedEpoch,
						rolloverBlock: event.rolloverBlock,
						groupKeyX: event.groupKey.x,
						groupKeyY: event.groupKey.y,
					},
				};
				const message = await this.#verificationEngine.verify(packet);
				this.#logger?.(`Verified message ${message}`);
				// Update state to "sign_rollover_msg"
				this.#keyGenState = {
					id: "sign_rollover_msg",
					msg: message,
					nextEpoch: event.proposedEpoch,
					groupId: this.#keyGenState.groupId,
				};
				// The signing will be triggered in a separate event
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
			this.#keyGenState.id === "waiting_for_rollover" &&
			this.#activeEpoch === 0n &&
			this.#stagedEpoch === 0n
		) {
			this.#logger?.("Trigger Genesis Group Generation");
			const { groupId, actions } = this.triggerKeyGen(0n, zeroAddress);
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
			this.#keyGenState.id === "waiting_for_rollover" &&
			this.#stagedEpoch === 0n
		) {
			// Trigger key gen for next epoch
			const nextEpoch = currentEpoch + 1n;
			this.#logger?.(`Trigger key gen for epoch ${nextEpoch}`);
			const { actions } = this.triggerKeyGen(nextEpoch);
			return actions;
		}
		return [];
	}

	private triggerKeyGen(
		epoch: bigint,
		consensus: Address = this.#protocol.consensus(),
	): { groupId: GroupId; actions: ProtocolAction[] } {
		// 4 bytes version, 20 bytes address, 8 bytes epoch number
		const context = encodePacked(
			["uint32", "address", "uint64"],
			[0, consensus, epoch],
		);
		const participantsRoot = this.#keyGenClient.registerParticipants(
			this.#participants,
		);
		const count = BigInt(this.#participants.length);
		const threshold = count / 2n + 1n;
		const { groupId, participantId, commitments, pok, poap } =
			this.#keyGenClient.setupGroup(
				participantsRoot,
				count,
				threshold,
				context,
			);

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
		this.#keyGenState = {
			id: "collecting_commitments",
			nextEpoch: epoch,
			groupId,
		};
		return {
			groupId,
			actions,
		};
	}

	private onGroupSetup(groupId: GroupId): ProtocolAction[] {
		if (
			this.#keyGenState.id !== "collecting_shares" ||
			this.#keyGenState.groupId !== groupId
		)
			return [];

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
			this.#keyGenState = { id: "waiting_for_rollover" };
			return actions;
		}
		const nextEpoch = this.#keyGenState.nextEpoch;
		this.#keyGenState = { id: "request_rollover_data", groupId, nextEpoch };
		return actions;
	}

	private checkAvailableNonces(): ProtocolAction[] {
		if (
			this.#activeEpoch === 0n &&
			this.#keyGenState.id !== "waiting_for_rollover"
		) {
			// We are in the genesis setup
			return [];
		}
		const activeGroup = this.#epochGroups.get(this.#activeEpoch);
		if (
			activeGroup !== undefined &&
			!this.#groupPendingNonces.has(activeGroup)
		) {
			const sequence = this.#groupSequence.get(activeGroup) ?? 0n;
			let { chunk, offset } = decodeSequence(sequence);
			let _availableNonces = 0n;
			while (true) {
				const noncesInChunk = this.#signingClient.availableNoncesCount(
					activeGroup,
					chunk,
				);
				_availableNonces += noncesInChunk - offset;
				// Chunk has no nonces, meaning the chunk was not initialized yet.
				if (noncesInChunk === 0n) break;
				// Offset for next chunk should be 0 as it was not used yet
				chunk++;
				offset = 0n;
			}
			if (
				this.#signingClient.availableNoncesCount(activeGroup, sequence) -
					offset <
				NONCE_THRESHOLD
			) {
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

	private checkEpochStaging(signatureId: SignatureId) {
		if (
			this.#keyGenState.id === "sign_rollover_msg" &&
			this.#keyGenState.msg === this.#signingClient.message(signatureId)
		) {
			const state = this.#signingState.get(signatureId);
			if (state?.id !== "signed") return;
			this.#stagedEpoch = this.#keyGenState.nextEpoch;
			this.#keyGenState = { id: "waiting_for_rollover" };
		}
	}
}
