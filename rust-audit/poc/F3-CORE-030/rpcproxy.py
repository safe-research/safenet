#!/usr/bin/env python3
"""QA3-CORE loopback JSON-RPC proxy that models faulty RPC nodes in front of Anvil.

Usage: rpcproxy.py <listen_port> <upstream_url> <control_json_path> <log_jsonl_path>

The control file is re-read on every request, so behaviour can change at runtime.
Keys (all optional):
  lag: int                      serve the chain as a node `lag` blocks behind upstream
  swallow_send: bool            record eth_sendRawTransaction, answer with its hash, do not forward
  too_soon_window_s: float      eth_getLogs by blockHash returns [] until this long after the
                                proxy first served that block (handbook "too soon" behaviour)
  inject_after_topic: hex str   when a getLogs response holds a log with this topic0 in block P,
                                answer the next getLogs by hash of block P + 1 with -32001 and the
                                next eth_getBlockByNumber(P + 1) with null (one shot each)
  rate_limit: {"window_ms": int, "max": int}  fixed-window limit on all requests, HTTP 429 above it
"""
import json
import sys
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# --- Keccak-256 (pure Python, original Keccak padding as used by Ethereum) ---
_RC = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
]
_ROT = [[0, 36, 3, 41, 18], [1, 44, 10, 45, 2], [62, 6, 43, 15, 61], [28, 55, 25, 21, 56], [27, 20, 39, 8, 14]]
_M = (1 << 64) - 1


def _rol(v, n):
    return ((v << n) | (v >> (64 - n))) & _M if n else v


def _keccak_f(a):
    for rc in _RC:
        c = [a[x][0] ^ a[x][1] ^ a[x][2] ^ a[x][3] ^ a[x][4] for x in range(5)]
        d = [c[(x - 1) % 5] ^ _rol(c[(x + 1) % 5], 1) for x in range(5)]
        a = [[a[x][y] ^ d[x] for y in range(5)] for x in range(5)]
        b = [[0] * 5 for _ in range(5)]
        for x in range(5):
            for y in range(5):
                b[y][(2 * x + 3 * y) % 5] = _rol(a[x][y], _ROT[x][y])
        a = [[b[x][y] ^ ((~b[(x + 1) % 5][y]) & b[(x + 2) % 5][y]) for y in range(5)] for x in range(5)]
        a[0][0] ^= rc
    return a


def keccak256(data: bytes) -> bytes:
    rate = 136
    msg = bytearray(data) + b"\x01"
    while len(msg) % rate:
        msg += b"\x00"
    msg[-1] |= 0x80
    a = [[0] * 5 for _ in range(5)]
    for off in range(0, len(msg), rate):
        block = msg[off:off + rate]
        for i in range(rate // 8):
            x, y = i % 5, i // 5
            a[x][y] ^= int.from_bytes(block[8 * i:8 * i + 8], "little")
        a = _keccak_f(a)
    out = b"".join(a[i % 5][i // 5].to_bytes(8, "little") for i in range(4))
    return out


assert keccak256(b"").hex() == "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"

# --- proxy ---
LISTEN_PORT = int(sys.argv[1])
UPSTREAM = sys.argv[2]
CONTROL = sys.argv[3]
LOG = sys.argv[4]

lock = threading.Lock()
log_lock = threading.Lock()
first_seen = {}  # block hash -> time first served
armed = {"logs_block": None, "null_block": None}
window = {"start": 0.0, "count": 0}
_id = [0]


def control():
    try:
        with open(CONTROL) as f:
            return json.load(f)
    except Exception:
        return {}


def log(entry):
    entry["t"] = round(time.time(), 3)
    with log_lock, open(LOG, "a") as f:
        f.write(json.dumps(entry) + "\n")


def upstream(method, params):
    with lock:
        _id[0] += 1
        rid = _id[0]
    body = json.dumps({"jsonrpc": "2.0", "id": rid, "method": method, "params": params}).encode()
    req = urllib.request.Request(UPSTREAM, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())


def upstream_result(method, params):
    resp = upstream(method, params)
    if "error" in resp:
        raise RuntimeError(f"upstream error for {method}: {resp['error']}")
    return resp["result"]


def head():
    return int(upstream_result("eth_blockNumber", []), 16)


def note_block(block):
    if isinstance(block, dict) and block.get("hash"):
        with lock:
            first_seen.setdefault(block["hash"], time.time())


def ok(rid, result):
    return {"jsonrpc": "2.0", "id": rid, "result": result}


def err(rid, code, message):
    return {"jsonrpc": "2.0", "id": rid, "error": {"code": code, "message": message}}


TAGS = ("latest", "pending", "safe", "finalized")


def handle(req, ctl):
    rid, method, params = req.get("id"), req.get("method"), req.get("params") or []
    lag = int(ctl.get("lag", 0))
    cap = max(head() - lag, 0) if lag else None

    if method == "eth_sendRawTransaction" and ctl.get("swallow_send"):
        raw = params[0]
        txhash = "0x" + keccak256(bytes.fromhex(raw[2:])).hex()
        log({"method": method, "swallowed": True, "hash": txhash, "raw": raw})
        return ok(rid, txhash)

    if method == "eth_blockNumber" and cap is not None:
        return ok(rid, hex(cap))

    if method == "eth_getBlockByNumber":
        tag = params[0]
        number = None if tag in TAGS else int(tag, 16)
        with lock:
            null_block = armed["null_block"]
        if number is not None and null_block is not None and number == null_block:
            with lock:
                armed["null_block"] = None
            log({"method": method, "number": number, "injected": "null (lagging backend)"})
            return ok(rid, None)
        if cap is not None:
            if number is None:
                number = cap
            if number > cap:
                log({"method": method, "number": number, "lag": lag, "answer": "null (behind)"})
                return ok(rid, None)
            params = [hex(number)] + list(params[1:])
        block = upstream_result(method, params)
        note_block(block)
        return ok(rid, block)

    if method == "eth_getBlockByHash":
        block = upstream_result(method, params)
        if cap is not None and block and int(block["number"], 16) > cap:
            return ok(rid, None)
        note_block(block)
        return ok(rid, block)

    if method == "eth_getLogs":
        flt = dict(params[0])
        bh = flt.get("blockHash")
        if bh:
            block = upstream_result("eth_getBlockByHash", [bh, False])
            number = int(block["number"], 16) if block else None
            with lock:
                target = armed["logs_block"]
            if number is not None and target is not None and number == target:
                with lock:
                    armed["logs_block"] = None
                    armed["null_block"] = number
                log({"method": method, "number": number, "injected": "-32001 (lagging backend)"})
                return err(rid, -32001, "resource not found")
            if cap is not None and (number is None or number > cap):
                return err(rid, -32001, "resource not found")
            w = float(ctl.get("too_soon_window_s", 0))
            if w:
                with lock:
                    seen = first_seen.setdefault(bh, time.time())
                age = time.time() - seen
                if age < w:
                    log({"method": method, "number": number, "age_s": round(age, 3), "answer": "[] (too soon)"})
                    return ok(rid, [])
        elif cap is not None:
            frm, to = flt.get("fromBlock"), flt.get("toBlock")
            if to is None or to in TAGS or int(to, 16) > cap:
                flt["toBlock"] = hex(cap)
            if frm not in (None,) and frm not in TAGS and int(frm, 16) > cap:
                return ok(rid, [])
        logs = upstream_result(method, [flt])
        topic = ctl.get("inject_after_topic")
        if topic:
            for entry in logs:
                if entry.get("topics") and entry["topics"][0].lower() == topic.lower():
                    p = int(entry["blockNumber"], 16)
                    with lock:
                        if armed["logs_block"] is None and armed.get("done") != p:
                            armed["logs_block"] = p + 1
                            armed["done"] = p
                            log({"armed_for_block": p + 1, "trigger_block": p})
        if bh or len(logs):
            log({"method": method, "filter": {k: v for k, v in flt.items() if k != "topics"}, "logs": len(logs)})
        return ok(rid, logs)

    if cap is not None and method in ("eth_getProof", "eth_call", "eth_getTransactionCount", "eth_getBalance", "eth_getCode", "eth_getStorageAt", "eth_estimateGas"):
        params = list(params)
        for i, p in enumerate(params):
            if isinstance(p, str) and p in TAGS:
                params[i] = hex(cap)
    resp = upstream(method, params)
    resp["id"] = rid
    if method == "eth_sendRawTransaction":
        log({"method": method, "forwarded": True, "result": resp.get("result"), "error": resp.get("error")})
    return resp


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        ctl = control()
        rl = ctl.get("rate_limit")
        if rl:
            now = time.time() * 1000
            with lock:
                if now - window["start"] >= rl["window_ms"]:
                    window["start"], window["count"] = now, 0
                window["count"] += 1
                over = window["count"] > rl["max"]
            if over:
                msg = b'{"error":"rate limited"}'
                self.send_response(429)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(msg)))
                self.end_headers()
                self.wfile.write(msg)
                log({"http": 429})
                return
        try:
            payload = json.loads(body)
            if isinstance(payload, list):
                out = [handle(r, ctl) for r in payload]
            else:
                out = handle(payload, ctl)
            data = json.dumps(out).encode()
            self.send_response(200)
        except Exception as e:  # report proxy faults loudly
            log({"proxy_error": repr(e)})
            data = json.dumps({"jsonrpc": "2.0", "id": None, "error": {"code": -32603, "message": repr(e)}}).encode()
            self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


ThreadingHTTPServer.daemon_threads = True
server = ThreadingHTTPServer(("127.0.0.1", LISTEN_PORT), Handler)
log({"started": LISTEN_PORT, "upstream": UPSTREAM})
server.serve_forever()
