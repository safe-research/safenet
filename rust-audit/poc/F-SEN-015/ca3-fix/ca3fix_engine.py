#!/usr/bin/env python3
"""Scripted sentinel engine for the CA3-FIX F-SEN-015 end-to-end test.

Usage: ca3fix_engine.py <port> <state-dir>

Reads `<state-dir>/mode` on every request:
  gated  answer {"verdict":"insecure","rule":"R-2.1"} once `<state-dir>/release`
         contains "1" (polled every 20 ms, at most 7 s)
  flip   answer {"verdict":"insecure","rule":"R-3.4"} at once
  down   answer HTTP 503 at once (an engine that is still starting)
Every request is appended to `<state-dir>/requests.log` twice, as one JSON line
when it arrives and one when it is answered (wall time, `x-request-id`, mode,
response).
"""
import http.server
import json
import pathlib
import sys
import time

PORT = int(sys.argv[1])
STATE = pathlib.Path(sys.argv[2])


def read(name, default=""):
    try:
        return (STATE / name).read_text().strip()
    except FileNotFoundError:
        return default


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(length)
        request_id = self.headers.get("x-request-id", "")
        mode = read("mode", "gated")
        with (STATE / "requests.log").open("a") as log:
            log.write(json.dumps({"t": time.time(), "request_id": request_id, "mode": mode, "event": "arrived"}) + "\n")
        status, body = 200, ""
        if mode == "gated":
            deadline = time.time() + 7
            while read("release", "0") != "1" and time.time() < deadline:
                time.sleep(0.02)
            body = json.dumps({"verdict": "insecure", "rule": "R-2.1"})
        elif mode == "flip":
            body = json.dumps({"verdict": "insecure", "rule": "R-3.4"})
        else:
            status, body = 503, "starting"
        with (STATE / "requests.log").open("a") as log:
            log.write(json.dumps({"t": time.time(), "request_id": request_id, "mode": mode, "event": "answered", "status": status, "body": body}) + "\n")
        payload = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
