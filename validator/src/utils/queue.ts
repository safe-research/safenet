type QueueEntry<T> = {
	prev?: QueueEntry<T>;
	next?: QueueEntry<T>;
	element: T;
};

export class Queue<T> {
	#head?: QueueEntry<T>;
	#tail?: QueueEntry<T>;

	push(element: T) {
		const entry = {
			next: this.#head,
			element,
		};
		if (this.#head !== undefined) {
			this.#head.prev = entry;
		}

		this.#head = entry;
		if (this.#tail === undefined) {
			this.#tail = entry;
		}
	}

	return(element: T) {
		const entry = {
			prev: this.#tail,
			element,
		};
		if (this.#tail !== undefined) {
			this.#tail.next = entry;
		}

		this.#tail = entry;
		if (this.#head === undefined) {
			this.#head = entry;
		}
	}

	pop(): T | undefined {
		const entry = this.#tail;
		this.#tail = entry?.prev;
		if (entry?.prev === undefined) {
			this.#head = undefined;
		} else {
			entry.prev.next = undefined;
		}
		return entry?.element;
	}
}
