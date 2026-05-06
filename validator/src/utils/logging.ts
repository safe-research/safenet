import util from "node:util";
import winston, { type Logger as WinstonLogger } from "winston";
import { jsonReplacer } from "./json.js";

const LEVELS = {
	error: 0,
	warn: 1,
	notice: 2,
	info: 3,
	debug: 4,
	silly: 5,
};

export type LogLevel = keyof typeof LEVELS;
export type Logger = Pick<WinstonLogger, LogLevel>;

export type LoggingOptions = {
	level?: LogLevel | "silent";
	pretty?: boolean;
};

const SPLAT = Symbol.for("splat");
const prettyFormat = winston.format.printf(({ timestamp, level, message, [SPLAT]: splat }) => {
	// We need to turn the "splat", i.e. the extra parameters passed to the log
	// after the message, into an array for inspecting. AFAICT, `winston` always
	// gives us an `array | undefined`, but the type-system says otherwise so
	// handle as following:
	// - If we have an array, then "yay"
	// - If we have some other value, then arrayify it
	// - If we have undefined, then there are no additional args
	const args = Array.isArray(splat) ? splat : splat !== undefined ? [splat] : [];

	const text = [message, ...args]
		.map((part) => (typeof part === "string" ? part : util.inspect(part, { colors: true })))
		.join(" ");
	return `[${timestamp} ${level}]: ${text}`;
});

export const createLogger = (options: LoggingOptions): Logger => {
	winston.addColors({
		error: "red",
		warn: "yellow",
		notice: "cyan",
		info: "green",
		debug: "blue",
		silly: "magenta",
	});

	const level = options.level === "silent" ? { silent: true } : { level: options.level ?? "notice" };
	const format =
		options.pretty === true
			? winston.format.combine(winston.format.timestamp(), winston.format.colorize(), prettyFormat)
			: winston.format.json({ replacer: jsonReplacer });
	return winston.createLogger({
		...level,
		levels: LEVELS,
		format,
		transports: [new winston.transports.Console()],
	});
};
