#!/usr/bin/env python3
"""Build the CA3-FIX F-VAL-005 end-to-end script from the repo's
run_validator_reorg_nonce_test.sh (asserted edits only).

Usage: adapt_reorg_script.py <original.sh> <worktree> <out.sh>

Changes, all additive except two:
1. `source` the worktree's shared lib by absolute path (the script lives
   outside the worktree, so nothing untracked is left there).
2. Validator B's `max_reorg_depth` comes from `B_DEPTH` (default: the repo's
   3 * MAX_REORG_DEPTH).
3. Right before the reorg: snapshot the epoch-1 KeyGenCommitted events
   (every group id other than genesis) for validator A.
4. Replace the final `exit 0` with epoch-1 checks: A's epoch-1 commitment on
   the reorged chain must equal the one from before the reorg, the epoch-1
   group must be confirmed, and neither validator may log an own-commitment
   mismatch or a lost epoch.
"""
import pathlib
import sys

src = pathlib.Path(sys.argv[1]).read_text()
wt = sys.argv[2]
out = pathlib.Path(sys.argv[3])


def replace_once(text, old, new):
    assert text.count(old) == 1, f"expected one match for {old!r}"
    return text.replace(old, new)


src = replace_once(
    src,
    'SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"\nsource "$SCRIPT_DIR/lib/shared_test_scripts.sh"',
    f'source "{wt}/scripts/lib/shared_test_scripts.sh"',
)
src = replace_once(
    src,
    "VALIDATOR_B_MAX_REORG_DEPTH=$((3 * MAX_REORG_DEPTH))",
    "VALIDATOR_B_MAX_REORG_DEPTH=${B_DEPTH:-$((3 * MAX_REORG_DEPTH))}\necho \"==> CA3-FIX: validator B max_reorg_depth = $VALIDATOR_B_MAX_REORG_DEPTH\"",
)

before_reorg = '''
# CA3-FIX: epoch-1 commitments before the reorg (key generation for epoch 1
# starts on the genesis confirmation log, after the block the reorg goes back to).
EPOCH1_SIG='KeyGenCommitted(bytes32,address,((uint256,uint256),((uint256,uint256),uint256),(uint256,uint256)[],((uint256,uint256),uint256)),bool)'
epoch1_commit_of() { # $1 participant (no 0x), prints the group id and the commitment tuple (head words participant, offset and the `committed` flag skipped) of its last epoch-1 commitment
    fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" "$EPOCH1_SIG" |
        jq -r --arg gid "$GENESIS_GROUP" --arg addr "$1" \\
            '[.[] | select(.topics[1] != $gid) | select((.data[26:66] | ascii_downcase) == ($addr | ascii_downcase))][-1] | (.topics[1] // "none") + " " + (.data[194:] // "none")'
}
EPOCH1_A_BEFORE=$(epoch1_commit_of "${PARTICIPANTS[0]#0x}")
EPOCH1_B_BEFORE=$(epoch1_commit_of "${PARTICIPANTS[1]#0x}")
echo "==> CA3-FIX: epoch-1 commitment of A before the reorg: ${EPOCH1_A_BEFORE:0:90}..."
echo "==> CA3-FIX: epoch-1 commitment of B before the reorg: ${EPOCH1_B_BEFORE:0:90}..."
'''
src = replace_once(src, 'echo "==> Reorging $REORG_DEPTH block(s)', before_reorg + 'echo "==> Reorging $REORG_DEPTH block(s)')

tail_old = 'exit 0\n'
assert src.endswith(tail_old)
tail_new = '''echo "$EXIT_MESSAGE"
echo "==> CA3-FIX: epoch-1 checks"
EPOCH1_OK=1
DEADLINE=$((SECONDS + TIMEOUT))
EPOCH1_CONFIRMED=0
while [ "$SECONDS" -lt "$DEADLINE" ]; do
    EPOCH1_CONFIRMED=$(fetch_logs "$ANVIL_RPC_URL" "$COORDINATOR_ADDR" 'KeyGenConfirmed(bytes32,address,bool)' |
        jq --arg gid "$GENESIS_GROUP" --arg true_word "$TRUE_WORD" \\
            '[.[] | select(.topics[1] != $gid) | select(.data | endswith($true_word))] | length')
    [ "$EPOCH1_CONFIRMED" -gt 0 ] && break
    sleep "$BLOCK_TIME"
done
EPOCH1_A_AFTER=$(epoch1_commit_of "${PARTICIPANTS[0]#0x}")
EPOCH1_B_AFTER=$(epoch1_commit_of "${PARTICIPANTS[1]#0x}")
echo "    epoch-1 commitment of A after the reorg:  ${EPOCH1_A_AFTER:0:90}..."
echo "    epoch-1 commitment of B after the reorg:  ${EPOCH1_B_AFTER:0:90}..."
if [ "$EPOCH1_A_BEFORE" = "$EPOCH1_A_AFTER" ] && [ "${EPOCH1_A_BEFORE#none}" = "$EPOCH1_A_BEFORE" ]; then
    echo "    A: identical epoch-1 commitment before and after the reorg"
else
    echo "    A: epoch-1 commitment CHANGED or missing"; EPOCH1_OK=0
fi
if [ "$EPOCH1_B_BEFORE" = "$EPOCH1_B_AFTER" ]; then
    echo "    B: identical epoch-1 commitment before and after the reorg"
else
    echo "    B: epoch-1 commitment changed or missing (B was stopped across the reorg)"
fi
echo "    epoch-1 group confirmations (confirmed=true): $EPOCH1_CONFIRMED"
[ "$EPOCH1_CONFIRMED" -gt 0 ] || EPOCH1_OK=0
for who in a b; do
    BAD=$(grep -c "commitment is incorrect\\|IncorrectCommitment\\|failed to advance key generation\\|permanently halted" "$REPO_ROOT/validator_${who}_logs.txt" || true)
    echo "    validator $who: own-commitment mismatch / lost-epoch log lines = $BAD"
    [ "$BAD" -eq 0 ] || EPOCH1_OK=0
done
if [ "$EPOCH1_OK" -eq 1 ]; then
    EXIT_MESSAGE="CA3-FIX SUCCESS: epoch-1 key generation survived the reorg past its start (A's commitment unchanged, group confirmed, no mismatch logged). Repo assertion: $EXIT_MESSAGE"
    exit 0
fi
EXIT_MESSAGE="CA3-FIX FAILURE: epoch-1 key generation did not survive the reorg (see the checks above)."
exit 1
'''
src = src[: -len(tail_old)] + tail_new
out.write_text(src)
print(f"wrote {out}")
