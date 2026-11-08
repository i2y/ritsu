# A service that keeps the state of one subscription, `sub_1`, from the events posted to `/events`
# or `/subscriptions/sub_1/events`, for tests/orders.rs. Run as `python3 events.py <port> <handler>`.
# With `newest` it keeps the number of the last event it applied and drops one that is not newer;
# with `as-it-comes` it writes each event as it comes; with `hang-up` it closes the connection,
# answering nothing, when an event comes that is not newer. `POST /subscriptions` answers the id.
# `GET /subscription` and `GET /subscriptions/sub_1` answer the state and how many events the
# server received, so a claim can see that it talks to a new server. Each start adds its process id
# to `starts.log` beside it. It sends no Date and no Server header, so two runs answer the same
# bytes.
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port, handler = int(sys.argv[1]), sys.argv[2]
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "starts.log"), "a") as f:
    f.write("%d\n" % os.getpid())

state = {"status": "none", "seq": 0, "received": 0}


class H(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, code, body):
        data = json.dumps(body, sort_keys=True).encode()
        self.send_response_only(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path in ("/subscription", "/subscriptions/sub_1"):
            self.send(200, state)
        else:
            self.send(404, {"error": "not found"})

    def do_POST(self):
        if self.path == "/subscriptions":
            self.send(201, {"id": "sub_1"})
            return
        if self.path not in ("/events", "/subscriptions/sub_1/events"):
            self.send(404, {"error": "not found"})
            return
        event = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        state["received"] += 1
        if event["seq"] <= state["seq"] and handler != "as-it-comes":
            if handler == "hang-up":
                self.close_connection = True
                return
            self.send(200, {"applied": False, "subscription": "sub_1"})  # an older event, or the same one again
            return
        state["status"], state["seq"] = event["status"], event["seq"]
        self.send(200, {"applied": True, "subscription": "sub_1"})


HTTPServer(("127.0.0.1", port), H).serve_forever()
