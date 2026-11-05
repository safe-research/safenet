#!/usr/bin/env python3
"""Loopback JSON-RPC stub for the F2-XC-002 re-run (QA2-XC).

Answers every JSON-RPC call (single or batched) with `result: "0x64"`, which is
enough for `Provider::connect` (`eth_chainId`) to succeed so that `main`
reaches `utils::connect_sqlite`. Nothing else is served; no real chain is
contacted. usage: stub.py <port>
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0)) or b"null"))
        reply = lambda req: {"jsonrpc": "2.0", "id": req.get("id"), "result": "0x64"}
        out = [reply(r) for r in body] if isinstance(body, list) else reply(body)
        data = json.dumps(out).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, fmt, *args):  # keep the run log clean
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
