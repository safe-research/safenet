import type { Address, Hex } from "viem";
import { formatError } from "../../utils/errors.js";
import type { Logger } from "../../utils/logging.js";
import type { Queue } from "../../utils/queue.js";
import type {
	ActionWithTimeout,
	AttestTransaction,
	Complain,
	ComplaintResponse,
	ConfirmKeyGen,
	ProtocolAction,
	PublishSecretShares,
	PublishSignatureShare,
	RegisterNonceCommitments,
	RequestSignature,
	RevealNonceCommitments,
	SafenetProtocol,
	SetValidatorStaker,
	StageEpoch,
	StartKeyGen,
} from "./types.js";

const ACTION_TIMEOUT = 10 * 60 * 1000; // 10 minutes
const ERROR_RETRY_DELAY = 1000;
const ERROR_RETRY_MAX_DELAY = 5000;

export type SubmittedAction = {
	nonce: number;
	hash: Hex | null;
};

export abstract class BaseProtocol implements SafenetProtocol {
	#actionQueue: Queue<ActionWithTimeout>;
	#currentAction?: ActionWithTimeout;
	#retryDelay?: number;
	#logger: Logger;

	abstract chainId(): bigint;
	abstract consensus(): Address;
	abstract coordinator(): Address;

	constructor(queue: Queue<ActionWithTimeout>, logger: Logger) {
		this.#actionQueue = queue;
		this.#logger = logger;
	}

	process(action: ProtocolAction, timeout: number = ACTION_TIMEOUT): void {
		this.#logger.info(`Enqueue ${action.id}`, { action });
		this.#actionQueue.enqueue({
			...action,
			validUntil: Date.now() + timeout,
		});
		// If no retry is scheduled, try immediate processing
		if (this.#retryDelay === undefined) {
			this.checkNextAction();
		}
	}

	private checkNextAction() {
		// An action is still processing
		if (this.#currentAction !== undefined) return;
		// Reset retry delay
		const lastRetryDelay = this.#retryDelay;
		this.#retryDelay = undefined;
		const action = this.#actionQueue.peek();
		// Nothing queued
		if (action === undefined) return;
		// Check if action is still valid
		const actionSpan = { action: { id: action.id } };
		if (action.validUntil < Date.now()) {
			this.#actionQueue.dequeue();
			this.#logger.warn("Timeout exceeded. Dropping action!", actionSpan);
			this.checkNextAction();
			return;
		}
		this.#currentAction = action;
		this.performAction(action)
			.then((submitted) => {
				// If action was successfully sent to the node, remove it from queue
				this.#logger.info(`Sent action for ${action.id} transaction`, { ...actionSpan, tx: submitted });
				this.#actionQueue.dequeue();
				this.#currentAction = undefined;
				this.checkNextAction();
			})
			.catch((err) => {
				// With each retry increase the delay until the maximum is reached
				this.#retryDelay = Math.min((lastRetryDelay ?? 0) + ERROR_RETRY_DELAY, ERROR_RETRY_MAX_DELAY);
				this.#logger.info(`Action failed, will retry after a delay of ${this.#retryDelay} ms!`, {
					...actionSpan,
					error: formatError(err),
				});
				this.#currentAction = undefined;
				setTimeout(() => {
					this.checkNextAction();
				}, this.#retryDelay);
			});
	}

	private async performAction(action: ProtocolAction): Promise<SubmittedAction> {
		switch (action.id) {
			case "key_gen_start":
				return await this.startKeyGen(action);
			case "key_gen_publish_secret_shares":
				return await this.publishKeygenSecretShares(action);
			case "key_gen_complain":
				return await this.complain(action);
			case "key_gen_complaint_response":
				return await this.complaintResponse(action);
			case "key_gen_confirm":
				return await this.confirmKeyGen(action);
			case "sign_request":
				return await this.requestSignature(action);
			case "sign_register_nonce_commitments":
				return await this.registerNonceCommitments(action);
			case "sign_reveal_nonce_commitments":
				return await this.revealNonceCommitments(action);
			case "sign_publish_signature_share":
				return await this.publishSignatureShare(action);
			case "consensus_attest_transaction":
				return await this.attestTransaction(action);
			case "consensus_stage_epoch":
				return await this.stageEpoch(action);
			case "consensus_set_validator_staker":
				return await this.setValidatorStaker(action);
		}
	}
	protected abstract startKeyGen(args: StartKeyGen): Promise<SubmittedAction>;

	protected abstract publishKeygenSecretShares(args: PublishSecretShares): Promise<SubmittedAction>;

	protected abstract complain(args: Complain): Promise<SubmittedAction>;

	protected abstract complaintResponse(args: ComplaintResponse): Promise<SubmittedAction>;

	protected abstract confirmKeyGen(args: ConfirmKeyGen): Promise<SubmittedAction>;

	protected abstract requestSignature(args: RequestSignature): Promise<SubmittedAction>;

	protected abstract registerNonceCommitments(args: RegisterNonceCommitments): Promise<SubmittedAction>;

	protected abstract revealNonceCommitments(args: RevealNonceCommitments): Promise<SubmittedAction>;

	protected abstract publishSignatureShare(args: PublishSignatureShare): Promise<SubmittedAction>;

	protected abstract attestTransaction(args: AttestTransaction): Promise<SubmittedAction>;

	protected abstract stageEpoch(args: StageEpoch): Promise<SubmittedAction>;

	protected abstract setValidatorStaker(args: SetValidatorStaker): Promise<SubmittedAction>;
}
