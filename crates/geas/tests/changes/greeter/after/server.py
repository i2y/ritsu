import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs

total = 0


class H(BaseHTTPRequestHandler):
    def _send(self, code, body, ctype="text/plain"):
        data = body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/greet":
            name = parse_qs(u.query).get("name", [""])[0].strip()
            if not name:
                self._send(400, "name required")
                return
            self._send(200, json.dumps({"message": f"Hello, {name}"}),
                       "application/json")
        elif u.path == "/total":
            self._send(200, str(total))
        elif u.path == "/health":
            self._send(200, "ok")
        else:
            self._send(404, "not found")

    def do_POST(self):
        global total
        u = urlparse(self.path)
        n = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(n).decode() if n else ""
        if u.path == "/reset":
            total = 0
            self._send(200, "ok")
        elif u.path == "/add":
            total += int(body)
            self._send(200, "ok")
        else:
            self._send(404, "not found")


port = int(sys.argv[1]) if len(sys.argv) > 1 else 8123
HTTPServer(("127.0.0.1", port), H).serve_forever()
