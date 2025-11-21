import { type Address, encodePacked, type Hex, zeroAddress } from "viem";
import type { KeyGenClient } from "../consensus/keyGen/client.js";
import type { SigningClient } from "../consensus/signing/client.js";
import { decodeSequence } from "../consensus/signing/nonces.js";
import type { Participant, ShieldnetProtocol } from "../consensus/types.js";
import type { VerificationEngine } from "../consensus/verify/engine.js";
import type { EpochRolloverPacket } from "../consensus/verify/rollover/schemas.js";
import { toPoint } from "../frost/math.js";
import type { GroupId, ParticipantId, SignatureId } from "../frost/types.js";
import {
	epochProposedEventSchema,
	keyGenCommittedEventSchema,
	keyGenSecretSharedEventSchema,
	nonceCommitmentsEventSchema,
	nonceCommitmentsHashEventSchema,
	signatureShareEventSchema,
	signedEventSchema,
	signRequestEventSchema,
} from "../types/schemas.js";

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

	async progressToBlock(block: bigint) {
		if (block < this.#lastProcessedBlock) {
			throw Error(`There is no going back!`);
		}
		// Check if we are already up to date
		if (block === this.#lastProcessedBlock) return;
		this.#lastProcessedBlock = block;

		await this.checkGenesis();
		await this.checkEpochRollover(block);
		await this.checkAvailableNonces();

		// TODO:
		// Check keyGen timeouts
		// Check signing timeouts
	}

	async processBlockEvent(
		block: bigint,
		index: number,
		eventName: string,
		eventArgs: unknown,
	) {
		if (
			block < this.#lastProcessedBlock ||
			(block === this.#lastProcessedBlock && index <= this.#lastProcessedIndex)
		) {
			throw Error(
				`Invalid block number (${block}) and index ${index} (currentyl at block ${this.#lastProcessedBlock} and index ${this.#lastProcessedIndex})`,
			);
		}
		this.#lastProcessedIndex = index;
		await this.progressToBlock(block);
		await this.handleEvent(eventName, eventArgs);
		// Check after every event if we could do a epoch rollover
		await this.checkEpochRollover(block);
	}

	private async handleEvent(eventName: string, eventArgs: unknown) {
		this.#logger?.(`Handle event ${eventName}`);
		switch (eventName) {
			case "KeyGenCommitted": {
				// A participant has committed to the new key gen
				// Ignore if not in "collecting_commitments" state
				if (this.#keyGenState.id !== "collecting_commitments") return;
				// Parse event from raw data
				const event = keyGenCommittedEventSchema.parse(eventArgs);
				// Verify that the group corresponds to the next epoch
				if (this.#keyGenState.groupId !== event.gid) return;
				// This will be handled by the keyGenClient
				await this.#keyGenClient.handleKeygenCommitment(
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
					const nextEpoch = this.#keyGenState.nextEpoch;
					this.#keyGenState = {
						id: "collecting_shares",
						groupId: event.gid,
						nextEpoch,
					};
				}
				return;
			}
			case "KeyGenSecretShared": {
				// A participant has submitted secret share for new group
				// Ignore if not in "collecting_shares" state
				if (this.#keyGenState.id !== "collecting_shares") return;
				// Parse event from raw data
				const event = keyGenSecretSharedEventSchema.parse(eventArgs);
				// Verify that the group corresponds to the next epoch
				if (this.#keyGenState.groupId !== event.gid) return;
				this.#keyGenState.lastParticipant = event.identifier;
				// Track identity that has submitted last share
				await this.#keyGenClient.handleKeygenSecrets(
					event.gid,
					event.identifier,
					event.share.f,
				);
				if (event.completed) {
					await this.onGroupSetup(event.gid);
				}
				return;
			}
			case "EpochProposed": {
				// This provides the data for the signing of the epoch rollover
				// Ignore if not in "request_rollover_data" state
				if (this.#keyGenState.id !== "request_rollover_data") {
					this.#logger?.(`Not expecting new epochf during ${this.#keyGenState.id}!`);
					return;
				}
				// Parse event from raw data
				const event = epochProposedEventSchema.parse(eventArgs);
				// TODO refactor verification logic
				if (event.activeEpoch !== this.#activeEpoch) {
					this.#logger?.(`Proposal for unexpected active epoch ${event.activeEpoch}!`);
					return;
				}
				if (event.proposedEpoch !== this.#keyGenState.nextEpoch) {
					this.#logger?.(`Proposal for unexpected next epoch ${event.proposedEpoch}!`);
					return;
				}
				if (event.rolloverBlock !== event.proposedEpoch * this.#blocksPerEpoch) {
					this.#logger?.(`Proposal for unexpected rollover block ${event.rolloverBlock}!`);
					return;
				}
				const groupKey = this.#keyGenClient.groupPublicKey(
					this.#keyGenState.groupId,
				);
				if (groupKey === undefined) {
					this.#logger?.(`Missing group key!`);
					return;
				}
				if (groupKey.x !== event.groupKey.x || groupKey.y !== event.groupKey.y) {
					this.#logger?.(`Proposal with unexpected group key!`);
					return;
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
				return;
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
				await this.#signingClient.handleNonceCommitmentsHash(
					event.gid,
					event.identifier,
					event.commitment,
					event.chunk,
				);
				return;
			}
			case "Sign": {
				// The signature process has been started
				// Parse event from raw data
				const event = signRequestEventSchema.parse(eventArgs);
				// Check that signing was initiated via consensus contract
				if (event.initiator !== this.#protocol.consensus()) {
					this.#logger?.(`Unexpected initiator ${event.initiator}!`);
					return;
				}
				// Check that message is verified
				if (!this.#verificationEngine.isVerified(event.message)) {
					this.#logger?.(`Message ${event.message} not verified!`);
					return;
				}
				const status = this.#signingState.get(event.sid);
				// Check that state for signature id is "not_started"
				if (status !== undefined) {
					this.#logger?.(`Alreay started signing ${event.sid}!`);
					return;
				}
				// Check that it has the expected sequence
				const currentSequence = this.#groupSequence.get(event.gid) ?? 0n;
				if (currentSequence !== event.sequence) {
					this.#logger?.(`Unexpected sequence ${event.sequence}!`);
					return;
				}
				this.#groupSequence.set(event.gid, currentSequence + 1n);
				await this.#signingClient.handleSignatureRequest(
					event.gid,
					event.sid,
					event.message,
					event.sequence,
				);
				// Update state for signature id to "collect_nonce_commitments"
				this.#signingState.set(event.sid, {
					id: "collect_nonce_commitments",
				});
				return;
			}
			case "SignRevealedNonces": {
				// A participant has submitted nonces for a signature id
				// Parse event from raw data
				const event = nonceCommitmentsEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_nonce_commitments"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_nonce_commitments") return;
				const submissionId = await this.#signingClient.handleNonceCommitments(
					event.sid,
					event.identifier,
					{
						hidingNonceCommitment: toPoint(event.nonces.d),
						bindingNonceCommitment: toPoint(event.nonces.e),
					},
				);
				// If all participants have committed update state for request id to "collect_signing_shares"
				if (submissionId !== undefined) {
					this.#signingState.set(event.sid, { id: "collect_signing_shares" });
				}
				return;
			}
			case "SignShared": {
				// A participant has submitted a singature share for a signature id
				// Parse event from raw data
				const event = signatureShareEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_signing_shares"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_signing_shares") return;
				// Track identity that has submitted last share
				status.lastSigner = event.identifier;
				return;
			}
			case "SignCompleted": {
				// A participant has submitted a singature share for a signature id
				// Parse event from raw data
				const event = signedEventSchema.parse(eventArgs);
				// Check that state for signature id is "collect_signing_shares"
				const status = this.#signingState.get(event.sid);
				if (status?.id !== "collect_signing_shares") return;

				const lastSigner = status.lastSigner;
				this.#signingState.set(event.sid, { id: "signed" });
				// If msg is rollover message check epoch update
				await this.checkEpochStaging(event.sid, lastSigner);
				return;
			}
			default: {
			}
		}
	}

	private async checkGenesis() {
		if (
			this.#keyGenState.id === "waiting_for_rollover" &&
			this.#activeEpoch === 0n &&
			this.#stagedEpoch === 0n
		) {
			this.#logger?.("Trigger Genesis Group Generation");
			this.#genesisGroupId = await this.triggerKeyGen(0n, zeroAddress);
			this.#logger?.(`Genesis group id: ${this.#genesisGroupId}`);
		}
	}

	private async checkEpochRollover(block: bigint) {
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
			// TODO discussed format, my proposal encodePacked(0x00000000, Consensus.address, nextEpoch) -> 4 bytes version, 20 bytes address, 8 bytes Epoch number)
			const nextEpoch = currentEpoch + 1n;
			this.#logger?.(`Trigger key gen for epoch ${nextEpoch}`);
			await this.triggerKeyGen(nextEpoch);
		}
	}

	private async triggerKeyGen(
		epoch: bigint,
		consensus: Address = this.#protocol.consensus(),
	): Promise<GroupId> {
		const context = encodePacked(["address", "uint96"], [consensus, epoch]);
		const participantsRoot = this.#keyGenClient.registerParticipants(
			this.#participants,
		);
		const count = BigInt(this.#participants.length);
		// TODO discusss
		const threshold = (2n * count) / 3n;
		const groupId = await this.#keyGenClient.triggerKeygenAndCommit(
			participantsRoot,
			count,
			threshold,
			context,
		);
		this.#epochGroups.set(epoch, groupId);
		this.#keyGenState = {
			id: "collecting_commitments",
			nextEpoch: epoch,
			groupId,
		};
		return groupId;
	}

	private async onGroupSetup(groupId: GroupId) {
		if (
			this.#keyGenState.id !== "collecting_shares" ||
			this.#keyGenState.groupId !== groupId
		)
			return;

		// If a group is setup start preprocess (aka nonce commitment)
		this.#groupPendingNonces.add(groupId);
		await this.#signingClient.commitNonces(groupId);

		if (this.#genesisGroupId === groupId) {
			this.#logger?.(`Genesis group ready!`);
			// Don't propose rollover for genesis groups
			this.#keyGenState = { id: "waiting_for_rollover" };
			return;
		}
		const participantId = this.#keyGenClient.participantId(groupId);
		const lastParticipant = this.#keyGenState.lastParticipant;
		const nextEpoch = this.#keyGenState.nextEpoch;
		this.#keyGenState = { id: "request_rollover_data", groupId, nextEpoch };
		this.#logger?.(`Propose rollover to ${nextEpoch}!`);
		// If all participants have committed update state to "request_rollover_data"
		// If this validator has submitted last sign, trigger epoch rollover
		if (participantId === lastParticipant) {
			await this.#protocol.proposeEpoch(
				nextEpoch,
				nextEpoch * this.#blocksPerEpoch,
				groupId,
			);
		}
	}

	private async checkAvailableNonces() {
		if (
			this.#activeEpoch === 0n &&
			this.#keyGenState.id !== "waiting_for_rollover"
		) {
			// We are in the genesis setup
			return;
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
				await this.#signingClient.commitNonces(activeGroup);
			}
		}
	}

	private async checkEpochStaging(
		signatureId: SignatureId,
		lastSigner: ParticipantId | undefined,
	) {
		if (
			this.#keyGenState.id === "sign_rollover_msg" &&
			this.#keyGenState.msg === this.#signingClient.message(signatureId)
		) {
			const state = this.#signingState.get(signatureId);
			if (state?.id !== "signed") return;
			this.#stagedEpoch = this.#keyGenState.nextEpoch;
			const nextEpoch = this.#keyGenState.nextEpoch;
			const nextGroupId = this.#keyGenState.groupId;
			this.#keyGenState = { id: "waiting_for_rollover" };
			this.#logger?.(`Stage rollover to ${nextEpoch}!`);
			if (lastSigner === this.#signingClient.participantId(signatureId)) {
				await this.#protocol.stageEpoch(
					nextEpoch,
					nextEpoch * this.#blocksPerEpoch,
					nextGroupId,
					signatureId,
				);
			}
		}
	}
}
