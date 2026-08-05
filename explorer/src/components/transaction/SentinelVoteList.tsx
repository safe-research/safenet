import type { Address } from "viem";
import { AnnotatedAddressList } from "@/components/common/AnnotatedAddressList";
import { shortAddress } from "@/lib/address";
import type { SentinelVote } from "@/lib/oracle";

const STATE_SUFFIXES: Record<SentinelVote["state"], string> = {
	committed: "⏳",
	approved: "✅",
	denied: "❌",
};

// Only sentinels who voted appear — no roster, so no "missing vote" rows (unlike
// `AnnotatedAddressList`'s accounts-vs-active framing, every account here is active).
// Renders nothing (not an empty-state message) when nobody has voted yet.
export function SentinelVoteList({ votes }: { votes: SentinelVote[] }) {
	if (votes.length === 0) return null;

	const sentinels = votes.map((vote) => vote.sentinel);
	const suffixBySentinel = new Map(votes.map((vote) => [vote.sentinel, STATE_SUFFIXES[vote.state]]));
	const reasonBySentinel = new Map(
		votes
			.filter((vote): vote is SentinelVote & { reason: string } => vote.state !== "committed")
			.map((vote) => [vote.sentinel, vote.reason]),
	);

	return (
		<div className="md:flex md:justify-between">
			<p className="ml-4">Votes:</p>
			<div>
				<AnnotatedAddressList
					accounts={sentinels}
					label={(address: Address) => `${shortAddress(address)} ${suffixBySentinel.get(address) ?? ""}`}
					popoverContent={(address: Address) => {
						const reason = reasonBySentinel.get(address);
						return reason ? <span className="text-muted">{reason}</span> : null;
					}}
				/>
			</div>
		</div>
	);
}
