"""Minimal stand-in for the sentinel engine: always returns a `secure`
verdict. The engine is a separate HTTP service (a true system boundary); the
sentinel treats any 200 `{"verdict":"secure"}` as an approve. Using a stub here
does not touch the F3-SEN-001 defect, which is the sentinel's fixed 55,000 gas
limit on its own on-chain `approve`, not anything the engine does.

Usage: python3 -I engine_stub.py <port>
"""

import sys
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(length)
        body = b'{"verdict":"secure"}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


port = int(sys.argv[1])
HTTPServer(("127.0.0.1", port), Handler).serve_forever()
