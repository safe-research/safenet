import type { Address, Hex } from "viem";
import type { Logger } from "../../utils/logging.js";
import { InMemoryQueue, type Queue } from "../../utils/queue.js";
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
	StageEpoch,
	StartKeyGen,
} from "./types.js";

const ACTION_TIMEOUT = 10 * 60 * 1000; // 10 minutes
const ERROR_RETRY_DELAY = 1000;

export abstract class BaseProtocol implements SafenetProtocol {
	#actionQueue: Queue<ActionWithTimeout> = new InMemoryQueue<ActionWithTimeout>();
	#currentAction?: ActionWithTimeout;
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
		this.#actionQueue.push({
			...action,
			validUntil: Date.now() + timeout,
		});
		this.checkNextAction();
	}

	private checkNextAction() {
		// Still processing
		if (this.#currentAction !== undefined) return;
		const action = this.#actionQueue.peek();
		// Nothing queued
		if (action === undefined) return;
		// Check if action is still valid
		const actionSpan = { action: { id: action.id } };
		if (action.validUntil < Date.now()) {
			this.#actionQueue.pop();
			this.#logger.warn("Timeout exeeded. Dropping action!", actionSpan);
			this.checkNextAction();
			return;
		}
		this.#currentAction = action;
		this.performAction(action)
			.then((transactionHash) => {
				// If action was successfully sent to the node, remove it from queue
				this.#logger.info(`Sent action for ${action.id} transaction`, { ...actionSpan, transactionHash });
				this.#actionQueue.pop();
				this.#currentAction = undefined;
				this.checkNextAction();
			})
			.catch((err) => {
				this.#logger.info("Action failed, will retry after a delay!", { ...actionSpan, ...err });
				this.#currentAction = undefined;
				setTimeout(() => {
					this.checkNextAction();
				}, ERROR_RETRY_DELAY);
			});
	}

	private async performAction(action: ProtocolAction): Promise<Hex | null> {
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
		}
	}
	protected abstract startKeyGen(args: StartKeyGen): Promise<Hex | null>;

	protected abstract publishKeygenSecretShares(args: PublishSecretShares): Promise<Hex | null>;

	protected abstract complain(args: Complain): Promise<Hex | null>;

	protected abstract complaintResponse(args: ComplaintResponse): Promise<Hex | null>;

	protected abstract confirmKeyGen(args: ConfirmKeyGen): Promise<Hex | null>;

	protected abstract requestSignature(args: RequestSignature): Promise<Hex | null>;

	protected abstract registerNonceCommitments(args: RegisterNonceCommitments): Promise<Hex | null>;

	protected abstract revealNonceCommitments(args: RevealNonceCommitments): Promise<Hex | null>;

	protected abstract publishSignatureShare(args: PublishSignatureShare): Promise<Hex | null>;

	protected abstract attestTransaction(args: AttestTransaction): Promise<Hex | null>;

	protected abstract stageEpoch(args: StageEpoch): Promise<Hex | null>;
}
