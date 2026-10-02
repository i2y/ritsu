# A small shop over HTTP: what it has, and what it costs with tax.
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

import old
import stock


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, status, value):
        body = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/items":
            self.send(200, {"items": stock.names()})
        elif self.path.startswith("/price/"):
            name = self.path[len("/price/"):]
            price = stock.price(name)
            if price is None:
                self.send(404, {"error": "no such item"})
                return
            self.send(200, {"price": old.with_tax(price)})
        else:
            self.send(404, {"error": "not found"})


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
