# A service for the claims that hold its answers to a contract (DESIGN §20). Run as
# `python3 orders.py <port>`. GET /orders/<id> answers as its id says:
#
# - ok: an order the contract allows
# - state: a state the contract does not list
# - total: no total, which the contract requires
# - many: three things wrong at once, and one more
# - secret: a member the contract says only goes to the service
# - teapot: a status the contract does not list
# - text: a body of a media type the contract does not list
# - broken: a body that is not JSON
# - later: state as `state` says until its third call, and then `ok`
#
# POST /orders answers 201 without the Location the contract requires when its body is `bare`.
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

calls = {}


class Handler(BaseHTTPRequestHandler):
    def answer(self, status, body, media="application/json", extra=()):
        data = body.encode()
        self.send_response_only(status)
        self.send_header("Content-Type", media)
        self.send_header("Content-Length", str(len(data)))
        for k, v in extra:
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(data)

    def order(self, id, **over):
        o = {"id": id, "state": "open", "total": 100, "placed": "2026-11-07"}
        o.update(over)
        return json.dumps({k: v for k, v in o.items() if v is not None})

    def do_GET(self):
        id = self.path.split("?")[0].rsplit("/", 1)[-1]
        calls[id] = calls.get(id, 0) + 1
        if id == "state":
            self.answer(200, self.order(id, state="lost"))
        elif id == "total":
            self.answer(200, self.order(id, total=None))
        elif id == "many":
            self.answer(200, self.order(id, state="lost", total="a", placed="yesterday", extra=1))
        elif id == "secret":
            self.answer(200, self.order(id, card="4242"))
        elif id == "teapot":
            self.answer(418, "short and stout", media="text/plain")
        elif id == "text":
            self.answer(200, "an order", media="text/plain")
        elif id == "broken":
            self.answer(200, "<html>")
        elif id == "later":
            self.answer(200, self.order(id, state="lost" if calls[id] < 3 else "open"))
        else:
            self.answer(200, self.order(id))

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length).decode()
        extra = [] if body == "bare" else [("Location", "/orders/1")]
        self.answer(201, self.order("1"), extra=extra)

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
