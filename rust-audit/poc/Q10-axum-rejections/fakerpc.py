import json, http.server
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get('content-length', 0))
        body = self.rfile.read(n)
        try: req = json.loads(body)
        except Exception: req = {"id": 1}
        def one(r):
            m = r.get("method")
            if m == "eth_chainId": res = "0x64"
            elif m == "net_version": res = "100"
            elif m == "eth_blockNumber": res = "0x1"
            else: res = None
            return {"jsonrpc": "2.0", "id": r.get("id", 1), "result": res}
        out = [one(r) for r in req] if isinstance(req, list) else one(req)
        data = json.dumps(out).encode()
        self.send_response(200); self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data))); self.end_headers()
        self.wfile.write(data)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", 8545), H).serve_forever()
