# The service behind the test page (tests/pages/app/index.html): the page at `/`,
# a shopping list at `/api/items` that the page and the claims both call (GET
# reads it, POST adds the body as an item), `/slow`, which never answers, and
# `/broken`, which Chrome refuses to load.
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PAGE = (Path(__file__).parent / "index.html").read_bytes()
items = []


class Handler(BaseHTTPRequestHandler):
    def send(self, status, kind, body):
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/":
            self.send(200, "text/html; charset=utf-8", PAGE)
        elif self.path == "/api/items":
            self.send(200, "application/json", json.dumps(items).encode())
        elif self.path == "/slow":
            time.sleep(60)
        elif self.path == "/broken":
            # two lengths for one body, which Chrome refuses to load
            self.wfile.write(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello!")
            self.close_connection = True
        else:
            self.send(404, "text/plain", b"not found")

    def do_POST(self):
        if self.path == "/api/items":
            n = int(self.headers.get("Content-Length", "0"))
            items.append(self.rfile.read(n).decode())
            self.send(200, "application/json", json.dumps(items).encode())
        else:
            self.send(404, "text/plain", b"not found")

    def log_message(self, *args):
        pass


ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
