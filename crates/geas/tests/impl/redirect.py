# A service that answers `/old` by sending the client to `/new` on its own address,
# `http://127.0.0.1:<port>/new`, and says so in its body as `localhost:<port>/new`;
# `/new` answers `here`. It appends the port it was given to `ports.log` beside it.
# It sends no Date and no Server header. Run as `python3 redirect.py <port>`.
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port = int(sys.argv[1])
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "ports.log"), "a") as f:
    f.write(f"{port}\n")


class Handler(BaseHTTPRequestHandler):
    def send(self, status, headers, body):
        data = body.encode()
        self.send_response_only(status)
        for name, value in headers.items():
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/old":
            self.send(302, {"Location": f"http://127.0.0.1:{port}/new"}, f"moved to localhost:{port}/new")
        elif self.path == "/new":
            self.send(200, {}, "here")
        else:
            self.send(404, {}, "not found")

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", port), Handler).serve_forever()
