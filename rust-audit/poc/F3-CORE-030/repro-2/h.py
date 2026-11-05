#!/usr/bin/env python3
"""QA3-2B local end-to-end harness for the Safenet sentinel (run 3, second reproduction).

Everything runs on loopback ports 31700..31709 only: one Anvil (chain id 100, so the sentinel
uses the Gnosis block timings 5000/500/[500,500,500,1000,250,250] ms), the real Consensus
(TestConsensus deployment, as in scripts/run_sentinel_integration_test.sh) and SentinelOracle
with the Aegis production parameters, real `sentinel` binaries with file-backed SQLite, and a
stub engine whose answer can be held. Blocks are mined by this harness every 5 s on the wall
clock (automine off), so the order of transactions per block is observable and repeatable.
Stdlib only.
"""
import http.server
import json
import os
import pathlib
import signal
import subprocess
import threading
import time
import urllib.request

RUN = "/home/sj.guest/.cache/kb-runs/safenet/2026-10-08-rust-audit-run3"
WT = f"{RUN}/wt/qa-2b"
CONTRACTS = f"{WT}/contracts"
SCRATCH = f"{RUN}/scratch/QA3-2B"
ANVIL_PORT = 31700
RPC = f"http://127.0.0.1:{ANVIL_PORT}"
CHAIN_ID = 100
BLOCK_TIME = 5

# Aegis production oracle parameters (contracts/.env.aegis.sample:15-25).
REQUEST_FEE = 400_000  # 0.40 in 6-decimal units
BOND_MULTIPLIER = 2000  # bond 800.00
SLASHING_MULTIPLIER = 1  # slash 0.40
COMMIT_WINDOW = 6
REVEAL_WINDOW = 6
ARBITRATION_TIMEOUT = 483_840
DAO_FEE_SHARE = 0
GOVERNANCE_DELAY = 0  # Aegis uses 51,840; 0 only makes addSentinel effective at once in setup
BOND = REQUEST_FEE * BOND_MULTIPLIER
SLASH = REQUEST_FEE * SLASHING_MULTIPLIER

# Anvil's default development accounts (local chain only).
ACCT = [
    ("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
    ("0x70997970C51812dc3A010C7d01b50e0d17dc79C8", "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"),
    ("0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC", "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"),
    ("0x90F79bf6EB2c4f870365E785982E1f101E93b906", "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6"),
    ("0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65", "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a"),
    ("0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc", "0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba"),
]
DEPLOYER, DEPLOYER_PK = ACCT[0]
SPONSOR, SPONSOR_PK = ACCT[4]
TX_SAFE = "0x1111111111111111111111111111111111111111"

T0 = time.time()
ERRORS = {"0xf41814a7": "RequestNotPending()", "0xbe7223ba": "FinalizeTooEarly()",
          "0xbfec5558": "AlreadyCommitted()", "0xa59930e2": "CommitWindowClosed()",
          "0x81b07242": "RevealWindowClosed()", "0x916176f7": "RevealWindowNotOpen()"}
LOG_LINES = []


def log(msg):
    line = f"[{time.time() - T0:7.1f}s] {msg}"
    LOG_LINES.append(line)
    print(line, flush=True)


def sh(args, env=None, check=True, timeout=300):
    full_env = dict(os.environ)
    if env:
        full_env.update(env)
    r = subprocess.run(args, capture_output=True, text=True, env=full_env, timeout=timeout)
    if check and r.returncode != 0:
        raise RuntimeError(f"command failed ({r.returncode}): {' '.join(args)}\n{r.stdout}\n{r.stderr}")
    return r.stdout.strip()


def rpc(method, params=None, url=RPC):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}).encode()
    req = urllib.request.Request(url, body, {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        resp = json.loads(r.read())
    if "error" in resp:
        raise RuntimeError(f"{method}: {resp['error']}")
    return resp["result"]


def block_number(url=RPC):
    return int(rpc("eth_blockNumber", url=url), 16)


def cast_send(pk, to, sig, *args, extra=(), wait=True):
    cmd = ["cast", "send", "--rpc-url", RPC, "--private-key", pk, *extra, to, sig, *map(str, args)]
    if not wait:
        cmd.insert(2, "--async")
    return sh(cmd)


def cast_call(to, sig, *args, block=None):
    cmd = ["cast", "call", "--rpc-url", RPC, "--json", to, sig, *map(str, args)]
    if block is not None:
        cmd += ["--block", str(block)]
    return json.loads(sh(cmd))


def selector(sig):
    return sh(["cast", "sig", sig])


# ---------------------------------------------------------------- Anvil and mining


class Anvil:
    def __init__(self, port=ANVIL_PORT, extra=()):
        self.port = port
        self.url = f"http://127.0.0.1:{port}"
        self.logf = open(f"{SCRATCH}/runs/anvil-{port}.log", "w")
        self.proc = subprocess.Popen(
            ["anvil", "--host", "127.0.0.1", "--port", str(port), "--chain-id", str(CHAIN_ID),
             "--gas-limit", "17000000", *extra],
            stdout=self.logf, stderr=subprocess.STDOUT, start_new_session=True)
        for _ in range(100):
            try:
                block_number(self.url)
                return
            except Exception:
                time.sleep(0.1)
        raise RuntimeError("anvil did not start")

    def stop(self):
        if self.proc.poll() is None:
            os.killpg(self.proc.pid, signal.SIGTERM)
            self.proc.wait(timeout=10)


class Miner(threading.Thread):
    """Mines one block on every multiple of BLOCK_TIME seconds of wall-clock time."""

    def __init__(self, url=RPC):
        super().__init__(daemon=True)
        self.url = url
        self.running = threading.Event()
        self.running.set()
        self.stopped = False
        self.mined = []

    def run(self):
        while not self.stopped:
            now = time.time()
            nxt = (int(now) // BLOCK_TIME + 1) * BLOCK_TIME
            time.sleep(max(0.0, nxt - now))
            if self.stopped:
                return
            if not self.running.is_set():
                continue
            try:
                rpc("evm_mine", [nxt], url=self.url)
                n = block_number(self.url)
                txs = rpc("eth_getBlockByNumber", [hex(n), False], url=self.url)["transactions"]
                self.mined.append((n, nxt, len(txs)))
            except Exception as e:  # keep mining; report
                log(f"miner error: {e}")

    def wait_block(self, n, timeout=600):
        end = time.time() + timeout
        while block_number(self.url) < n:
            if time.time() > end:
                raise TimeoutError(f"block {n} not reached")
            time.sleep(0.2)


# ---------------------------------------------------------------- contracts


class Deployment:
    def __init__(self, sentinels):
        """`sentinels` is a list of (address, key) registered with the oracle."""
        env = {"FACTORY": "2"}
        sh(["forge", "script", "--root", CONTRACTS, "DeployERC20Script", "--rpc-url", RPC,
            "--private-key", DEPLOYER_PK, "--broadcast"], env=env)
        self.fee_token = self._ret("DeployERC20.s.sol", "erc20")
        sh(["forge", "script", "--root", CONTRACTS, "DeployTestConsensusScript", "--rpc-url", RPC,
            "--private-key", DEPLOYER_PK, "--broadcast"])
        self.consensus = self._ret("DeployTestConsensus.s.sol", "consensus")
        env = {
            "FACTORY": "2",
            "SENTINEL_ARBITRATOR": DEPLOYER,
            "SENTINEL_GOVERNANCE": DEPLOYER,
            "SENTINEL_PROTOCOL_FUNDS_RECEIVER": DEPLOYER,
            "SENTINEL_CONSENSUS": self.consensus,
            "SENTINEL_FEE_TOKEN": self.fee_token,
            "SENTINEL_REQUEST_FEE": str(REQUEST_FEE),
            "SENTINEL_COMMIT_WINDOW": str(COMMIT_WINDOW),
            "SENTINEL_REVEAL_WINDOW": str(REVEAL_WINDOW),
            "SENTINEL_GOVERNANCE_DELAY": str(GOVERNANCE_DELAY),
            "SENTINEL_BOND_MULTIPLIER": str(BOND_MULTIPLIER),
            "SENTINEL_INITIAL_SLASHING_MULTIPLIER": str(SLASHING_MULTIPLIER),
            "SENTINEL_INITIAL_DAO_FEE_SHARE": str(DAO_FEE_SHARE),
            "SENTINEL_CHARTER_ENS": "charter.safenet-gov.eth",
            "SENTINEL_ARBITRATION_TIMEOUT": str(ARBITRATION_TIMEOUT),
        }
        sh(["forge", "script", "--root", CONTRACTS, "DeploySentinelOracleScript", "--rpc-url", RPC,
            "--private-key", DEPLOYER_PK, "--broadcast"], env=env)
        self.oracle = self._ret("DeploySentinelOracle.s.sol", "sentinelOracle")
        for addr, _ in sentinels + [(SPONSOR, None)]:
            cast_send(DEPLOYER_PK, self.fee_token, "transfer(address,uint256)", addr, 100 * BOND)
        for addr, _ in sentinels:
            cast_send(DEPLOYER_PK, self.oracle, "addSentinel(address)", addr)
        cast_send(SPONSOR_PK, self.fee_token, "approve(address,uint256)", self.oracle, 10**30)
        self.nonce = 0
        self.sel = {
            selector("approve(address,uint256)"): "approve",
            selector("commit(bytes32,bytes32)"): "commit",
            selector("reveal(bytes32,bool,bytes32,string)"): "reveal",
            selector("finalize(bytes32)"): "finalize",
            selector("claim(bytes32)"): "claim",
            selector("timeoutArbitration(bytes32)"): "timeoutArbitration",
        }
        log(f"deployed fee_token={self.fee_token} consensus={self.consensus} oracle={self.oracle}")

    @staticmethod
    def _ret(script, key):
        path = f"{CONTRACTS}/build/broadcast/{script}/{CHAIN_ID}/run-latest.json"
        return json.load(open(path))["returns"][key]["value"]

    def safe_tx(self, nonce):
        # (chainId, safe, to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken,
        # refundReceiver, nonce)
        z = "0x0000000000000000000000000000000000000000"
        return f"({CHAIN_ID},{TX_SAFE},{TX_SAFE},0,0x,0,0,0,0,{z},{z},{nonce})"

    PROPOSE_SIG = ("proposeTransaction(address,bytes,(uint256,address,address,uint256,bytes,uint8,"
                   "uint256,uint256,uint256,address,address,uint256))")

    def request_id(self, nonce):
        """The request id (Consensus transaction-proposal message) for the next proposal."""
        safe_tx_hash = sh(["cast", "call", "--rpc-url", RPC, "--from", SPONSOR, self.consensus,
                           self.PROPOSE_SIG, self.oracle, "0x", self.safe_tx(nonce)])
        domain = sh(["cast", "call", "--rpc-url", RPC, self.consensus, "domainSeparator()(bytes32)"])
        epoch = 0
        typehash = "9c6706f5afdb1de99f5ad39011e7770ce471f51d78380634f6cedb21a648b8d0"
        empty = sh(["cast", "keccak", "0x"])[2:]
        struct = typehash + f"{epoch:064x}" + self.oracle[2:].lower().rjust(64, "0") + empty + safe_tx_hash[2:]
        struct_hash = sh(["cast", "keccak", "0x" + struct])[2:]
        return sh(["cast", "keccak", "0x1901" + domain[2:] + struct_hash])

    def propose(self, wait=False):
        """Sends a proposal for the configured oracle; returns (request id, tx hash)."""
        nonce = self.nonce
        self.nonce += 1
        rid = self.request_id(nonce)
        out = cast_send(SPONSOR_PK, self.consensus, self.PROPOSE_SIG, self.oracle, "0x",
                        self.safe_tx(nonce), extra=["--gas-limit", "1000000"], wait=wait)
        return rid, out

    def get_request(self, rid, block=None):
        r = cast_call(self.oracle,
                      "getRequest(bytes32)((uint64,uint24,uint64,uint96,address,uint96,uint8),"
                      "(uint8,uint96,uint64,uint16,uint16,uint16,uint16,uint24))", rid, block=block)
        terms, prog = r
        names = ["NONE", "PENDING", "FROZEN", "RESOLVED_APPROVED", "RESOLVED_DENIED", "TIMED_OUT"]
        return {
            "commitDeadline": int(terms[0]), "revealDeadline": int(terms[2]),
            "state": names[int(prog[0])], "committed": int(prog[3]), "revealed": int(prog[4]),
            "approve": int(prog[5]), "deny": int(prog[6]),
        }

    def get_commitment(self, rid, who):
        c = cast_call(self.oracle, "getCommitment(bytes32,address)((bytes32,uint96,uint8,bool))", rid, who)[0]
        votes = ["NONE", "PENDING", "APPROVED", "DENIED"]
        return {"bond": int(c[1]), "vote": votes[int(c[2])] if int(c[2]) < 4 else c[2], "claimed": bool(c[3])}

    def balance(self, who):
        return int(cast_call(self.fee_token, "balanceOf(address)(uint256)", who)[0])

    def txs_from(self, who, first, last):
        """Every transaction `who` sent to the oracle or fee token in blocks first..last."""
        out = []
        for n in range(first, last + 1):
            blk = rpc("eth_getBlockByNumber", [hex(n), True])
            for i, tx in enumerate(blk["transactions"]):
                if tx["from"].lower() != who.lower():
                    continue
                rc = rpc("eth_getTransactionReceipt", [tx["hash"]])
                name = self.sel.get(tx["input"][:10], tx["input"][:10])
                reason = ""
                if rc["status"] == "0x0":
                    try:
                        rpc("eth_call", [{"from": tx["from"], "to": tx["to"], "data": tx["input"],
                                          "gas": tx["gas"]}, hex(n - 1)])
                    except RuntimeError as e:
                        reason = str(e)[:160]
                        for errcode, errname in ERRORS.items():
                            if errcode in reason:
                                reason = errname
                out.append({"block": n, "index": i, "fn": name, "status": int(rc["status"], 16),
                            "gasUsed": int(rc["gasUsed"], 16), "replay": reason})
        return out

    def oracle_events(self, first, last):
        """Decoded oracle event names per block (topic0 lookup)."""
        names = {}
        for sig in ["NewRequest(bytes32,address,uint96,uint96,uint24,uint96,uint64,uint64)",
                    "Committed(bytes32,address,uint96)", "Revealed(bytes32,address,bool,uint96,string)",
                    "OracleResult(bytes32,address,bytes,bool)", "RequestTimedOut(bytes32)",
                    "DisputeTriggered(bytes32,uint64)", "Claimed(bytes32,address,uint96,uint96)"]:
            names[sh(["cast", "keccak", sig])] = sig.split("(")[0]
        logs = rpc("eth_getLogs", [{"fromBlock": hex(first), "toBlock": hex(last), "address": self.oracle}])
        out = []
        for lg in logs:
            name = names.get(lg["topics"][0], lg["topics"][0][:10])
            who = ""
            if name in ("Committed", "Revealed", "Claimed") and len(lg["topics"]) > 2:
                who = "0x" + lg["topics"][2][-40:]
            out.append((int(lg["blockNumber"], 16), int(lg["logIndex"], 16), name, who))
        return out


# ---------------------------------------------------------------- engine stub


class Engine:
    """Answers POST /v1/security-check with {"verdict":"secure"}; `hold` delays the answer."""

    def __init__(self, port, name):
        self.port, self.name = port, name
        self.hold = threading.Event()
        self.requests = []
        engine = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def do_POST(self):
                body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                rid = self.headers.get("x-request-id")
                t = time.time()
                engine.requests.append({"t": round(t - T0, 1), "request_id": rid,
                                        "timeout_ms": self.headers.get("x-request-timeout")})
                log(f"engine {engine.name}: request {rid} (hold={engine.hold.is_set()})")
                while engine.hold.is_set() and time.time() - t < 60:
                    time.sleep(0.05)
                out = b'{"verdict":"secure"}'
                try:
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(out)))
                    self.end_headers()
                    self.wfile.write(out)
                    log(f"engine {engine.name}: answered {rid} after {time.time() - t:.1f}s")
                except OSError as e:
                    log(f"engine {engine.name}: client gone for {rid}: {e}")

            def log_message(self, *a):
                pass

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
        self.server.daemon_threads = True
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def stop(self):
        self.server.shutdown()
        self.server.server_close()


# ---------------------------------------------------------------- sentinel process


class Sentinel:
    def __init__(self, name, key, dep, engine_port, metrics_port, binary, run_dir,
                 index_extra="", rpc_url=RPC, voting_window=6, log_filter="info,sentinel=debug,safenet_core=debug"):
        self.name, self.key, self.dep, self.binary = name, key, dep, binary
        self.addr = sh(["cast", "wallet", "address", key])
        self.dir = pathlib.Path(run_dir) / name
        self.dir.mkdir(parents=True, exist_ok=True)
        self.db = self.dir / "sentinel.db"
        self.metrics_port = metrics_port
        self.config = self.dir / "sentinel.toml"
        self.config.write_text(f'''rpc = "{rpc_url}"
signer = "{key}"
database = "sqlite:{self.db}?mode=rwc"
oracle = "{dep.oracle}"
consensus = "{dep.consensus}"

[sentinel]
fee_token = "{dep.fee_token}"
voting_window = {voting_window}
engine = "http://127.0.0.1:{engine_port}"

[observability]
log_filter = "{log_filter}"
metrics_address = "127.0.0.1:{metrics_port}"

[index]
{index_extra}
''')
        self.proc = None
        self.runs = 0
        self.logpath = None

    def start(self):
        self.runs += 1
        self.logpath = self.dir / f"run{self.runs}.jsonl"
        self.logf = open(self.logpath, "w")
        self.proc = subprocess.Popen([self.binary, "--config-file", str(self.config)], stdout=self.logf,
                                     stderr=subprocess.STDOUT, start_new_session=True)
        log(f"sentinel {self.name} ({self.addr}) started, run {self.runs}, pid {self.proc.pid}")

    def send_signal(self, sig):
        os.kill(self.proc.pid, sig)

    def stop(self, sig=signal.SIGTERM):
        if self.proc and self.proc.poll() is None:
            os.kill(self.proc.pid, sig)
            try:
                self.proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                os.kill(self.proc.pid, signal.SIGKILL)
                self.proc.wait()
        log(f"sentinel {self.name} stopped ({signal.Signals(sig).name}), exit {self.proc.returncode}")
        return self.proc.returncode

    def lines(self, run=None):
        path = self.dir / f"run{run}.jsonl" if run else self.logpath
        out = []
        for raw in open(path):
            try:
                out.append(json.loads(raw))
            except json.JSONDecodeError:
                out.append({"fields": {"message": raw.strip()}, "level": "RAW"})
        return out

    def find(self, text, run=None, field=None, value=None):
        hits = []
        for ln in self.lines(run):
            f = ln.get("fields", {})
            if text in str(f.get("message", "")) and (field is None or str(f.get(field)) == str(value)):
                hits.append(ln)
        return hits

    def wait_log(self, text, timeout=60, field=None, value=None):
        end = time.time() + timeout
        while time.time() < end:
            hits = self.find(text, field=field, value=value)
            if hits:
                return hits[0]
            time.sleep(0.1)
        raise TimeoutError(f"{self.name}: no log line containing {text!r}")

    def metric(self, name):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{self.metrics_port}/metrics", timeout=5) as r:
                text = r.read().decode()
        except Exception as e:
            return f"unavailable: {e}"
        return [ln for ln in text.splitlines() if ln.startswith(name)]


def summarize(sentinel, keys=("ignoring", "dropping", "engine check", "unrecoverable", "oracle result",
                             "reorg", "uncl", "reusing recorded", "initializing block watcher",
                             "unexpected", "submission failed", "queued", "transaction")):
    out = []
    for ln in sentinel.lines():
        f = ln.get("fields", {})
        msg = str(f.get("message", ""))
        if ln.get("level") in ("WARN", "ERROR") or any(k in msg for k in keys):
            extra = {k: v for k, v in f.items() if k != "message"}
            out.append(f"{ln.get('timestamp', '')[11:23]} {ln.get('level')} {msg} {json.dumps(extra)[:300]}")
    return out


# ---------------------------------------------------------------- JSON-RPC proxy


class JsonRpcProxy:
    """A loopback JSON-RPC proxy in front of Anvil whose per-method policy a test controls.

    `policy(method, params, raw)` returns one of:
      None / "forward"            -> relay to the upstream node unchanged
      ("http", status_int)        -> reply with that HTTP status and an empty body (e.g. 429)
      ("result", value)           -> reply 200 with {"result": value}
      ("error", code, message)    -> reply 200 with a JSON-RPC error object
    Only single (non-batch) requests are policed; batches are always forwarded.
    """

    def __init__(self, port, upstream, policy):
        self.port, self.upstream, self.policy = port, upstream, policy
        self.url = f"http://127.0.0.1:{port}"
        self.calls = []  # (t, method)
        proxy = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def _send_json(self, obj, status=200):
                body = json.dumps(obj).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def _send_status(self, status):
                self.send_response(status)
                self.send_header("Content-Length", "0")
                self.end_headers()

            def do_POST(self):
                raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                try:
                    req = json.loads(raw)
                except json.JSONDecodeError:
                    self._send_status(400)
                    return
                if isinstance(req, list):
                    self._forward(raw)
                    return
                method = req.get("method", "")
                proxy.calls.append((time.time() - T0, method))
                decision = proxy.policy(method, req.get("params", []), req)
                if decision in (None, "forward"):
                    self._forward(raw, req_id=req.get("id"))
                elif decision[0] == "http":
                    self._send_status(decision[1])
                elif decision[0] == "result":
                    self._send_json({"jsonrpc": "2.0", "id": req.get("id"), "result": decision[1]})
                elif decision[0] == "error":
                    self._send_json({"jsonrpc": "2.0", "id": req.get("id"),
                                     "error": {"code": decision[1], "message": decision[2]}})
                else:
                    self._forward(raw, req_id=req.get("id"))

            def _forward(self, raw, req_id=None):
                try:
                    r = urllib.request.Request(proxy.upstream, raw, {"Content-Type": "application/json"})
                    with urllib.request.urlopen(r, timeout=30) as up:
                        body = up.read()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                except Exception as e:
                    try:
                        self._send_json({"jsonrpc": "2.0", "id": req_id,
                                         "error": {"code": -32603, "message": f"proxy upstream: {e}"}})
                    except OSError:
                        pass

            def log_message(self, *a):
                pass

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
        self.server.daemon_threads = True
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def getlogs_bursts(self, window=0.2):
        """Groups eth_getLogs calls into time-bursts, returning [(t, count)]."""
        times = sorted(t for t, m in self.calls if m == "eth_getLogs")
        bursts, cur = [], []
        for t in times:
            if cur and t - cur[0] > window:
                bursts.append((round(cur[0], 1), len(cur)))
                cur = []
            cur.append(t)
        if cur:
            bursts.append((round(cur[0], 1), len(cur)))
        return bursts

    def stop(self):
        self.server.shutdown()
        self.server.server_close()


def health(port):
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=5) as r:
            return r.status, r.read().decode()
    except Exception as e:
        return None, str(e)
