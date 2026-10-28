# A service whose answers change after a while, for the claims that wait with
# `within` (DESIGN §18). Run as `python3 later.py <port>`.
#
# - GET /after/<n> counts the calls to its path: before the n-th it answers
#   {"calls": k, "state": "running"}, from the n-th on {"calls": k, "state": "done"}.
#   While a file named `never` is beside this one, it answers {"state": "running"}.
# - POST /jobs starts a job that is done 300 ms later and answers 202
#   {"id": "<n>", "state": "running"}; GET /jobs/<n> answers its state, worked out
#   from the time on each read: nothing runs in the background.
# - GET /never answers {"state": "running"}, always.
# - GET /gone/<n> answers {"calls": k, "state": "running"} until its n-th call,
#   which ends the process without an answer.
import json
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
calls = {}
jobs = {}


class Handler(BaseHTTPRequestHandler):
    def answer(self, status, value):
        body = json.dumps(value).encode()
        self.send_response_only(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        calls[self.path] = calls.get(self.path, 0) + 1
        k = calls[self.path]
        parts = self.path.strip("/").split("/")
        if parts[0] == "after" and len(parts) == 2 and os.path.exists(os.path.join(HERE, "never")):
            self.answer(200, {"state": "running"})
        elif parts[0] == "after" and len(parts) == 2:
            self.answer(200, {"calls": k, "state": "done" if k >= int(parts[1]) else "running"})
        elif parts[0] == "jobs" and len(parts) == 2 and parts[1] in jobs:
            done = time.monotonic() >= jobs[parts[1]]
            self.answer(200, {"id": parts[1], "state": "done" if done else "running"})
        elif parts == ["never"]:
            self.answer(200, {"state": "running"})
        elif parts[0] == "gone" and len(parts) == 2:
            if k >= int(parts[1]):
                os._exit(3)
            self.answer(200, {"calls": k, "state": "running"})
        else:
            self.answer(404, {"error": "not found"})

    def do_POST(self):
        if self.path == "/jobs":
            n = str(len(jobs) + 1)
            jobs[n] = time.monotonic() + 0.3
            self.answer(202, {"id": n, "state": "running"})
        else:
            self.answer(404, {"error": "not found"})

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
