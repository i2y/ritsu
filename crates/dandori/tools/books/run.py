"""Runs the operations of books of chobo's through the default Transport that dandori writes in
Python (io.py's `book`), and through the Lambda function it writes for Step Functions
(lambda/book_<book>_handler.py), on the Python clients chobo writes, against PostgreSQL or
TigerBeetle, for tests/examples.rs (books_run_on_postgres_and_tigerbeetle). The TypeScript twin is
run.ts, which says what goes in and what comes out; a run here answers its calls twice: through
the Transport on its tenant ("answers"), and through the handler on the tenant with "/lambda" after
it ("handled": the handler's result, or the type of what it raised, "raised", which for a refusal
is the reason, as Step Functions names the error).

    <chobo>/tools/runner/.venv/bin/python -B run.py <input.json>

The Python is chobo's tools/runner's, which has psycopg and tigerbeetle; -B keeps it from writing
bytecode into that venv.
"""

from __future__ import annotations

import asyncio
import importlib.util
import json
import sys
import traceback
from typing import Any

import psycopg
import tigerbeetle as tb

with open(sys.argv[1], encoding="utf-8") as f:
    INPUT = json.load(f)

_loaded: dict[str, Any] = {}


def load(path: str) -> Any:
    """A module by its file, once (io.py would hide the standard library's io by its name)."""
    if path not in _loaded:
        name = f"dandori_books_{len(_loaded)}"
        spec = importlib.util.spec_from_file_location(name, path)
        assert spec is not None and spec.loader is not None
        mod = importlib.util.module_from_spec(spec)
        # dataclasses look the module up while they are made
        sys.modules[name] = mod
        spec.loader.exec_module(mod)
        _loaded[path] = mod
    return _loaded[path]


async def main() -> None:
    conn: Any = None
    client: Any = None
    if INPUT["backend"] == "postgres":
        c = INPUT["postgres"]
        conn = psycopg.connect(host=c["host"], port=c["port"], dbname=c["database"], user=c["user"], autocommit=True)
    else:
        c = INPUT["tigerbeetle"]
        client = tb.ClientSync(cluster_id=int(c["cluster"]), replica_addresses=",".join(c["addresses"]))

    def book(path: str, tenant: str) -> Any:
        mod = load(path)
        return mod.postgres(conn, tenant) if conn is not None else mod.tigerbeetle(client, tenant)

    try:
        runs = []
        for r in INPUT["runs"]:
            io = load(r["io"])
            transport = io.transport(books={name: book(path, r["tenant"]) for name, path in r["clients"].items()})
            answers = []
            for call in r["calls"]:
                try:
                    answers.append(await transport.book(call))
                except Exception:
                    answers.append({"error": traceback.format_exc()})
            handlers = {name: (load(path), load(path).make_handler(lambda p=r["clients"][name], t=r["tenant"] + "/lambda": book(p, t))) for name, path in r.get("handlers", {}).items()}
            handled = []
            for call in r["calls"] if handlers else []:
                # what the state machine invokes the book's function with: the call without the book
                event = {k: v for k, v in call.items() if k != "book"}
                module, handler = handlers[call["book"]]
                try:
                    handled.append(handler(event, None))
                except Exception as e:
                    # a refusal is raised as a type of the handler's own, named by the reason
                    refused = type(e).__module__ == module.__name__
                    handled.append({"raised": type(e).__name__} if refused else {"error": traceback.format_exc()})
            runs.append({"answers": answers, "handled": handled})
        sys.stdout.write(json.dumps({"runs": runs}, ensure_ascii=False) + "\n")
    finally:
        if conn is not None:
            conn.close()
        if client is not None:
            client.close()


asyncio.run(main())
