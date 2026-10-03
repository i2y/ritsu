# A service that takes 400 ms to answer each request, and appends to `slow.log`
# beside it a line as it starts answering and one as it ends, each with its pid and
# the time, so that a test can read which claims ran at the same time. Run as
# `python3 slow.py <port>`.
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

LOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "slow.log")


def log(what):
    with open(LOG, "a") as f:
        f.write(f"{what} {os.getpid()} {time.time():.6f}\n")


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        log("start")
        time.sleep(0.4)
        log("end")
        body = b"done"
        self.send_response_only(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
