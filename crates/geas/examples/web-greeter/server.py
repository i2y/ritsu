# The greeter's service: the page at `/`, and the names it has greeted at
# `/api/greetings`, which the page and the claims both call. GET reads them;
# POST greets the name in the body and keeps it, or refuses an empty one.
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PAGE = (Path(__file__).parent / "index.html").read_bytes()
names = []


class Handler(BaseHTTPRequestHandler):
    def send(self, status, kind, body):
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_json(self, status, value):
        self.send(status, "application/json", json.dumps(value).encode())

    def do_GET(self):
        if self.path == "/":
            self.send(200, "text/html; charset=utf-8", PAGE)
        elif self.path == "/api/greetings":
            self.send_json(200, {"names": names})
        else:
            self.send(404, "text/plain", b"not found")

    def do_POST(self):
        if self.path != "/api/greetings":
            self.send(404, "text/plain", b"not found")
            return
        n = int(self.headers.get("Content-Length", "0"))
        name = self.rfile.read(n).decode().strip()
        if not name:
            self.send_json(400, {"error": "a name is required"})
            return
        names.append(name)
        self.send_json(201, {"names": names})

    def log_message(self, *args):
        pass


ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
