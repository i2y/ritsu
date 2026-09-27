#!/bin/sh
# Preview the built site with caching turned off, under the path it is published at.
#
# `python3 -m http.server` sends only Last-Modified, and a browser will happily keep showing
# a cached `<img>` — which is how an edited diagram can look unchanged for an hour. This
# sends `no-store` instead, so a plain reload is always enough.
#
# The built pages link to `/dandori/` absolutely, because that is where GitHub Pages serves
# them. Handing out `build/` at the root therefore breaks the header, the language switch and
# every link a page writes from the site root — so `/dandori/` is served here too, and `/`
# redirects to it.
#
# The port is 8002 unless one is given, so that rulec's preview, which takes 8001, can stay up
# beside it.
#
#   $ website/serve.sh [port]
exec python3 - "${1:-8002}" <<'PY'
import errno, functools, http.server, socketserver, sys

BASE = "/dandori"

class NoCache(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store, must-revalidate")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

    def send_head(self):
        if self.path == "/":
            self.send_response(302)
            self.send_header("Location", BASE + "/")
            self.end_headers()
            return None
        return super().send_head()

    def translate_path(self, path):
        if path == BASE or path.startswith(BASE + "/"):
            path = path[len(BASE):] or "/"
        return super().translate_path(path)

    def log_message(self, *a):
        pass

port = int(sys.argv[1])
handler = functools.partial(NoCache, directory="build")
socketserver.TCPServer.allow_reuse_address = True
try:
    httpd = socketserver.TCPServer(("", port), handler)
except OSError as e:
    if e.errno != errno.EADDRINUSE:
        raise
    sys.exit(f"port {port} is in use; give another: website/serve.sh <port>")
with httpd:
    print(f"serving build/ on http://localhost:{port}{BASE}/  (no-store)", flush=True)
    httpd.serve_forever()
PY
