#!/bin/sh
# Preview the built site with caching turned off.
#
# `python3 -m http.server` sends only Last-Modified, and a browser will happily keep showing
# a cached `<img>` — which is how an edited diagram can look unchanged for an hour. This
# sends `no-store` instead, so a plain reload is always enough.
#
#   $ website/serve.sh [port]
exec python3 - "${1:-8001}" <<'PY'
import functools, http.server, socketserver, sys

class NoCache(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store, must-revalidate")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

    def log_message(self, *a):
        pass

port = int(sys.argv[1])
handler = functools.partial(NoCache, directory="build")
socketserver.TCPServer.allow_reuse_address = True
with socketserver.TCPServer(("", port), handler) as httpd:
    print(f"serving build/ on http://localhost:{port}/  (no-store)", flush=True)
    httpd.serve_forever()
PY
