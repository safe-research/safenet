import type { Address } from "viem";
import { InlineAddress } from "@/components/common/InlineAddress";
import { useConsensusState } from "@/hooks/useConsensusState";
import { useOracleArbitrator } from "@/hooks/useOracleArbitrator";
import type { Arbitration } from "@/lib/consensus";
import { InlineBlockInfo, InlineExplorerTxLink } from "../common/Info";

function outcomeLabel(outcome: NonNullable<Arbitration["outcome"]>): string {
	switch (outcome.kind) {
		case "ruled":
			return outcome.secure ? "Ruled secure" : "Ruled insecure";
		case "outOfScope":
			return "Out of scope";
		case "timedOut":
			return "Timed out without a ruling";
	}
}

export function SafeTxArbitration({ oracle, arbitration }: { oracle: Address; arbitration: Arbitration }) {
	const { data: consensusState } = useConsensusState();
	const arbitrator = useOracleArbitrator(oracle, true);
	const { outcome } = arbitration;
	const deadlinePassed = outcome === null && consensusState.currentBlock > arbitration.deadline;
	// The Council's reason is shown verbatim as text: it may be free text or an IPFS CID.
	const reason = outcome !== null && outcome.kind !== "timedOut" ? outcome.context : "";

	return (
		<div className="space-y-2">
			<p>Arbitration:</p>
			<div className="md:flex md:justify-between">
				<p className="ml-4 mr-2">Started:</p>
				<p>
					<InlineBlockInfo block={arbitration.triggeredAt.block} />{" "}
					<InlineExplorerTxLink txHash={arbitration.triggeredAt.tx}>Explorer Tx</InlineExplorerTxLink>
				</p>
			</div>
			<div className="md:flex md:justify-between">
				<p className="ml-4 mr-2">Deadline:</p>
				<p>
					<span className="font-mono">Block {arbitration.deadline}</span>
					{deadlinePassed && " (passed)"}
				</p>
			</div>
			<div className="md:flex md:justify-between">
				<p className="ml-4 mr-2">Arbitrator:</p>
				<p>
					{arbitrator.data === undefined ? (
						"-"
					) : (
						<InlineAddress chainId={consensusState.chainId} address={arbitrator.data} />
					)}
				</p>
			</div>
			<div className="md:flex md:justify-between">
				<p className="ml-4 mr-2">Outcome:</p>
				<p>
					{outcome === null ? (
						"Pending"
					) : (
						<>
							{outcomeLabel(outcome)}, <InlineBlockInfo block={outcome.at.block} />{" "}
							<InlineExplorerTxLink txHash={outcome.at.tx}>Explorer Tx</InlineExplorerTxLink>
						</>
					)}
				</p>
			</div>
			{reason !== "" && (
				<div className="md:flex md:justify-between">
					<p className="ml-4 mr-2">Reason:</p>
					<p className="break-all">{reason}</p>
				</div>
			)}
			<p className="ml-4 text-muted">
				Transactions that enter arbitration are never attested, whatever the ruling. To execute this one, propose it
				again in a later epoch or use the escape hatch.
			</p>
		</div>
	);
}
