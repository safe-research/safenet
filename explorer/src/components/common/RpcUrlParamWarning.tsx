import { useState } from "react";
import { Button } from "@/components/common/Button";
import { getRpcUrlParam } from "@/lib/settings";

export function RpcUrlParamWarning() {
	const rpcUrlParam = getRpcUrlParam();
	const [dismissed, setDismissed] = useState(false);

	if (!rpcUrlParam || dismissed) return null;

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
			<div className="bg-surface-1 border border-warning-outline rounded-card p-6 max-w-md mx-4 shadow-elevated">
				<h2 className="text-base font-semibold text-warning mb-2">Custom RPC Endpoint Detected</h2>
				<p className="text-sm text-sub-title mb-2">
					This page was opened with a custom RPC endpoint provided via URL parameter:
				</p>
				<p className="text-sm font-mono break-all text-title bg-surface-0 border border-surface-outline rounded-input px-3 py-2 mb-4">
					{rpcUrlParam}
				</p>
				<p className="text-sm text-sub-title mb-6">
					Only open links from sources you trust. A malicious RPC can return false data about transactions and balances.
				</p>
				<Button variant="primary" onClick={() => setDismissed(true)} className="w-full justify-center">
					I understand, continue
				</Button>
			</div>
		</div>
	);
}
