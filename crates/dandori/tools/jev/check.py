# Sends decision tasks' requests for real — to TypeSafe's API, OpenAI's Decisions API, or a server
# of the System One API such as Ollama — through the default Transport that dandori writes for
# Python (io.py), and reads each answer with io.jev, as the generated activities do. The Transport
# adds the key the case names ("typesafe", "openai", or none; TypeSafe's when left out). The Python
# twin of check.mjs; it needs nothing but the standard library.
#
#   python3 tools/jev/check.py <io.py> <cases.json> <results.json>

from __future__ import annotations

import asyncio
import importlib.util
import json
import sys
import time


class Failed(Exception):
    def __init__(self, kind: str, message: str) -> None:
        super().__init__(message)
        self.kind = kind
        self.message = message


def fail(kind: str, message: str) -> Failed:
    return Failed(kind, message)


async def main() -> None:
    io_file, cases_file, out_file = sys.argv[1:4]
    spec = importlib.util.spec_from_file_location("dandori_io", io_file)
    io = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(io)
    transport = io.transport()
    with open(cases_file, encoding="utf-8") as f:
        cases = json.load(f)
    results = []
    for c in cases:
        began = time.monotonic()
        key = c.get("key", "typesafe")
        res = await transport.http({**c["request"], **({key: True} if key else {})})
        r = {"status": res["status"], "body": res["body"], "ms": round((time.monotonic() - began) * 1000)}
        if 200 <= res["status"] < 300:
            try:
                r["value"] = io.jev(res["body"], c["spec"], fail)
            except Failed as e:
                r["error"] = {"kind": e.kind, "message": e.message}
        results.append(r)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")


asyncio.run(main())
