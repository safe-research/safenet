import type { Hex } from "viem";

export type Typed = {
	type: string;
};

export interface PacketHandler<T extends Typed> {
	hashAndVerify(packet: T): Promise<Hex>;
}

export class VerificationEngine {
	#typeHandlers: Map<string, PacketHandler<Typed>>;
	#verfiedMessages: Set<Hex> = new Set();

	constructor(typeHandlers: Map<string, PacketHandler<Typed>>) {
		this.#typeHandlers = typeHandlers;
	}

	async verify(packet: Typed): Promise<Hex> {
		const handler = this.#typeHandlers.get(packet.type);
		if (handler === undefined)
			throw Error(`No handler registered for type ${packet.type}`);
		// Throws if packet is invalid
		const packetId = await handler.hashAndVerify(packet);
		this.#verfiedMessages.add(packetId);
		return packetId;
	}
    
	isVerified(packetId: Hex): boolean {
		return this.#verfiedMessages.has(packetId);
	}
}
