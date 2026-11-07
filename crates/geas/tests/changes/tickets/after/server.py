# The ticket service after an agent renamed `id` to `ticketId`: a change the OpenAPI document does not
# allow, and one that leaves the next call with no number to go on from. Otherwise it is
# examples/tickets/server.py.
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
            tickets[id] = {"ticketId": id, "state": "open", **self.sent()}
            self.answer(201, tickets[id], location=f"/tickets/{id}")

    def do_GET(self):
        if self.allowed() and (ticket := self.ticket()):
            self.answer(200, ticket)

    def do_PUT(self):
        if self.allowed() and (ticket := self.ticket()):
            tickets[ticket["ticketId"]] = {"ticketId": ticket["ticketId"], **self.sent()}
            self.answer(200, tickets[ticket["ticketId"]])

    def do_PATCH(self):
        if self.allowed() and (ticket := self.ticket()):
            ticket.update(self.sent())
            self.answer(200, ticket)

    def do_DELETE(self):
        if self.allowed() and (ticket := self.ticket()):
            del tickets[ticket["ticketId"]]
            self.answer(204)

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
