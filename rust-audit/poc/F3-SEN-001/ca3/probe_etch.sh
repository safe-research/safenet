#!/usr/bin/env bash
# Probe: etch the real Gnosis USDC.e into a local Anvil and measure an approve
# from a zero allowance under the sentinel's Direct-mode gas limit (55,000).
# No sentinel binary yet; this just confirms the etch and the OOG threshold.
set -euo pipefail
D="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RPC="http://127.0.0.1:31500"
PROXY=0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0
IMPL=0x107CF7fb73EA48D1D200989b156Ce1894d7AfEC7
# Anvil dev account 0 as the token holder/approver, and an arbitrary spender.
HOLDER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
HOLDER_PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
SPENDER=0x4F61B8832978e83b80D69551AEf07557DBE41d03

rpc() { cast rpc --rpc-url "$RPC" "$@" >/dev/null; }

echo "== etch proxy + impl runtime code"
cast rpc --rpc-url "$RPC" anvil_setCode "$PROXY" "$(cat "$D/proxy.code")" >/dev/null
cast rpc --rpc-url "$RPC" anvil_setCode "$IMPL" "$(cat "$D/impl.code")" >/dev/null

echo "== write the 14 cached proxy storage slots"
while read -r slot val; do
	cast rpc --rpc-url "$RPC" anvil_setStorageAt "$PROXY" "$slot" "$val" >/dev/null
done <"$D/slots.txt"

echo "== fund HOLDER's USDC.e balance via the balances mapping (slot 9, FiatTokenV2_2)"
BAL_SLOT=$(cast index address "$HOLDER" 9)
cast rpc --rpc-url "$RPC" anvil_setStorageAt "$PROXY" "$BAL_SLOT" \
	0x000000000000000000000000000000000000000000000000000000e8d4a51000 >/dev/null
echo "  balanceOf(HOLDER) = $(cast call --rpc-url "$RPC" "$PROXY" 'balanceOf(address)(uint256)' "$HOLDER")"

echo "== allowance(HOLDER, SPENDER) before = $(cast call --rpc-url "$RPC" "$PROXY" 'allowance(address,address)(uint256)' "$HOLDER" "$SPENDER")"

echo "== approve(SPENDER, 800000000) with gas limit 55000 (Direct-mode sentinel limit)"
set +e
cast send --rpc-url "$RPC" --private-key "$HOLDER_PK" --gas-limit 55000 --json \
	"$PROXY" 'approve(address,uint256)' "$SPENDER" 800000000 >"$D/probe_approve_55000.json" 2>"$D/probe_approve_55000.err"
rc=$?
set -e
echo "  cast rc=$rc"
echo "  status : $(jq -r '.status' "$D/probe_approve_55000.json" 2>/dev/null)"
echo "  gasUsed: $(jq -r '.gasUsed' "$D/probe_approve_55000.json" 2>/dev/null) (hex)"
echo "  allowance after = $(cast call --rpc-url "$RPC" "$PROXY" 'allowance(address,address)(uint256)' "$HOLDER" "$SPENDER")"

echo "== approve with gas limit 80000 (raised, bite-check value)"
cast send --rpc-url "$RPC" --private-key "$HOLDER_PK" --gas-limit 80000 --json \
	"$PROXY" 'approve(address,uint256)' "$SPENDER" 800000000 >"$D/probe_approve_80000.json"
echo "  status : $(jq -r '.status' "$D/probe_approve_80000.json")"
echo "  gasUsed: $(jq -r '.gasUsed' "$D/probe_approve_80000.json") (hex)"
echo "  allowance after = $(cast call --rpc-url "$RPC" "$PROXY" 'allowance(address,address)(uint256)' "$HOLDER" "$SPENDER")"
