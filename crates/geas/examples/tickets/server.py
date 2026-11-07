# A small ticket service in Python's standard library, the one examples/tickets/tickets.geas holds to
# api.yaml. Run as `python3 server.py <port>`, with the token its callers show in TICKETS_TOKEN.
#
# It numbers what it makes itself, differently on every run. Every call shows the token, as
# `Authorization: Bearer <token>`, or is answered 401.
#
# POST /tickets makes a ticket and answers it, with where it is. GET /tickets/<id> answers it,
# PUT /tickets/<id> replaces it, PATCH /tickets/<id> changes the members it is sent, and
# DELETE /tickets/<id> takes it away. A ticket that is not there is answered 404.
import json
import os
import sys
import uuid
from http.server import BaseHTTPRequestHandler, HTTPServer

TOKEN = os.environ.get("TICKETS_TOKEN")
tickets = {}


class Handler(BaseHTTPRequestHandler):
    def answer(self, status, body=None, location=None):
        data = b"" if body is None else json.dumps(body).encode()
        self.send_response_only(status)
        if body is not None:
            self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        if location:
            self.send_header("Location", location)
        self.end_headers()
        self.wfile.write(data)

    def sent(self):
        length = int(self.headers.get("Content-Length", "0"))
        return json.loads(self.rfile.read(length) or b"{}")

    def allowed(self):
        if TOKEN and self.headers.get("Authorization") == f"Bearer {TOKEN}":
            return True
        self.answer(401, {"error": "show the token"})
        return False

    def ticket(self):
        ticket = tickets.get(self.path.rsplit("/", 1)[-1])
        if ticket is None:
            self.answer(404, {"error": "no such ticket"})
        return ticket

    def do_POST(self):
        if self.allowed():
            id = str(uuid.uuid4())
            tickets[id] = {"id": id, "state": "open", **self.sent()}
            self.answer(201, tickets[id], location=f"/tickets/{id}")

    def do_GET(self):
        if self.allowed() and (ticket := self.ticket()):
            self.answer(200, ticket)

    def do_PUT(self):
        if self.allowed() and (ticket := self.ticket()):
            tickets[ticket["id"]] = {"id": ticket["id"], **self.sent()}
            self.answer(200, tickets[ticket["id"]])

    def do_PATCH(self):
        if self.allowed() and (ticket := self.ticket()):
            ticket.update(self.sent())
            self.answer(200, ticket)

    def do_DELETE(self):
        if self.allowed() and (ticket := self.ticket()):
            del tickets[ticket["id"]]
            self.answer(204)

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
