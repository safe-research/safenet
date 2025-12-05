import type { Address, Hex } from "viem";
import { InMemoryQueue, type Queue } from "../../utils/queue.js";
import type {
	ActionWithTimeout,
	AttestTransaction,
	ConfirmKeyGen,
	ProtocolAction,
	PublishSecretShares,
	PublishSignatureShare,
	RegisterNonceCommitments,
	RequestSignature,
	RevealNonceCommitments,
	ShieldnetProtocol,
	StageEpoch,
	StartKeyGen,
} from "./types.js";

const ACTION_TIMEOUT = 10 * 60 * 1000; // 10 minutes
const ERROR_RETRY_DELAY = 1000;

export abstract class BaseProtocol implements ShieldnetProtocol {
	#actionQueue: Queue<ActionWithTimeout> = new InMemoryQueue<ActionWithTimeout>();
	#currentAction?: ActionWithTimeout;
	#logger?: (msg: unknown) => void;

	abstract chainId(): bigint;
	abstract consensus(): Address;
	abstract coordinator(): Address;

	constructor(queue: Queue<ActionWithTimeout>, logger?: (msg: unknown) => void) {
		this.#actionQueue = queue;
		this.#logger = logger;
	}

	process(action: ProtocolAction, timeout: number = ACTION_TIMEOUT): void {
		this.#logger?.(`Enqueue ${action.id}`);
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
		if (action.validUntil < Date.now()) {
			this.#actionQueue.pop();
			this.#logger?.(`Timeout exeeded for ${action.id}. Dropping action!`);
			this.checkNextAction();
			return;
		}
		this.#currentAction = action;
		this.performAction(action)
			.then(() => {
				// If action was successfully executed, remove it from queue
				this.#actionQueue.pop();
				this.#currentAction = undefined;
				this.checkNextAction();
			})
			.catch(() => {
				this.#logger?.("Action failed, will retry after a delay!");
				this.#currentAction = undefined;
				setTimeout(() => {
					this.checkNextAction();
				}, ERROR_RETRY_DELAY);
			});
	}

	private async performAction(action: ProtocolAction): Promise<Hex> {
		switch (action.id) {
			case "key_gen_start":
				return await this.startKeyGen(action);
			case "key_gen_publish_secret_shares":
				return await this.publishKeygenSecretShares(action);
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
	protected abstract startKeyGen(args: StartKeyGen): Promise<Hex>;

	protected abstract publishKeygenSecretShares(args: PublishSecretShares): Promise<Hex>;

	protected abstract confirmKeyGen(args: ConfirmKeyGen): Promise<Hex>;

	protected abstract requestSignature(args: RequestSignature): Promise<Hex>;

	protected abstract registerNonceCommitments(args: RegisterNonceCommitments): Promise<Hex>;

	protected abstract revealNonceCommitments(args: RevealNonceCommitments): Promise<Hex>;

	protected abstract publishSignatureShare(args: PublishSignatureShare): Promise<Hex>;

	protected abstract attestTransaction(args: AttestTransaction): Promise<Hex>;

	protected abstract stageEpoch(args: StageEpoch): Promise<Hex>;
}
