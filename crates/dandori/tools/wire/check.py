# Sends calls of the generated workflows through the default Transport that dandori writes in
# Python (io.py: the standard library for HTTP, and boto3) to stand-ins on this machine, and
# writes down what arrived and what the Transport gave back. The TypeScript twin is check.mjs,
# which says how: the same cases, the same server, the same results.
#
#   .venv/bin/python check.py <generated package> <cases.json> <results.json> <moto's address>

from __future__ import annotations

import asyncio
import http.client
import http.server
import importlib.util
import json
import os
import re
import sys
import threading
import urllib.parse
import urllib.request
from typing import Any

import boto3

REGION = "ap-northeast-1"
ACCOUNT = "123456789012"
CREDENTIALS = {"aws_access_key_id": "wire", "aws_secret_access_key": "wire"}

package_dir, cases_file, out_file, moto = sys.argv[1:5]
with open(cases_file, encoding="utf-8") as f:
    cases = json.load(f)

# io.py by itself: it needs nothing of the package, and boto3 only when a call first needs it
spec = importlib.util.spec_from_file_location("dandori_io", os.path.join(package_dir, "io.py"))
assert spec is not None and spec.loader is not None
io = importlib.util.module_from_spec(spec)
spec.loader.exec_module(io)

# what the case being sent scripts, and what arrived for it
now: dict[str, Any] = {}
moto_url = urllib.parse.urlsplit(moto)


def pairs(text: str) -> list[list[str]]:
    return [[k, v] for k, v in urllib.parse.parse_qsl(text, keep_blank_values=True)]


class Here(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args: Any) -> None:
        pass

    def answer(self) -> None:
        body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        url = urllib.parse.urlsplit(self.path)
        lambda_ = re.fullmatch(r"/2015-03-31/functions/([^/]+)/invocations", url.path)
        if url.path.startswith("/http/"):
            # /http/<host><path>: an HTTP task's request
            kind = self.headers.get("Content-Type") or ""
            text = body.decode("utf-8")
            now["received"] = {
                "method": self.command,
                "path": re.sub(r"^/[^/]+", "", url.path[len("/http") :]),
                "query": pairs(url.query),
                "headers": {k.lower(): v for k, v in self.headers.items()},
                "body": None if text == "" else json.loads(text) if "json" in kind else pairs(text) if "x-www-form-urlencoded" in kind else text,
                "raw": text,
                "rawQuery": f"?{url.query}" if url.query else "",
            }
            r = now["case"]["reply"]
            is_json = not isinstance(r["body"], str)
            out = (json.dumps(r["body"], ensure_ascii=False) if is_json else r["body"]).encode("utf-8")
            self.send_response(r["status"])
            self.send_header("Content-Type", "application/json" if is_json else "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            self.wfile.write(out)
        elif lambda_ and self.command == "POST":
            now["received"] = {"fn": urllib.parse.unquote(lambda_.group(1)), "invocationType": self.headers.get("X-Amz-Invocation-Type"), "payload": json.loads(body), "raw": body.decode("utf-8")}
            r = now["case"]["reply"]
            out = json.dumps(r["ok"] if "ok" in r else {"errorType": r["error"], "errorMessage": r["message"]}, ensure_ascii=False).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            if "ok" not in r:
                self.send_header("X-Amz-Function-Error", "Unhandled")
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            self.wfile.write(out)
        else:
            # any other AWS API: moto's
            c = http.client.HTTPConnection(moto_url.hostname, moto_url.port)
            headers = {k: v for k, v in self.headers.items() if k.lower() != "host"}
            c.request(self.command, self.path, body=body, headers=headers)
            m = c.getresponse()
            out = m.read()
            self.send_response(m.status)
            for k, v in m.getheaders():
                if k.lower() not in ("content-length", "transfer-encoding", "connection", "server", "date"):
                    self.send_header(k, v)
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            self.wfile.write(out)
            c.close()

    do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = answer


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Here)
threading.Thread(target=server.serve_forever, daemon=True).start()
# this server's address, in place of an HTTP task's scheme and host, and boto3's endpoint
front = f"http://127.0.0.1:{server.server_address[1]}"

transport = io.DefaultTransport(headers=lambda url: {"X-Dandori-Check": "wire"}, aws={"endpoint_url": front, "region_name": REGION, **CREDENTIALS})
sns = boto3.client("sns", endpoint_url=moto, region_name=REGION, **CREDENTIALS)
sqs = boto3.client("sqs", endpoint_url=moto, region_name=REGION, **CREDENTIALS)


async def main() -> list[dict[str, Any]]:
    results = []
    for c in cases:
        now.clear()
        now.update({"case": c, "received": None})
        messages = None
        if c["kind"] == "http":
            u = urllib.parse.urlsplit(c["request"]["url"])
            req = {**c["request"], "url": f"{front}/http/{u.netloc}{u.path}" + (f"?{u.query}" if u.query else "")}
            returned = await transport.http(req)
        elif c["kind"] == "lambda":
            returned = await transport.lambda_(c["fn"], c["payload"])
        else:
            # moto as the case wants it: the topics and queues that exist, and a queue that listens
            urllib.request.urlopen(urllib.request.Request(f"{moto}/moto-api/reset", method="POST")).read()
            for name in c["topics"]:
                sns.create_topic(Name=name)
            for name in c["queues"]:
                sqs.create_queue(QueueName=name)
            listen = c.get("listen")
            if listen and listen.get("topic"):
                sqs.create_queue(QueueName=listen["queue"])
                sns.subscribe(
                    TopicArn=f"arn:aws:sns:{REGION}:{ACCOUNT}:{listen['topic']}",
                    Protocol="sqs",
                    Endpoint=f"arn:aws:sqs:{REGION}:{ACCOUNT}:{listen['queue']}",
                    Attributes={"RawMessageDelivery": "true"},
                )
            returned = await transport.aws(c["service"], c["action"], c["input"])
            if listen:
                got = sqs.receive_message(QueueUrl=sqs.get_queue_url(QueueName=listen["queue"])["QueueUrl"], MaxNumberOfMessages=10)
                messages = [m["Body"] for m in got.get("Messages", [])]
        results.append({"received": now["received"], "returned": returned, "messages": messages})
    return results


try:
    results = asyncio.run(main())
finally:
    server.shutdown()
with open(out_file, "w", encoding="utf-8") as f:
    json.dump(results, f, ensure_ascii=False, indent=2)
    f.write("\n")
