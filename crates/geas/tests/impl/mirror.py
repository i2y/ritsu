# A service that answers with what it was sent, for the claims of DESIGN §22. Run as
# `python3 mirror.py <port>`.
#
# Every call of any of the five methods is answered 200 with {"method": ..., "path": ...,
# "headers": [[name, value], ...], "body": ...}, the headers as they came, in the order they came,
# without the three geas always sends. A path under /saved/ answers the same and writes it to a file
# beside this one, `saved.jsonl`, one line a call. Whatever came in `x-echo` goes back in the header
# `x-echoed`, and the body of a POST to /plain goes back as a plain text.
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
OWN = {"host", "connection", "content-length"}


class Handler(BaseHTTPRequestHandler):
    def respond(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length).decode()
        headers = [[k, v] for k, v in self.headers.items() if k.lower() not in OWN]
        seen = {"method": self.command, "path": self.path, "headers": headers, "body": body}
        if self.path.startswith("/saved/"):
            with open(os.path.join(HERE, "saved.jsonl"), "a") as f:
                f.write(json.dumps(seen) + "\n")
        data = body.encode() if self.path == "/plain" else json.dumps(seen).encode()
        self.send_response_only(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        for k, v in headers:
            if k.lower() == "x-echo":
                self.send_header("x-echoed", v)
        self.end_headers()
        self.wfile.write(data)

    do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = respond

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
