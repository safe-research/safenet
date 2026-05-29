import type { Hex } from "viem";

export type Typed = {
	type: string;
};

export interface PacketHandler<T extends Typed> {
	hash(packet: T): Hex;
	verify(packet: T): Promise<void>;
}

export type PacketVerificationResult =
	| {
			status: "valid";
			packetId: Hex;
	  }
	| { status: "invalid"; packetId: Hex; error: Error };

export class VerificationEngine {
	#typeHandlers: Map<string, PacketHandler<Typed>>;
	#verifiedMessages: Set<Hex> = new Set();

	constructor(typeHandlers: Map<string, PacketHandler<Typed>>) {
		this.#typeHandlers = typeHandlers;
	}

	async verify(packet: Typed): Promise<PacketVerificationResult> {
		const handler = this.#typeHandlers.get(packet.type);
		if (handler === undefined) {
			throw new Error(`No handler registered for type ${packet.type}`);
		}
		const packetId = handler.hash(packet);
		try {
			await handler.verify(packet);
			this.#verifiedMessages.add(packetId);
			return {
				status: "valid",
				packetId,
			};
		} catch (err: unknown) {
			const error = err instanceof Error ? err : new Error(`unknown error: ${err}`);
			return {
				status: "invalid",
				packetId,
				error,
			};
		}
	}

	isVerified(packetId: Hex): boolean {
		return this.#verifiedMessages.has(packetId);
	}
}
