# A small shop over HTTP: what it has, and what it costs with tax.
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

import inventory
import tax


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, status, value):
        body = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/items":
            self.send(200, {"items": inventory.names()})
        elif self.path.startswith("/price/"):
            name = self.path[len("/price/"):]
            price = inventory.price(name)
            if price is None:
                self.send(404, {"error": f"no such item: {name}"})
                return
            self.send(200, {"price": tax.with_tax(price)})
        elif self.path == "/count":
            self.send(200, {"count": len(inventory.names())})
        else:
            self.send(404, {"error": "not found"})


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
