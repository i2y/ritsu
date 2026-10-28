# A small job service. POST /jobs with a whole number starts a job that squares
# it; the service answers at once, and the job finishes half a second later, as
# work handed to a queue does. GET /jobs/<id> answers the job's state, and its
# result once it is done. Run as `python3 server.py <port>`.
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

TAKES = 0.5  # how long a job runs, in seconds
jobs = {}


class Handler(BaseHTTPRequestHandler):
    def answer(self, status, value):
        body = json.dumps(value).encode()
        self.send_response_only(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if self.path != "/jobs":
            self.answer(404, {"error": "no such path"})
            return
        text = self.rfile.read(int(self.headers.get("Content-Length") or 0)).decode()
        if not text.strip().isdigit():
            self.answer(400, {"error": "the body is not a whole number"})
            return
        job = str(len(jobs) + 1)
        jobs[job] = {"n": int(text), "ends": time.monotonic() + TAKES}
        self.answer(202, {"id": job, "state": "running"})

    def do_GET(self):
        job = self.path.removeprefix("/jobs/")
        if job not in jobs or not self.path.startswith("/jobs/"):
            self.answer(404, {"error": "no such job"})
            return
        done = time.monotonic() >= jobs[job]["ends"]
        self.answer(200, {"id": job, "state": "done", "result": jobs[job]["n"] ** 2} if done else {"id": job, "state": "running"})

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
