# Reads what a rule's Connect service answered with the code dandori writes for it in Python
# (io.py's `rule`, which sends through a Transport, here a stand-in that gives the answer), and
# writes down what it read. tests/examples.rs gives it answers the services would not give, as well as
# ones they do, and holds the reading to the reference's (`render::rule_read`).
#
#   python read.py <the directory that has io.py> <cases.json> <results.json>
#
# cases.json:   [ { "wire": <the RuleWire of the rule>, "body": <the answer> }, ... ]
# results.json: [ <what io.py's rule gives, or {"threw": …}>, ... ]

import asyncio
import importlib.util
import json
import os
import sys
from typing import Any

directory, cases_file, out_file = sys.argv[1:4]
spec = importlib.util.spec_from_file_location("dandori_io", os.path.join(directory, "io.py"))
assert spec is not None and spec.loader is not None
io = importlib.util.module_from_spec(spec)
sys.modules["dandori_io"] = io
spec.loader.exec_module(io)


class Answer:
    def __init__(self, body: Any) -> None:
        self.body = body

    async def http(self, req: dict[str, Any]) -> dict[str, Any]:
        return {"status": 200, "body": self.body}


def fail(kind: str, message: str) -> Exception:
    return Exception(f"{kind}: {message}")


async def main() -> list[Any]:
    with open(cases_file, encoding="utf-8") as f:
        cases = json.load(f)
    results = []
    for c in cases:
        try:
            results.append(await io.rule(Answer(c["body"]), c["wire"], {}, fail))
        except Exception as e:  # noqa: BLE001 - what is told is that it threw
            results.append({"threw": str(e)})
    return results


with open(out_file, "w", encoding="utf-8") as f:
    json.dump(asyncio.run(main()), f, ensure_ascii=False)
