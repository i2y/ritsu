# A service that numbers what it makes itself, for the claims of DESIGN §22. Run as
# `python3 things.py <port>`.
#
# - POST /things makes a thing named by the body and answers 201 {"id": <a new uuid>, "name": ...,
#   "qty": 1}, with `Location: /things/<id>`. GET /things/<id> answers it, PUT replaces its name,
#   PATCH answers it with {"patched": true}, DELETE takes it away with a 204 and no body. A thing
#   that is not there is a 404.
# - POST /small makes a thing whose id is the next whole number, as a number; GET /small/<id> answers
#   it. POST /text makes one whose id is a string of digits, the same way.
# - POST /absolute makes a thing and says where it is with this service's own address, in the
#   `Location` and as `.self`; POST /no-port does the same without the port.
# - POST /elsewhere says it is on another host.
# - GET /gaps answers what `keep` cannot read: `.none` is null, `.empty` is "", and the header
#   `x-empty` is empty. GET /plain answers a text that is not JSON.
# - GET /later/<n> answers {"calls": k, "result": null} before its n-th call, and from then on
#   {"calls": k, "result": "<a uuid made once>", "state": "done"}.
# - GET /seen/<anything> answers the path as it came.
import json
import sys
import uuid
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = int(sys.argv[1])
things = {}
small = {}
calls = {}
result = str(uuid.uuid4())


class Handler(BaseHTTPRequestHandler):
    def answer(self, status, value=None, headers=()):
        data = b"" if value is None else json.dumps(value).encode()
        self.send_response_only(status)
        if value is not None:
            self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        for k, v in headers:
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(data)

    def sent(self):
        return self.rfile.read(int(self.headers.get("Content-Length", "0"))).decode()

    def make(self):
        id = str(uuid.uuid4())
        things[id] = {"id": id, "name": self.sent(), "qty": 1}
        return id

    def do_POST(self):
        if self.path == "/things":
            id = self.make()
            self.answer(201, things[id], [("Location", f"/things/{id}")])
        elif self.path in ("/small", "/text"):
            self.sent()
            id = len(small) + 1
            shown = id if self.path == "/small" else str(id)
            small[str(id)] = {"id": shown, "qty": 1, "note": f"thing {id} of v1"}
            self.answer(201, small[str(id)], [("Location", f"/small/{id}")])
        elif self.path == "/absolute":
            id = self.make()
            at = f"http://127.0.0.1:{PORT}/things/{id}"
            self.answer(201, {**things[id], "self": at}, [("Location", at)])
        elif self.path == "/no-port":
            id = self.make()
            self.answer(201, things[id], [("Location", f"http://127.0.0.1/things/{id}")])
        elif self.path == "/elsewhere":
            id = self.make()
            self.answer(201, things[id], [("Location", f"http://example.com/things/{id}")])
        else:
            self.answer(404, {"error": "no such path"})

    def thing(self):
        return things.get(self.path.rsplit("/", 1)[-1])

    def do_GET(self):
        parts = self.path.strip("/").split("/")
        calls[self.path] = calls.get(self.path, 0) + 1
        if parts[0] == "things" and self.thing():
            self.answer(200, self.thing())
        elif parts[0] == "small" and len(parts) == 2 and parts[1] in small:
            self.answer(200, small[parts[1]])
        elif parts == ["gaps"]:
            self.answer(200, {"none": None, "empty": ""}, [("x-empty", "")])
        elif parts == ["plain"]:
            self.send_response_only(200)
            self.send_header("Content-Length", "5")
            self.end_headers()
            self.wfile.write(b"plain")
        elif parts[0] == "later":
            k = calls[self.path]
            if k >= int(parts[1]):
                self.answer(200, {"calls": k, "result": result, "state": "done"})
            else:
                self.answer(200, {"calls": k, "result": None})
        elif parts[0] == "seen":
            self.answer(200, {"path": self.path})
        else:
            self.answer(404, {"error": "no such thing"})

    def do_PUT(self):
        thing = self.thing()
        if thing:
            thing["name"] = self.sent()
            self.answer(200, thing)
        else:
            self.answer(404, {"error": "no such thing"})

    def do_PATCH(self):
        thing = self.thing()
        if thing:
            self.answer(200, {**thing, "patched": True})
        else:
            self.answer(404, {"error": "no such thing"})

    def do_DELETE(self):
        thing = self.thing()
        if thing:
            del things[thing["id"]]
            self.answer(204)
        else:
            self.answer(404, {"error": "no such thing"})

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
