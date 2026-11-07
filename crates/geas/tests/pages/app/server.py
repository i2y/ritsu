# The service behind the test page (tests/pages/app/index.html): the page at `/`,
# a shopping list at `/api/items` that the page and the claims both call (GET
# reads it, POST adds the body as an item), `/api/orders`, where a POST makes an
# order with a new number, `/orders/<number>`, the page of that order, `/slow`, which
# never answers (and leaves `slow.asked` beside this file once it is asked for, so
# that a test knows the page has begun to load), and `/broken`, which Chrome refuses
# to load.
import json
import sys
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PAGE = (Path(__file__).parent / "index.html").read_bytes()
items = []
orders = set()
ORDER_PAGE = """<!doctype html>
<title>Order</title>
<h1>Order {number}</h1>
<input aria-label="Note">
<button onclick="document.querySelector('p').textContent = document.querySelector('input').value">Save {number}</button>
<p role="status"></p>
"""


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
        elif self.path.startswith("/orders/") and self.path[8:] in orders:
            self.send(200, "text/html; charset=utf-8", ORDER_PAGE.format(number=self.path[8:]).encode())
        elif self.path == "/slow":
            (Path(__file__).parent / "slow.asked").touch()
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
        elif self.path == "/api/orders":
            number = str(uuid.uuid4())
            orders.add(number)
            self.send(201, "application/json", json.dumps({"id": number}).encode())
        else:
            self.send(404, "text/plain", b"not found")

    def log_message(self, *args):
        pass


ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
