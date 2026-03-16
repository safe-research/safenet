import type { Hex } from "viem";
import { formatHashShort } from "@/lib/safe/formatting";

export function InlineHash({ hash }: { hash: Hex }) {
	return <span className="font-mono text-sm leading-none mt-1">{formatHashShort(hash)}</span>;
}
