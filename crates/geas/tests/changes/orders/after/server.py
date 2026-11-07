# A small order service in Python's standard library, the one examples/orders/orders.geas holds to
# api.yaml. Run as `python3 server.py <port>`.
#
# POST /orders with a whole number makes an order of that total and answers it, with where it is;
# GET /orders/<id> answers the order, or 404.
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

orders = {}


class Handler(BaseHTTPRequestHandler):
    def answer(self, status, body, location=None):
        data = json.dumps(body).encode()
        self.send_response_only(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        if location:
            self.send_header("Location", location)
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        text = self.rfile.read(length).decode()
        if not text.isdigit():
            return self.answer(400, {"error": "the total is a whole number"})
        id = str(len(orders) + 1)
        orders[id] = {"id": id, "state": "open", "total": text, "placedAt": "2026-11-07"}
        self.answer(201, orders[id], location=f"/orders/{id}")

    def do_GET(self):
        order = orders.get(self.path.rsplit("/", 1)[-1])
        if order:
            self.answer(200, order)
        else:
            self.answer(404, {"error": "no such order"})

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
