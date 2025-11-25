export type Logger = {
	error(error: Error): void;
	debug(msg: unknown): void;
	info(msg: unknown): void;
};
