# The same service with a handler that writes each event as it comes. It holds when the events
# come in the order they happened, and ends in the state of an older event when that one comes
# last.
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

subscription = {"status": "none", "seq": 0}


class H(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def _send(self, code, body):
        data = json.dumps(body).encode()
        # no Date and no Server header, so two runs answer the same bytes
        self.send_response_only(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/subscription":
            self._send(200, subscription)
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self):
        if self.path != "/events":
            self._send(404, {"error": "not found"})
            return
        n = int(self.headers.get("Content-Length", 0))
        event = json.loads(self.rfile.read(n))
        subscription["status"] = event["status"]
        subscription["seq"] = event["seq"]
        self._send(200, {"applied": True})


port = int(sys.argv[1]) if len(sys.argv) > 1 else 8123
HTTPServer(("127.0.0.1", port), H).serve_forever()
