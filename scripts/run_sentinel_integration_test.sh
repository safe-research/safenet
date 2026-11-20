#!/bin/bash
# Interop/integration test for the Rust sentinel (epic Phase F1, adapted for
# commit-reveal): runs two independent Rust sentinel instances side by side
# against the same dispute on Anvil, and asserts they agree (no arbitration)
# and settle fees/bonds correctly. Unlike scripts/run_validator_integration_test.sh,
# this does not require a full validator/FROST genesis: a `TestConsensus` contract
# stands in for `Consensus`, only emitting the `TransactionProposed`
# event the sentinels need.
#
# Sentinel A is a plain account. Sentinel B is a Safe whose key only holds a
# Zodiac Roles role scoped to the sentinel's calls (see the sentinel handbook's
# "Running as a Safe"): the test checks that the role denies everything else,
# that the sentinel refuses to start on a broken Roles setup, and finally
# measures the gas the Roles path costs.
set -eo pipefail
# Job control, so each `&`-backgrounded command below gets its own process
# group (its PID doubling as its PGID) instead of sharing this script's.
# Cleanup then kills each job's *group* (`kill -- -$pid`), including any
# subprocesses it spawns, without touching whatever else happens to share this
# script's own process group (e.g. a CI runner's step wrapper) the way `kill 0`
# would.
set -m

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# --- Configuration ---
RPC_URL="http://127.0.0.1:8545"
SENTINEL_A_ENGINE_ADDR="127.0.0.1:5473"
SENTINEL_B_ENGINE_ADDR="127.0.0.1:5474"
CHAIN_ID=31337
BLOCK_TIME_SECONDS=1
REQUEST_FEE=1000
BOND_MULTIPLIER=4
COMMIT_WINDOW=5
REVEAL_WINDOW=5
GOVERNANCE_DELAY=0
# Deliberately less than BOND_MULTIPLIER, so the dispute scenario below
# exercises a *partial* bond slash rather than the full bond.
INITIAL_SLASHING_MULTIPLIER=2
INITIAL_DAO_FEE_SHARE=0
CHARTER_ENS="safenet-charter.safe.eth"
# Short enough for the unruled-dispute scenario below to wait it out, long
# enough that the ruled dispute before it is arbitrated well within it.
ARBITRATION_TIMEOUT=20
FUNDING_ETH=1ether
FUNDING_TOKEN=1000000
# The fee token is the production one, Gnosis Chain's USDC.e: Circle's
# `FiatTokenProxy` in front of a `FiatTokenV2_2` implementation, installed at
# their production addresses (see step 3), so the sentinels pay the same gas
# for approvals and bond transfers as they do onchain.
FEE_TOKEN=0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0
FEE_TOKEN_IMPLEMENTATION=0x107CF7fb73EA48D1D200989b156Ce1894d7AfEC7
# Anvil account 0 — deployer, fee token master minter, and SentinelOracle
# arbitrator.
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
DEPLOYER_PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
TX_SAFE=0x1111111111111111111111111111111111111111
TX_RECIPIENT=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
ZERO_ADDRESS=0x0000000000000000000000000000000000000000
# Sentinel B's Safe and Roles modifier: Safe v1.5.0 and Zodiac Roles v2.1.1,
# installed at their Gnosis Chain addresses from their production bytecode
# (see step 3).
SAFE_SINGLETON=0xFf51A5898e281Db6DfC7855790607438dF2ca44b
SAFE_PROXY_FACTORY=0x14F2982D601c9458F93bd70B218933A6f8165e7b
ROLES_MASTERCOPY=0xF2964CE6161ce0e75964Fe7927cE114cb0B283D5
ROLES_INTEGRITY=0x6a6Af4b16458Bc39817e4019fB02BD3b26d41049
ROLES_PACKER=0x869718C939652084BC491fBC5ce0D3C1d5B309F0
SINGLETON_FACTORY=0xce0042B868300000d44A59004Da54A005ffdcf9f
SENTINEL_ROLE_KEY=$(cast keccak "SENTINEL")
# The sentinel's default `[sentinel.roles] gas_overhead`, which sentinel B
# runs with.
ROLES_GAS_OVERHEAD=150000
# Where step 18 reads the current Gnosis Chain gas price to price the Roles
# path in USD (xDAI is pegged to the dollar). Optional: the step skips the
# USD figures if it can't be reached.
GNOSIS_RPC_URL="${GNOSIS_RPC_URL:-https://rpc.gnosischain.com}"

# Register cleanup before anything, to make sure we don't leave anything running
# in the background in case of an error.
PIDS=()
cleanup() {
	echo "Stopping background processes (${PIDS[*]})..."
	for pid in "${PIDS[@]}"; do
		# Negative PID targets the whole process group `set -m` gave this job,
		# so any subprocesses are reaped too rather than left orphaned holding
		# a port.
		kill -- "-$pid" >/dev/null 2>&1 || true
	done
	rm -f "$SENTINEL_A_CONFIG" "$SENTINEL_B_CONFIG" \
		"$SENTINEL_A_ENGINE_CONFIG" "$SENTINEL_B_ENGINE_CONFIG" \
		"$BROKEN_SENTINEL_CONFIG" "$GAS_MEASUREMENTS"
}
trap cleanup EXIT

# --- 1. Build the Rust sentinel and sentinel engine ---
# Built up front, before Anvil or anything else starts, so a compile error
# fails fast and no compile time is wasted while other test infrastructure
# sits idle in the background. The resulting binaries are invoked directly
# below so Cargo cannot rebuild or contend for its build lock during the test.
echo "Building the Rust sentinel and sentinel engine..."
cargo build --package sentinel --package sentinel-engine

# --- 2. Start Anvil with a 1-second block interval ---
echo "Starting Anvil..."
anvil --block-time "$BLOCK_TIME_SECONDS" > "$ROOT/anvil_logs.txt" 2>&1 &
PIDS+=("$!")
sleep 2

# --- 3. Deploy contracts ---
echo "Installing the USDC.e fee token..."
cast rpc --rpc-url "$RPC_URL" anvil_setCode "$FEE_TOKEN" "$(cat "$ROOT/scripts/bytecode/USDCProxy.hex")" >/dev/null
cast rpc --rpc-url "$RPC_URL" anvil_setCode "$FEE_TOKEN_IMPLEMENTATION" "$(cat "$ROOT/scripts/bytecode/USDC.hex")" >/dev/null
# `setCode` leaves the proxy with empty storage, so seed the slots the
# production deployment's `initialize*` calls would have written, mirroring
# their values onchain except for the master minter, which is the deployer
# here so it can mint the fee tokens funded in step 4.
set_fee_token_storage() {
	cast rpc --rpc-url "$RPC_URL" anvil_setStorageAt "$FEE_TOKEN" "$1" "$2" >/dev/null
}
# `FiatTokenProxy` admin and implementation (ZeppelinOS unstructured storage).
set_fee_token_storage 0x10d6a54a4754c8869d6886b5f5d7fbfa5b4522237ea5c60d11bc4e7a1ff9390b \
	0x0000000000000000000000007a48dac683da91e4faa5ab13d91ab5fd170875bd
set_fee_token_storage 0x7050c9e0f4ca769c69bd3a8ef740bc37934f8e2c036e5a723fd8ee048ed3f8c3 \
	"$(cast to-uint256 "$FEE_TOKEN_IMPLEMENTATION")"
# `FiatTokenV2_2` state: owner, pauser (unpaused), blacklister, name, symbol,
# decimals, currency, master minter (initialized), and initialized version.
set_fee_token_storage 0 0x000000000000000000000000458cd345b4c05e8df39d0a07220feb4ec19f5e6f
set_fee_token_storage 1 0x000000000000000000000000458cd345b4c05e8df39d0a07220feb4ec19f5e6f
set_fee_token_storage 2 0x000000000000000000000000458cd345b4c05e8df39d0a07220feb4ec19f5e6f
set_fee_token_storage 4 0x4272696467656420555344432028476e6f73697329000000000000000000002a
set_fee_token_storage 5 0x555344432e65000000000000000000000000000000000000000000000000000c
set_fee_token_storage 6 0x0000000000000000000000000000000000000000000000000000000000000006
set_fee_token_storage 7 0x5553440000000000000000000000000000000000000000000000000000000006
set_fee_token_storage 8 "0x000000000000000000000001${DEPLOYER:2}"
set_fee_token_storage 18 0x0000000000000000000000000000000000000000000000000000000000000003
echo "Fee token installed at $FEE_TOKEN ($(cast call --rpc-url "$RPC_URL" "$FEE_TOKEN" "symbol()(string)"))"

echo "Deploying test consensus..."
forge script --root "$ROOT/contracts" DeployTestConsensusScript --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --broadcast
CONSENSUS=$(jq -r '.returns.consensus.value' "$ROOT/contracts/build/broadcast/DeployTestConsensus.s.sol/$CHAIN_ID/run-latest.json")
echo "Test consensus deployed at $CONSENSUS"

echo "Deploying sentinel oracle..."
env \
	FACTORY=2 \
	SENTINEL_ARBITRATOR="$DEPLOYER" \
	SENTINEL_GOVERNANCE="$DEPLOYER" \
	SENTINEL_PROTOCOL_FUNDS_RECEIVER="$DEPLOYER" \
	SENTINEL_CONSENSUS="$CONSENSUS" \
	SENTINEL_FEE_TOKEN="$FEE_TOKEN" \
	SENTINEL_REQUEST_FEE="$REQUEST_FEE" \
	SENTINEL_COMMIT_WINDOW="$COMMIT_WINDOW" \
	SENTINEL_REVEAL_WINDOW="$REVEAL_WINDOW" \
	SENTINEL_GOVERNANCE_DELAY="$GOVERNANCE_DELAY" \
	SENTINEL_BOND_MULTIPLIER="$BOND_MULTIPLIER" \
	SENTINEL_INITIAL_SLASHING_MULTIPLIER="$INITIAL_SLASHING_MULTIPLIER" \
	SENTINEL_INITIAL_DAO_FEE_SHARE="$INITIAL_DAO_FEE_SHARE" \
	SENTINEL_CHARTER_ENS="$CHARTER_ENS" \
	SENTINEL_ARBITRATION_TIMEOUT="$ARBITRATION_TIMEOUT" \
	forge script --root "$ROOT/contracts" DeploySentinelOracleScript --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --broadcast
ORACLE=$(jq -r '.returns.sentinelOracle.value' "$ROOT/contracts/build/broadcast/DeploySentinelOracle.s.sol/$CHAIN_ID/run-latest.json")
echo "Sentinel oracle deployed at $ORACLE"

echo "Installing Safe v1.5.0 and Zodiac Roles v2.1.1..."
install_code() {
	cast rpc --rpc-url "$RPC_URL" anvil_setCode "$1" "$(cat "$ROOT/scripts/bytecode/$2.hex")" >/dev/null
}
install_code "$SAFE_SINGLETON" Safe
install_code "$SAFE_PROXY_FACTORY" SafeProxyFactory
# When a function is scoped, the mastercopy checks and packs its conditions
# with its `Integrity` and `Packer` libraries, and stores them as a contract
# deployed through the EIP-2470 singleton factory.
install_code "$ROLES_MASTERCOPY" ZodiacRoles
install_code "$ROLES_INTEGRITY" ZodiacRolesIntegrity
install_code "$ROLES_PACKER" ZodiacRolesPacker
install_code "$SINGLETON_FACTORY" SingletonFactory

# --- 4. Fund the required accounts ---
echo "Generating sentinel and sponsor accounts..."
WALLETS=$(cast wallet new --json --number 3)
SENTINEL_A_ADDR=$(echo "$WALLETS" | jq -r '.[0].address')
SENTINEL_A_PK=$(echo "$WALLETS" | jq -r '.[0].private_key')
# Sentinel B's key only signs; the sentinel itself is the Safe deployed below.
SENTINEL_B_KEY_ADDR=$(echo "$WALLETS" | jq -r '.[1].address')
SENTINEL_B_PK=$(echo "$WALLETS" | jq -r '.[1].private_key')
SPONSOR_ADDR=$(echo "$WALLETS" | jq -r '.[2].address')
SPONSOR_PK=$(echo "$WALLETS" | jq -r '.[2].private_key')

echo "Deploying sentinel B's Safe, owned by the deployer..."
SAFE_SETUP=$(cast calldata "setup(address[],uint256,address,bytes,address,address,uint256,address)" \
	"[$DEPLOYER]" 1 "$ZERO_ADDRESS" 0x "$ZERO_ADDRESS" "$ZERO_ADDRESS" 0 "$ZERO_ADDRESS")
SENTINEL_B_ADDR=$(cast call --rpc-url "$RPC_URL" --from "$DEPLOYER" "$SAFE_PROXY_FACTORY" \
	"createProxyWithNonce(address,bytes,uint256)(address)" "$SAFE_SINGLETON" "$SAFE_SETUP" 0)
cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$SAFE_PROXY_FACTORY" \
	"createProxyWithNonce(address,bytes,uint256)" "$SAFE_SINGLETON" "$SAFE_SETUP" 0 >/dev/null

# Executes a call as the Safe, signed by its only owner as the sender itself
# (an "approved hash" signature: r = owner, s = 0, v = 1).
safe_exec() {
	local signature="0x000000000000000000000000${DEPLOYER:2}0000000000000000000000000000000000000000000000000000000000000000""01"
	cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$SENTINEL_B_ADDR" \
		"execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)" \
		"$1" 0 "$2" 0 0 0 0 "$ZERO_ADDRESS" "$ZERO_ADDRESS" "$signature" >/dev/null
}

echo "Deploying a Roles modifier for the Safe and enabling it as a module..."
# An EIP-1167 minimal proxy to the mastercopy, as Zodiac's module factory
# deploys, set up with the Safe as avatar and target. The deployer owns it, so
# it can scope the role directly; production setups have the Safe own it.
ROLES_MODULE=$(cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --json --create \
	"0x3d602d80600a3d3981f3363d3d373d3d3d363d73${ROLES_MASTERCOPY:2}5af43d82803e903d91602b57fd5bf3" |
	jq -r '.contractAddress')
roles_owner() {
	cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$ROLES_MODULE" "$@" >/dev/null
}
roles_owner "setUp(bytes)" \
	"$(cast abi-encode "f(address,address,address)" "$DEPLOYER" "$SENTINEL_B_ADDR" "$SENTINEL_B_ADDR")"
safe_exec "$SENTINEL_B_ADDR" "$(cast calldata "enableModule(address)" "$ROLES_MODULE")"

echo "Scoping the SENTINEL role to the sentinel's calls and granting it to sentinel B's key..."
# Each request's bond is its fee times the bond multiplier.
SENTINEL_BOND=$((REQUEST_FEE * BOND_MULTIPLIER))
# `approve(spender, amount)` with spender == oracle and amount <= one bond:
# conditions (parent, ParameterType, Operator, compValue) with Calldata = 5,
# Static = 1, Matches = 5, EqualTo = 16 and LessThan = 18.
APPROVE_CONDITIONS="[(0,5,5,0x),(0,1,16,$(cast abi-encode "f(address)" "$ORACLE")),(0,1,18,$(cast abi-encode "f(uint256)" $((SENTINEL_BOND + 1))))]"
roles_owner "scopeTarget(bytes32,address)" "$SENTINEL_ROLE_KEY" "$FEE_TOKEN"
roles_owner "scopeFunction(bytes32,address,bytes4,(uint8,uint8,uint8,bytes)[],uint8)" \
	"$SENTINEL_ROLE_KEY" "$FEE_TOKEN" "$(cast sig "approve(address,uint256)")" "$APPROVE_CONDITIONS" 0
roles_owner "scopeTarget(bytes32,address)" "$SENTINEL_ROLE_KEY" "$ORACLE"
for function in "commit(bytes32,bytes32)" "reveal(bytes32,bool,bytes32,string)" \
	"finalize(bytes32)" "claim(bytes32)" "timeoutArbitration(bytes32)"; do
	roles_owner "allowFunction(bytes32,address,bytes4,uint8)" \
		"$SENTINEL_ROLE_KEY" "$ORACLE" "$(cast sig "$function")" 0
done
roles_owner "assignRoles(address,bytes32[],bool[])" "$SENTINEL_B_KEY_ADDR" "[$SENTINEL_ROLE_KEY]" "[true]"

echo "Sentinel A:     $SENTINEL_A_ADDR"
echo "Sentinel B:     $SENTINEL_B_ADDR (Safe, through Roles modifier $ROLES_MODULE)"
echo "Sentinel B key: $SENTINEL_B_KEY_ADDR"
echo "Sponsor:        $SPONSOR_ADDR"

# Sentinel B's key pays its gas; its Safe holds its bonds.
echo "Funding accounts with ETH and the fee token..."
cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" \
	"$FEE_TOKEN" "configureMinter(address,uint256)" "$DEPLOYER" "$((FUNDING_TOKEN * 3))" >/dev/null
for addr in "$SENTINEL_A_ADDR" "$SENTINEL_B_KEY_ADDR" "$SPONSOR_ADDR"; do
	cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" --value "$FUNDING_ETH" "$addr" >/dev/null
done
for addr in "$SENTINEL_A_ADDR" "$SENTINEL_B_ADDR" "$SPONSOR_ADDR"; do
	cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" \
		"$FEE_TOKEN" "mint(address,uint256)" "$addr" "$FUNDING_TOKEN" >/dev/null
done

# --- 4b. Check what a stolen sentinel B key can make the Safe do ---
# Simulated from the key, as `execTransactionWithRole` calls: only the bond
# approval goes through, so the key can't move the Safe's funds or take it
# over.
role_call() {
	cast call --rpc-url "$RPC_URL" --from "$SENTINEL_B_KEY_ADDR" "$ROLES_MODULE" \
		"execTransactionWithRole(address,uint256,bytes,uint8,bytes32,bool)(bool)" \
		"$1" 0 "$2" 0 "$SENTINEL_ROLE_KEY" true >/dev/null 2>&1
}
if ! role_call "$FEE_TOKEN" "$(cast calldata "approve(address,uint256)" "$ORACLE" "$SENTINEL_BOND")"; then
	echo "FAILED: the SENTINEL role doesn't let sentinel B's key approve one bond to the oracle"
	exit 1
fi
expect_role_denies() {
	if role_call "$2" "$3"; then
		echo "FAILED: the SENTINEL role lets sentinel B's key $1"
		exit 1
	fi
}
expect_role_denies "transfer the Safe's fee tokens" \
	"$FEE_TOKEN" "$(cast calldata "transfer(address,uint256)" "$SENTINEL_B_KEY_ADDR" 1)"
expect_role_denies "approve the fee token to another spender" \
	"$FEE_TOKEN" "$(cast calldata "approve(address,uint256)" "$SENTINEL_B_KEY_ADDR" 1)"
expect_role_denies "approve more than one bond to the oracle" \
	"$FEE_TOKEN" "$(cast calldata "approve(address,uint256)" "$ORACLE" $((SENTINEL_BOND + 1)))"
expect_role_denies "add itself as a Safe owner" \
	"$SENTINEL_B_ADDR" "$(cast calldata "addOwnerWithThreshold(address,uint256)" "$SENTINEL_B_KEY_ADDR" 1)"
expect_role_denies "disable the Roles module" \
	"$SENTINEL_B_ADDR" "$(cast calldata "disableModule(address,address)" 0x0000000000000000000000000000000000000001 "$ROLES_MODULE")"
echo "OK: sentinel B's key can only make the Safe send the sentinel's calls."

echo "Deploying transaction token..."
env FACTORY=2 \
	forge script --root "$ROOT/contracts" DeployERC20Script --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" --broadcast
TX_TOKEN=$(jq -r '.returns.erc20.value' "$ROOT/contracts/build/broadcast/DeployERC20.s.sol/$CHAIN_ID/run-latest.json")
echo "Transaction token deployed at $TX_TOKEN"

# Seed a genuine prior interaction from the proposed Safe to the recipient so
# sentinel A's address-poisoning checker approves the disputed transfer.
cast send --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" \
	"$TX_TOKEN" "mint(address,uint256)" "$TX_SAFE" 2 >/dev/null
cast rpc --rpc-url "$RPC_URL" anvil_setBalance "$TX_SAFE" 0xde0b6b3a7640000 >/dev/null
cast rpc --rpc-url "$RPC_URL" anvil_impersonateAccount "$TX_SAFE" >/dev/null
cast send --rpc-url "$RPC_URL" --unlocked --from "$TX_SAFE" \
	"$TX_TOKEN" "transfer(address,uint256)" "$TX_RECIPIENT" 1 >/dev/null
cast rpc --rpc-url "$RPC_URL" anvil_stopImpersonatingAccount "$TX_SAFE" >/dev/null

echo "Approving the oracle to pull the request fee from the sponsor..."
cast send --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" \
	"$FEE_TOKEN" "approve(address,uint256)" "$ORACLE" "$REQUEST_FEE" >/dev/null

# Balances just before either sentinel commits a bond, to measure the fee/bond
# flow against once the dispute resolves. `--json` avoids `cast call`'s
# human-readable output appending a "[1e6]"-style scientific-notation hint,
# which breaks bash arithmetic.
balance_of() {
	cast call --rpc-url "$RPC_URL" --json "$FEE_TOKEN" "balanceOf(address)(uint256)" "$1" | jq -r '.[0]'
}
SENTINEL_A_BALANCE_BEFORE=$(balance_of "$SENTINEL_A_ADDR")
SENTINEL_B_BALANCE_BEFORE=$(balance_of "$SENTINEL_B_ADDR")

# `getRequest` returns two separate tuples, field-for-field with
# `SentinelOracleRequest.Terms` (`commitDeadline, daoFeeShare, revealDeadline,
# bondTarget, sponsor, slashAmount`) and `SentinelOracleRequest.Progress`
# (`state, fee, arbitrationDeadline, committedCount, revealedCount,
# approveSentinelCount, denySentinelCount`) — every field must be listed, in its
# exact declared width and in this order, or `cast` silently misaligns/misdecodes
# the rest.
get_request() {
	cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
		"getRequest(bytes32)((uint64,uint24,uint64,uint96,address,uint96,uint8),(uint8,uint96,uint64,uint16,uint16,uint16,uint16,uint24))" \
		"$1" 2>/dev/null
}
REQUEST_PROGRESS_INDEX=1
REQUEST_STATE_INDEX=0
REQUEST_DENY_SENTINEL_COUNT_INDEX=6

# `SentinelOracleRequest.State` ordinals (`NONE, PENDING, FROZEN, RESOLVED_APPROVED,
# RESOLVED_DENIED, TIMED_OUT`) — `NONE` is a zero-value sentinel meaning "never created", never a
# real state a live request reports.
STATE_FROZEN=2
STATE_RESOLVED_APPROVED=3
STATE_TIMED_OUT=5
REQUEST_ARBITRATION_DEADLINE_INDEX=2

# --- 5. Spin up both Rust sentinels with their engines ---
# Already built in step 1, so this just runs it — twice, once per account,
# each with its own config, engine, and in-memory database.
sentinel_engine_config() {
	local bind_address=$1
	local blocklist=$2
	cat <<EOF
rpc = "$RPC_URL"
bind_address = "$bind_address"

[engine]
blocklist = $blocklist
address_poisoning_lookback_blocks = 1000
EOF
}
# An optional third and fourth argument run the sentinel as a Safe, through
# that Roles modifier and role.
sentinel_config() {
	local signer=$1
	local engine=$2
	local roles=""
	if [ -n "${3:-}" ]; then
		roles="
[sentinel.roles]
module = \"$3\"
role_key = \"$4\""
	fi
	cat <<EOF
rpc = "$RPC_URL"
signer = "$signer"
database = "sqlite::memory:"
oracle = "$ORACLE"
consensus = "$CONSENSUS"

[sentinel]
fee_token = "$FEE_TOKEN"
voting_window = $((COMMIT_WINDOW + REVEAL_WINDOW))
claim_window = 10
engine = "http://$engine"
$roles

[index]
block_time = $((BLOCK_TIME_SECONDS * 1000))
# Anvil mines blocks locally, so there is no propagation to wait for.
block_propagation_delay = 0
block_retry_delays = []
EOF
}

SENTINEL_A_ENGINE_CONFIG=$(mktemp)
sentinel_engine_config "$SENTINEL_A_ENGINE_ADDR" "[]" >"$SENTINEL_A_ENGINE_CONFIG"
SENTINEL_A_CONFIG=$(mktemp)
sentinel_config "$SENTINEL_A_PK" "$SENTINEL_A_ENGINE_ADDR" >"$SENTINEL_A_CONFIG"

# Sentinel B's engine alone blocklists $TX_TOKEN, so it denies a request
# proposing a call to it while Sentinel A approves — the genuine dispute
# exercised in step 10 below.
SENTINEL_B_ENGINE_CONFIG=$(mktemp)
sentinel_engine_config "$SENTINEL_B_ENGINE_ADDR" "[\"$TX_TOKEN\"]" >"$SENTINEL_B_ENGINE_CONFIG"
SENTINEL_B_CONFIG=$(mktemp)
sentinel_config "$SENTINEL_B_PK" "$SENTINEL_B_ENGINE_ADDR" "$ROLES_MODULE" "$SENTINEL_ROLE_KEY" >"$SENTINEL_B_CONFIG"

# Starts a sentinel with config `$2`, expecting it to refuse to start with the
# error `$3` instead of running with a Roles setup that can't work.
BROKEN_SENTINEL_CONFIG=$(mktemp)
expect_startup_failure() {
	local log="$ROOT/sentinel_startup_failure_logs.txt"
	"$ROOT/target/debug/sentinel" --config-file "$2" >"$log" 2>&1 &
	local pid=$!
	PIDS+=("$pid")
	for _ in $(seq 1 20); do
		kill -0 "$pid" 2>/dev/null || break
		sleep 1
	done
	if kill -0 "$pid" 2>/dev/null; then
		echo "FAILED: the sentinel started $1"
		exit 1
	fi
	if wait "$pid" || ! grep -q "$3" "$log"; then
		echo "FAILED: expected the sentinel to refuse to start $1 with $3:"
		cat "$log"
		exit 1
	fi
	echo "OK: the sentinel refused to start $1."
}

expect_startup_failure "as a Safe that is not a registered sentinel" "$SENTINEL_B_CONFIG" "NotRegistered"

echo "Registering both sentinels..."
cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$ORACLE" "addSentinel(address)" "$SENTINEL_A_ADDR" >/dev/null
cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$ORACLE" "addSentinel(address)" "$SENTINEL_B_ADDR" >/dev/null

sentinel_config "$SENTINEL_B_PK" "$SENTINEL_B_ENGINE_ADDR" "$ROLES_MODULE" "$(cast keccak "NOT_A_ROLE")" >"$BROKEN_SENTINEL_CONFIG"
expect_startup_failure "with a role its key isn't a member of" "$BROKEN_SENTINEL_CONFIG" "RoleDenied"
sentinel_config "$SENTINEL_B_PK" "$SENTINEL_B_ENGINE_ADDR" "$SENTINEL_B_KEY_ADDR" "$SENTINEL_ROLE_KEY" >"$BROKEN_SENTINEL_CONFIG"
expect_startup_failure "with a Roles modifier address that has no code" "$BROKEN_SENTINEL_CONFIG" "NoModule"

echo "Starting sentinel engine A..."
"$ROOT/target/debug/sentinel-engine" --config-file "$SENTINEL_A_ENGINE_CONFIG" >"$ROOT/sentinel_engine_a_logs.txt" 2>&1 &
PIDS+=("$!")

echo "Starting sentinel A..."
"$ROOT/target/debug/sentinel" --config-file "$SENTINEL_A_CONFIG" >"$ROOT/sentinel_a_logs.txt" 2>&1 &
PIDS+=("$!")

echo "Starting sentinel engine B..."
"$ROOT/target/debug/sentinel-engine" --config-file "$SENTINEL_B_ENGINE_CONFIG" >"$ROOT/sentinel_engine_b_logs.txt" 2>&1 &
PIDS+=("$!")

echo "Starting sentinel B..."
"$ROOT/target/debug/sentinel" --config-file "$SENTINEL_B_CONFIG" >"$ROOT/sentinel_b_logs.txt" 2>&1 &
PIDS+=("$!")

# Give both sentinels time to connect and start watching before the dispute
# exists. Neither watcher replays history, so proposing too early makes a
# sentinel miss the block entirely.
sleep 3

# --- 6. Propose a transaction ---
echo "Proposing an oracle-checked transaction..."
env \
	CONSENSUS_ADDRESS="$CONSENSUS" \
	ORACLE_ADDRESS="$ORACLE" \
	TX_CHAIN_ID="$CHAIN_ID" \
	TX_SAFE="$TX_SAFE" \
	TX_TO="$TX_SAFE" \
	TX_NONCE=0 \
	forge script --root "$ROOT/contracts" ProposeTransactionScript --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" --broadcast

REQUEST_ID=$(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$ORACLE" \
	'NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)' | jq -r '.[0].topics[1]')
echo "Request id: $REQUEST_ID"

# --- 7. Wait for 10 blocks ---
START_BLOCK=$(cast block-number --rpc-url "$RPC_URL")
TARGET_BLOCK=$((START_BLOCK + 10))
echo "Waiting for block $TARGET_BLOCK (currently $START_BLOCK)..."
TIMEOUT_SECONDS=30
ELAPSED_SECONDS=0
while [ "$(cast block-number --rpc-url "$RPC_URL")" -lt "$TARGET_BLOCK" ]; do
	if [ "$ELAPSED_SECONDS" -ge "$TIMEOUT_SECONDS" ]; then
		echo "FAILED: timed out waiting for block $TARGET_BLOCK; is Anvil still mining?"
		exit 1
	fi
	sleep "$BLOCK_TIME_SECONDS"
	ELAPSED_SECONDS=$((ELAPSED_SECONDS + BLOCK_TIME_SECONDS))
done

# --- 8. Check the final vote ---
# A short grace period on top of the 10-block wait for the last finalize/claim
# transactions to land, in case they landed late in the window above. `|| true`
# on each `cast` failure keeps the loop retrying instead of `set -e` aborting
# the script on a transient RPC hiccup.
REQUEST=""
for _ in $(seq 1 10); do
	REQUEST=$(get_request "$REQUEST_ID") || true
	STATE=$(echo "$REQUEST" | jq -r ".[$REQUEST_PROGRESS_INDEX][$REQUEST_STATE_INDEX]" 2>/dev/null) || true
	[ "$STATE" = "$STATE_RESOLVED_APPROVED" ] && break
	sleep "$BLOCK_TIME_SECONDS"
done

echo "Final request state: $REQUEST"
if [ "$STATE" != "$STATE_RESOLVED_APPROVED" ]; then
	echo "FAILED: expected state RESOLVED_APPROVED ($STATE_RESOLVED_APPROVED), got $STATE"
	exit 1
fi
DENY_SENTINEL_COUNT=$(echo "$REQUEST" | jq -r ".[$REQUEST_PROGRESS_INDEX][$REQUEST_DENY_SENTINEL_COUNT_INDEX]")
if [ "$DENY_SENTINEL_COUNT" != "0" ]; then
	echo "FAILED: expected a unanimous approve vote, but denySentinelCount is $DENY_SENTINEL_COUNT"
	exit 1
fi
DISPUTES=$(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$ORACLE" \
	'DisputeResolved(bytes32,uint8,uint128,string)' | jq 'length')
if [ "$DISPUTES" != "0" ]; then
	echo "FAILED: expected no arbitration, but $DISPUTES DisputeResolved event(s) were emitted"
	exit 1
fi
echo "OK: both sentinels agreed (approved) and no arbitration was triggered."

# --- 9. Check the fee and bond flow ---
SENTINEL_A_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
	"getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))" "$REQUEST_ID" "$SENTINEL_A_ADDR")
SENTINEL_B_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
	"getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))" "$REQUEST_ID" "$SENTINEL_B_ADDR")
if [ "$(echo "$SENTINEL_A_COMMITMENT" | jq -r '.[0][3]')" != "true" ] || [ "$(echo "$SENTINEL_B_COMMITMENT" | jq -r '.[0][3]')" != "true" ]; then
	echo "FAILED: expected both sentinels to have claimed their bond and reward"
	echo "Sentinel A commitment: $SENTINEL_A_COMMITMENT"
	echo "Sentinel B commitment: $SENTINEL_B_COMMITMENT"
	exit 1
fi

SENTINEL_A_BALANCE_AFTER=$(balance_of "$SENTINEL_A_ADDR")
SENTINEL_B_BALANCE_AFTER=$(balance_of "$SENTINEL_B_ADDR")
ORACLE_BALANCE_AFTER=$(balance_of "$ORACLE")
# Both bonds are returned in full (no slashing on a unanimous vote), so any
# balance gained beyond the bond amount is the sentinel's share of the request
# fee. The two shares should add up to (approximately) the whole fee: the
# equal-split reward can leave a wei or two of rounding dust behind in the
# oracle.
SENTINEL_A_REWARD=$((SENTINEL_A_BALANCE_AFTER - SENTINEL_A_BALANCE_BEFORE))
SENTINEL_B_REWARD=$((SENTINEL_B_BALANCE_AFTER - SENTINEL_B_BALANCE_BEFORE))
TOTAL_REWARD=$((SENTINEL_A_REWARD + SENTINEL_B_REWARD))
echo "Sentinel A fee share: $SENTINEL_A_REWARD"
echo "Sentinel B fee share: $SENTINEL_B_REWARD"
echo "Oracle balance after claims (dust only): $ORACLE_BALANCE_AFTER"
if [ "$SENTINEL_A_REWARD" -le 0 ] || [ "$SENTINEL_B_REWARD" -le 0 ]; then
	echo "FAILED: expected both sentinels to receive a nonzero share of the request fee"
	exit 1
fi
if [ "$TOTAL_REWARD" -gt "$REQUEST_FEE" ] || [ "$((REQUEST_FEE - TOTAL_REWARD))" -gt 2 ]; then
	echo "FAILED: expected the fee shares to add up to ~$REQUEST_FEE, got $TOTAL_REWARD"
	exit 1
fi
if [ "$ORACLE_BALANCE_AFTER" -gt 2 ]; then
	echo "FAILED: expected the oracle to hold no more than rounding dust, got $ORACLE_BALANCE_AFTER"
	exit 1
fi
echo "OK: bonds were returned in full and the request fee was split between the sentinels."

# --- 10. Propose a disputed transaction: sentinel B denies, sentinel A approves ---
# The sentinel client must claim after arbitration regardless of which side it
# was on, since bond slashing is only partial (INITIAL_SLASHING_MULTIPLIER <
# BOND_MULTIPLIER).
echo "Approving the oracle to pull the request fee for the disputed request..."
cast send --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" \
	"$FEE_TOKEN" "approve(address,uint256)" "$ORACLE" "$REQUEST_FEE" >/dev/null

echo "Proposing a transaction sentinel B's blocklist denies (to trigger a genuine dispute)..."
env \
	CONSENSUS_ADDRESS="$CONSENSUS" \
	ORACLE_ADDRESS="$ORACLE" \
	TX_CHAIN_ID="$CHAIN_ID" \
	TX_SAFE="$TX_SAFE" \
	TX_TO="$TX_TOKEN" \
	TX_DATA="$(cast calldata "transfer(address,uint256)" "$TX_RECIPIENT" 1)" \
	TX_NONCE=1 \
	forge script --root "$ROOT/contracts" ProposeTransactionScript --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" --broadcast

DISPUTE_REQUEST_ID=$(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$ORACLE" \
	'NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)' | jq -r '.[-1].topics[1]')
echo "Disputed request id: $DISPUTE_REQUEST_ID"

DISPUTE_SENTINEL_A_BALANCE_BEFORE=$(balance_of "$SENTINEL_A_ADDR")
DISPUTE_SENTINEL_B_BALANCE_BEFORE=$(balance_of "$SENTINEL_B_ADDR")

# --- 11. Wait for both sentinels to commit, reveal opposing votes, and freeze ---
echo "Waiting for the disputed request to freeze..."
TIMEOUT_SECONDS=30
ELAPSED_SECONDS=0
STATE=""
while true; do
	REQUEST=$(get_request "$DISPUTE_REQUEST_ID") || true
	STATE=$(echo "$REQUEST" | jq -r ".[$REQUEST_PROGRESS_INDEX][$REQUEST_STATE_INDEX]" 2>/dev/null) || true
	[ "$STATE" = "$STATE_FROZEN" ] && break
	if [ "$ELAPSED_SECONDS" -ge "$TIMEOUT_SECONDS" ]; then
		echo "FAILED: timed out waiting for the disputed request to freeze; last state was $STATE"
		echo "Request: $REQUEST"
		exit 1
	fi
	sleep "$BLOCK_TIME_SECONDS"
	ELAPSED_SECONDS=$((ELAPSED_SECONDS + BLOCK_TIME_SECONDS))
done
echo "OK: both sentinels revealed opposing votes and the request froze."

# --- 12. Arbitrate: rule for the approving side (sentinel A), against sentinel B ---
echo "Resolving the dispute in favor of the approving side..."
cast send --rpc-url "$RPC_URL" --private-key "$DEPLOYER_PK" "$ORACLE" \
	"resolveDispute(bytes32,bool,string)" "$DISPUTE_REQUEST_ID" true "sentinel B's blocklisted destination" >/dev/null

# --- 13. Wait for both the winning and the losing sentinel to claim ---
# Before Phase 6, a losing sentinel never claimed (nothing to claim, since the
# whole bond was forfeited); now slashing is partial, so sentinel B must claim
# its unslashed remainder too.
A_CLAIMED=""
B_CLAIMED=""
for _ in $(seq 1 10); do
	REQUEST=$(get_request "$DISPUTE_REQUEST_ID") || true
	STATE=$(echo "$REQUEST" | jq -r ".[$REQUEST_PROGRESS_INDEX][$REQUEST_STATE_INDEX]" 2>/dev/null) || true
	SENTINEL_A_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
		"getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))" "$DISPUTE_REQUEST_ID" "$SENTINEL_A_ADDR") || true
	SENTINEL_B_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
		"getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))" "$DISPUTE_REQUEST_ID" "$SENTINEL_B_ADDR") || true
	A_CLAIMED=$(echo "$SENTINEL_A_COMMITMENT" | jq -r '.[0][3]' 2>/dev/null) || true
	B_CLAIMED=$(echo "$SENTINEL_B_COMMITMENT" | jq -r '.[0][3]' 2>/dev/null) || true
	[ "$A_CLAIMED" = "true" ] && [ "$B_CLAIMED" = "true" ] && break
	sleep "$BLOCK_TIME_SECONDS"
done

echo "Disputed request state after arbitration: $REQUEST"
if [ "$STATE" != "$STATE_RESOLVED_APPROVED" ]; then
	echo "FAILED: expected state RESOLVED_APPROVED ($STATE_RESOLVED_APPROVED), got $STATE"
	exit 1
fi
if [ "$A_CLAIMED" != "true" ] || [ "$B_CLAIMED" != "true" ]; then
	echo "FAILED: expected both the winning and the losing sentinel to have claimed"
	echo "Sentinel A commitment: $SENTINEL_A_COMMITMENT"
	echo "Sentinel B commitment: $SENTINEL_B_COMMITMENT"
	exit 1
fi
echo "OK: arbitration resolved the dispute and both sentinels claimed, including the losing one."

# --- 14. Check the partial bond slash reconciles ---
SLASH_AMOUNT=$((REQUEST_FEE * INITIAL_SLASHING_MULTIPLIER))
DISPUTE_SENTINEL_A_BALANCE_AFTER=$(balance_of "$SENTINEL_A_ADDR")
DISPUTE_SENTINEL_B_BALANCE_AFTER=$(balance_of "$SENTINEL_B_ADDR")
DISPUTE_ORACLE_BALANCE_AFTER=$(balance_of "$ORACLE")
DISPUTE_SENTINEL_A_CHANGE=$((DISPUTE_SENTINEL_A_BALANCE_AFTER - DISPUTE_SENTINEL_A_BALANCE_BEFORE))
DISPUTE_SENTINEL_B_CHANGE=$((DISPUTE_SENTINEL_B_BALANCE_AFTER - DISPUTE_SENTINEL_B_BALANCE_BEFORE))
echo "Sentinel A (won the dispute) balance change: $DISPUTE_SENTINEL_A_CHANGE"
echo "Sentinel B (lost the dispute) balance change: $DISPUTE_SENTINEL_B_CHANGE"

# The sole winner's bond returns in full (net 0) plus the whole request fee as
# its reward (no other winner to split it with, no DAO cut configured).
if [ "$DISPUTE_SENTINEL_A_CHANGE" != "$REQUEST_FEE" ]; then
	echo "FAILED: expected the winning sentinel to net exactly the request fee ($REQUEST_FEE), got $DISPUTE_SENTINEL_A_CHANGE"
	exit 1
fi
# The sole loser forfeits only the governed slash amount, not its whole bond —
# the partial-slashing behavior this scenario exists to exercise.
if [ "$DISPUTE_SENTINEL_B_CHANGE" != "-$SLASH_AMOUNT" ]; then
	echo "FAILED: expected the losing sentinel to net exactly -$SLASH_AMOUNT (a partial, not full, bond forfeiture), got $DISPUTE_SENTINEL_B_CHANGE"
	exit 1
fi
if [ "$DISPUTE_ORACLE_BALANCE_AFTER" -gt 2 ]; then
	echo "FAILED: expected the oracle to hold no more than rounding dust after arbitration, got $DISPUTE_ORACLE_BALANCE_AFTER"
	exit 1
fi
echo "OK: the losing sentinel's bond was slashed only partially and both sides' balances reconcile."

# --- 15. Propose another disputed transaction, which the arbitrator never rules on ---
# Nobody but the sentinels calls the permissionless `timeoutArbitration` here,
# so the bonds only come back if the sentinels submit it themselves once the
# arbitration deadline passes.
UNRULED_SPONSOR_BALANCE_BEFORE=$(balance_of "$SPONSOR_ADDR")
echo "Approving the oracle to pull the request fee for the unruled disputed request..."
cast send --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" \
	"$FEE_TOKEN" "approve(address,uint256)" "$ORACLE" "$REQUEST_FEE" >/dev/null

UNRULED_SENTINEL_A_BALANCE_BEFORE=$(balance_of "$SENTINEL_A_ADDR")
UNRULED_SENTINEL_B_BALANCE_BEFORE=$(balance_of "$SENTINEL_B_ADDR")

echo "Proposing another transaction sentinel B's blocklist denies (a dispute left unruled)..."
env \
	CONSENSUS_ADDRESS="$CONSENSUS" \
	ORACLE_ADDRESS="$ORACLE" \
	TX_CHAIN_ID="$CHAIN_ID" \
	TX_SAFE="$TX_SAFE" \
	TX_TO="$TX_TOKEN" \
	TX_DATA="$(cast calldata "transfer(address,uint256)" "$TX_RECIPIENT" 1)" \
	TX_NONCE=2 \
	forge script --root "$ROOT/contracts" ProposeTransactionScript --rpc-url "$RPC_URL" --private-key "$SPONSOR_PK" --broadcast

UNRULED_REQUEST_ID=$(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$ORACLE" \
	'NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)' | jq -r '.[-1].topics[1]')
echo "Unruled disputed request id: $UNRULED_REQUEST_ID"

# --- 16. Wait for the request to freeze, then time out without a ruling ---
# The whole commit/reveal window plus `ARBITRATION_TIMEOUT` and a margin for
# the sentinels' `timeoutArbitration`/`claim` transactions to land.
echo "Waiting for the sentinels to time out the unruled arbitration..."
TIMEOUT_SECONDS=$((COMMIT_WINDOW + REVEAL_WINDOW + ARBITRATION_TIMEOUT + 20))
ELAPSED_SECONDS=0
STATE=""
FROZE=""
while true; do
	REQUEST=$(get_request "$UNRULED_REQUEST_ID") || true
	STATE=$(echo "$REQUEST" | jq -r ".[$REQUEST_PROGRESS_INDEX][$REQUEST_STATE_INDEX]" 2>/dev/null) || true
	if [ -z "$FROZE" ] && [ "$STATE" = "$STATE_FROZEN" ]; then
		FROZE=1
		ARBITRATION_DEADLINE=$(echo "$REQUEST" | jq -r ".[$REQUEST_PROGRESS_INDEX][$REQUEST_ARBITRATION_DEADLINE_INDEX]")
		echo "Request froze; arbitration deadline is block $ARBITRATION_DEADLINE."
	fi
	[ "$STATE" = "$STATE_TIMED_OUT" ] && break
	if [ "$ELAPSED_SECONDS" -ge "$TIMEOUT_SECONDS" ]; then
		echo "FAILED: timed out waiting for the unruled arbitration to time out; last state was $STATE"
		echo "Request: $REQUEST"
		exit 1
	fi
	sleep "$BLOCK_TIME_SECONDS"
	ELAPSED_SECONDS=$((ELAPSED_SECONDS + BLOCK_TIME_SECONDS))
done
if [ -z "$FROZE" ]; then
	echo "FAILED: expected the request to freeze (a dispute) before timing out"
	exit 1
fi

ARBITRATION_TIMEOUT_TX=$(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$ORACLE" \
	'ArbitrationTimedOut(bytes32)' "$UNRULED_REQUEST_ID" | jq -r '.[0].transactionHash')
ARBITRATION_TIMEOUT_SENDER=$(cast tx --rpc-url "$RPC_URL" --json "$ARBITRATION_TIMEOUT_TX" | jq -r '.from')
ARBITRATION_TIMEOUT_SENDER=$(cast to-check-sum-address "$ARBITRATION_TIMEOUT_SENDER")
if [ "$ARBITRATION_TIMEOUT_SENDER" != "$SENTINEL_A_ADDR" ] && [ "$ARBITRATION_TIMEOUT_SENDER" != "$SENTINEL_B_KEY_ADDR" ]; then
	echo "FAILED: expected a sentinel to have submitted timeoutArbitration, but it came from $ARBITRATION_TIMEOUT_SENDER"
	exit 1
fi
echo "OK: $ARBITRATION_TIMEOUT_SENDER timed out the unruled arbitration."

# --- 17. Wait for both sentinels to claim their full bonds back ---
A_CLAIMED=""
B_CLAIMED=""
for _ in $(seq 1 10); do
	SENTINEL_A_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
		"getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))" "$UNRULED_REQUEST_ID" "$SENTINEL_A_ADDR") || true
	SENTINEL_B_COMMITMENT=$(cast call --rpc-url "$RPC_URL" --json "$ORACLE" \
		"getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))" "$UNRULED_REQUEST_ID" "$SENTINEL_B_ADDR") || true
	A_CLAIMED=$(echo "$SENTINEL_A_COMMITMENT" | jq -r '.[0][3]' 2>/dev/null) || true
	B_CLAIMED=$(echo "$SENTINEL_B_COMMITMENT" | jq -r '.[0][3]' 2>/dev/null) || true
	[ "$A_CLAIMED" = "true" ] && [ "$B_CLAIMED" = "true" ] && break
	sleep "$BLOCK_TIME_SECONDS"
done
if [ "$A_CLAIMED" != "true" ] || [ "$B_CLAIMED" != "true" ]; then
	echo "FAILED: expected both sentinels to have claimed after the arbitration timeout"
	echo "Sentinel A commitment: $SENTINEL_A_COMMITMENT"
	echo "Sentinel B commitment: $SENTINEL_B_COMMITMENT"
	exit 1
fi

# Both sentinels revealed, so no ruling means no slash: each bond returns in
# full (net 0, no fee reward), and the fee is refunded to the sponsor.
UNRULED_SENTINEL_A_CHANGE=$(($(balance_of "$SENTINEL_A_ADDR") - UNRULED_SENTINEL_A_BALANCE_BEFORE))
UNRULED_SENTINEL_B_CHANGE=$(($(balance_of "$SENTINEL_B_ADDR") - UNRULED_SENTINEL_B_BALANCE_BEFORE))
UNRULED_SPONSOR_CHANGE=$(($(balance_of "$SPONSOR_ADDR") - UNRULED_SPONSOR_BALANCE_BEFORE))
echo "Sentinel A balance change: $UNRULED_SENTINEL_A_CHANGE"
echo "Sentinel B balance change: $UNRULED_SENTINEL_B_CHANGE"
echo "Sponsor balance change: $UNRULED_SPONSOR_CHANGE"
if [ "$UNRULED_SENTINEL_A_CHANGE" != "0" ] || [ "$UNRULED_SENTINEL_B_CHANGE" != "0" ]; then
	echo "FAILED: expected both sentinels' bonds to return in full after the arbitration timeout"
	exit 1
fi
if [ "$UNRULED_SPONSOR_CHANGE" != "0" ]; then
	echo "FAILED: expected the sponsor's request fee to be refunded after the arbitration timeout"
	exit 1
fi
echo "OK: the sentinels timed out the unruled arbitration and recovered their bonds in full."

# --- 18. Measure the gas the Roles path costs sentinel B ---
# Each of sentinel B's transactions went through the Roles modifier, and its
# Safe logged `ExecutionFromModuleSuccess` for it. Tracing one finds the
# Safe's own call to the oracle or fee token inside it, which gives:
#   - the Roles overhead: the gas used, minus that call's gas and the
#     intrinsic gas of sending the same call directly;
#   - the headroom: the gas that call received, minus the gas limit the
#     sentinel sets for the direct call (the transaction's limit minus
#     `gas_overhead`). It must not be negative, or `gas_overhead` is too small
#     and a call that fits its own limit could run out of gas through Roles.
echo "Measuring the gas of sentinel B's transactions through the Roles modifier..."
ACTIONS=$(jq -n \
	--arg approve "$(cast sig "approve(address,uint256)")" \
	--arg commit "$(cast sig "commit(bytes32,bytes32)")" \
	--arg reveal "$(cast sig "reveal(bytes32,bool,bytes32,string)")" \
	--arg finalize "$(cast sig "finalize(bytes32)")" \
	--arg claim "$(cast sig "claim(bytes32)")" \
	--arg timeout "$(cast sig "timeoutArbitration(bytes32)")" \
	'{($approve): "approve", ($commit): "commit", ($reveal): "reveal",
	  ($finalize): "finalize", ($claim): "claim", ($timeout): "timeoutArbitration"}')
GAS_MEASUREMENTS=$(mktemp)
for tx in $(cast logs --rpc-url "$RPC_URL" --json --from-block 0 --address "$SENTINEL_B_ADDR" \
	"ExecutionFromModuleSuccess(address)" | jq -r '.[].transactionHash' | sort -u); do
	cast rpc --rpc-url "$RPC_URL" debug_traceTransaction "$tx" '{"tracer":"callTracer"}' | jq -c \
		--arg tx "$tx" \
		--arg safe "$SENTINEL_B_ADDR" \
		--argjson actions "$ACTIONS" \
		--argjson receipt "$(cast receipt --rpc-url "$RPC_URL" --json "$tx")" \
		--argjson overheadLimit "$ROLES_GAS_OVERHEAD" '
		def num: if type == "number" then . else ltrimstr("0x") | ascii_downcase | explode
			| reduce .[] as $c (0; . * 16 + (if $c >= 97 then $c - 87 else $c - 48 end)) end;
		def calldata_gas: ltrimstr("0x") | [range(0; length; 2) as $i | .[$i:$i + 2]]
			| map(if . == "00" then 4 else 16 end) | add;
		. as $outer
		| [.. | objects | select(.type? == "CALL" and ((.from? // "") | ascii_downcase) == ($safe | ascii_downcase))]
		| first as $inner
		| ($receipt.gasUsed | num) as $gasUsed
		| {
			tx: $tx,
			action: ($actions[$inner.input[0:10]] // $inner.input[0:10]),
			success: (($receipt.status | num) == 1),
			gasUsed: $gasUsed,
			overhead: ($gasUsed - ($inner.gasUsed | num) - 21000 - ($inner.input | calldata_gas)),
			headroom: (($inner.gas | num) - (($outer.gas | num) - $overheadLimit))
		}' >>"$GAS_MEASUREMENTS"
done

FAILED_TXS=$(jq -s -r 'map(select(.success | not) | .tx) | join(" ")' "$GAS_MEASUREMENTS")
if [ -n "$FAILED_TXS" ]; then
	echo "FAILED: sentinel B's transactions through Roles reverted: $FAILED_TXS"
	exit 1
fi
MISSING_ACTIONS=$(jq -s -r '[map(.action) as $seen | ("approve", "commit", "reveal", "claim")
	| select(. as $a | $seen | index($a) | not)] | join(" ")' "$GAS_MEASUREMENTS")
if [ -n "$MISSING_ACTIONS" ]; then
	echo "FAILED: expected sentinel B to have sent through Roles: $MISSING_ACTIONS"
	exit 1
fi
SHORT_TXS=$(jq -s -r 'map(select(.headroom < 0) | "\(.action) \(.tx) (\(.headroom))") | join(", ")' "$GAS_MEASUREMENTS")
if [ -n "$SHORT_TXS" ]; then
	echo "FAILED: gas_overhead ($ROLES_GAS_OVERHEAD) leaves these calls less gas than sent directly: $SHORT_TXS"
	exit 1
fi

# The USD cost at the current Gnosis Chain gas price, if it can be read.
GAS_PRICE_WEI=$(cast gas-price --rpc-url "$GNOSIS_RPC_URL" 2>/dev/null) || GAS_PRICE_WEI=""
echo
echo "Sentinel B (Safe v1.5.0 + Zodiac Roles v2.1.1) gas per action, worst case over this run:"
printf "  %-19s %4s %10s %15s %10s %13s %13s\n" \
	action txs "gas used" "Roles overhead" headroom "cost (USD)" "overhead (USD)"
jq -s -r --arg price "${GAS_PRICE_WEI:-0}" '
	($price | tonumber) as $price
	| group_by(.action)[]
	| [.[0].action, length, (map(.gasUsed) | max), (map(.overhead) | max), (map(.headroom) | min),
	   ((map(.gasUsed) | max) * $price / 1e18), ((map(.overhead) | max) * $price / 1e18)]
	| @tsv' "$GAS_MEASUREMENTS" |
	while IFS=$'\t' read -r action count gas_used overhead headroom cost overhead_cost; do
		if [ -n "$GAS_PRICE_WEI" ]; then
			printf "  %-19s %4s %10s %15s %10s %13.3g %13.3g\n" \
				"$action" "$count" "$gas_used" "$overhead" "$headroom" "$cost" "$overhead_cost"
		else
			printf "  %-19s %4s %10s %15s %10s %13s %13s\n" \
				"$action" "$count" "$gas_used" "$overhead" "$headroom" - -
		fi
	done
if [ -n "$GAS_PRICE_WEI" ]; then
	echo "  USD at the current Gnosis Chain gas price of $GAS_PRICE_WEI wei (xDAI = \$1)."
else
	echo "  USD skipped: couldn't read the gas price from $GNOSIS_RPC_URL."
fi
echo "OK: every sentinel B transaction went through Roles within gas_overhead ($ROLES_GAS_OVERHEAD)."

echo "Sentinel integration test finished successfully."
