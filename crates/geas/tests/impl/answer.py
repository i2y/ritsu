# A service that answers `/` with the same JSON and the same headers every time,
# `/text` with a text that is not JSON, and anything else with 404. It sends no
# Date and no Server header, so two runs answer the same bytes. Run as
# `python3 answer.py <port>`.
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

HEADERS = {
    "Content-Type": "application/json",
    "Cache-Control": "no-store",
    "X-Version": "1.2.3",
    "X-Count": " 42 ",
    "Set-Cookie": "a=1",
}

BODY = {
    "id": "a1b2c3",
    "total": 12,
    "ratio": 0.5,
    "count": "42",
    "ok": True,
    "missing": None,
    "tags": ["x", "y"],
    "message": "Hello, Ada",
}


class Handler(BaseHTTPRequestHandler):
    def send(self, status, headers, body):
        data = body.encode()
        self.send_response_only(status)
        for name, value in headers.items():
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/":
            self.send(200, HEADERS, json.dumps(BODY))
        elif self.path == "/text":
            self.send(200, {"Content-Type": "text/plain"}, "Hello, Ada\n")
        else:
            self.send(404, {"Content-Type": "text/plain"}, "not found")

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
