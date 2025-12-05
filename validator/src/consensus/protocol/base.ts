import type { Address, Hex } from "viem";
import { Queue } from "../../utils/queue.js";
import type {
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

type ActionWithRetry = ProtocolAction & {
	retryCount: number;
};

const MAX_RETRIES = 5;
const ERROR_RETRY_DELAY = 1000;

export abstract class BaseProtocol implements ShieldnetProtocol {
	#actionQueue = new Queue<ActionWithRetry>();
	#currentAction?: ActionWithRetry;
	#logger?: (msg: unknown) => void;

	abstract chainId(): bigint;
	abstract consensus(): Address;
	abstract coordinator(): Address;

	constructor(logger?: (msg: unknown) => void) {
		this.#logger = logger;
	}

	process(action: ProtocolAction): void {
		this.#logger?.(`Enqueue ${action.id}`);
		this.#actionQueue.push({
			...action,
			retryCount: 0,
		});
		this.checkNextAction();
	}

	private checkNextAction() {
		// Still processing
		if (this.#currentAction !== undefined) return;
		const action = this.#actionQueue.pop();
		// Nothing queued
		if (action === undefined) return;
		if (action.retryCount > MAX_RETRIES) {
			this.#logger?.(`Max retry count exeeded for ${action.id}. Dropping action!`);
			this.checkNextAction();
			return;
		}
		this.#currentAction = action;
		const executionDelay = action.retryCount > 0 ? ERROR_RETRY_DELAY : 0;
		setTimeout(() => {
			this.performAction(action)
				.catch(() => {
					action.retryCount++;
					this.#actionQueue.return(action);
				})
				.finally(() => {
					this.#currentAction = undefined;
					this.checkNextAction();
				});
		}, executionDelay);
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
