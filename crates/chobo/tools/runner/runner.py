"""Runs scenarios through the Python clients chobo writes, for tests/backends.rs:

    .venv/bin/python runner.py <input.json>

The input and the output are those of runner.ts (see there). Every scenario runs in a thread of
its own; the callers of a `together` run in threads of their own too, each through a connection
(or a TigerBeetle client) of its own.
"""

from __future__ import annotations

import importlib.util
import itertools
import json
import queue
import sys
import threading
import time
import traceback
from concurrent.futures import ThreadPoolExecutor
from typing import Any

import psycopg
import tigerbeetle as tb

INPUT = json.load(open(sys.argv[1], encoding="utf-8"))

ACCOUNT_FLAGS = ["linked", "debits_must_not_exceed_credits", "credits_must_not_exceed_debits", "history", "imported", "closed"]
TRANSFER_FLAGS = ["linked", "pending", "post_pending_transfer", "void_pending_transfer", "balancing_debit", "balancing_credit", "closing_debit", "closing_credit", "imported"]


def hex32(v: int) -> str:
    return format(v, "032x")


def flag_names(names: list, bits: int) -> list:
    return [n for i, n in enumerate(names) if int(bits) & (1 << i)]


def account_json(a: tb.Account) -> dict:
    return {
        "id": hex32(a.id),
        "ledger": a.ledger,
        "code": a.code,
        "flags": flag_names(ACCOUNT_FLAGS, a.flags),
        "user_data_128": hex32(a.user_data_128),
        "user_data_64": a.user_data_64,
        "user_data_32": a.user_data_32,
    }


def transfer_json(x: tb.Transfer) -> dict:
    return {
        "id": hex32(x.id),
        "debit_account_id": hex32(x.debit_account_id),
        "credit_account_id": hex32(x.credit_account_id),
        "amount": str(x.amount),
        "pending_id": hex32(x.pending_id),
        "user_data_128": hex32(x.user_data_128),
        "user_data_64": x.user_data_64,
        "user_data_32": x.user_data_32,
        "timeout": x.timeout,
        "ledger": x.ledger,
        "code": x.code,
        "flags": flag_names(TRANSFER_FLAGS, x.flags),
    }


def param(v: Any) -> Any:
    """A value the clients pass PostgreSQL, as the test compares it: an amount as its decimal digits."""
    return str(v) if isinstance(v, int) and not isinstance(v, bool) else v


class Recorder:
    """The requests of one operation, written down as the client sends them."""

    def __init__(self) -> None:
        self.current: list | None = None


class Pool:
    """Connections to PostgreSQL, each in autocommit: a call borrows one for its cursor."""

    def __init__(self, conninfo: dict, size: int) -> None:
        self._free: queue.Queue = queue.Queue()
        for _ in range(size):
            self._free.put(psycopg.connect(**conninfo, autocommit=True))

    def borrow(self) -> Any:
        return self._free.get()

    def give_back(self, conn: Any) -> None:
        self._free.put(conn)

    def close(self) -> None:
        while not self._free.empty():
            self._free.get().close()


class _Cursor:
    def __init__(self, pool: Pool, rec: Recorder) -> None:
        self._pool, self._rec = pool, rec

    def __enter__(self) -> "_Cursor":
        self._conn = self._pool.borrow()
        self._cur = self._conn.cursor()
        return self

    def __exit__(self, *exc: Any) -> None:
        self._cur.close()
        self._pool.give_back(self._conn)

    def execute(self, sql: str, params: list) -> None:
        if self._rec.current is not None:
            self._rec.current.append({"sql": sql, "params": [param(p) for p in params]})
        self._cur.execute(sql, params)

    def fetchone(self) -> Any:
        return self._cur.fetchone()


class PgConnection:
    """What the client takes as a DB-API connection: each cursor borrows from the pool."""

    def __init__(self, pool: Pool, rec: Recorder) -> None:
        self._pool, self._rec = pool, rec

    def cursor(self) -> _Cursor:
        return _Cursor(self._pool, self._rec)


class TbClient:
    def __init__(self, client: Any, rec: Recorder) -> None:
        self._c, self._rec = client, rec

    def _note(self, what: dict) -> None:
        if self._rec.current is not None:
            self._rec.current.append(what)

    def create_accounts(self, batch: list) -> list:
        self._note({"create_accounts": [account_json(a) for a in batch]})
        return self._c.create_accounts(batch)

    def create_transfers(self, batch: list) -> list:
        self._note({"create_transfers": [transfer_json(x) for x in batch]})
        return self._c.create_transfers(batch)

    def lookup_accounts(self, ids: list) -> list:
        self._note({"lookup_accounts": [hex32(i) for i in ids]})
        return self._c.lookup_accounts(ids)

    def lookup_transfers(self, ids: list) -> list:
        self._note({"lookup_transfers": [hex32(i) for i in ids]})
        return self._c.lookup_transfers(ids)


POOL: Pool | None = None
EXPIRING = threading.Lock()
CLIENTS: list = []
_next = itertools.count()


def connection(rec: Recorder) -> Any:
    if INPUT["backend"] == "postgres":
        return PgConnection(POOL, rec)
    return TbClient(CLIENTS[next(_next) % len(CLIENTS)], rec)


def run_scenario(mod: Any, book: dict, sc: dict) -> dict:
    factory = mod.postgres if INPUT["backend"] == "postgres" else mod.tigerbeetle
    sent: list = []
    sent_lock = threading.Lock()
    done_holds: list = []
    # on TigerBeetle: what each hold of the scenario still holds, by the hold's kind and key
    pending: dict = {}
    debited: set = set()

    def hold_key(kind: str, args: dict) -> str:
        return kind + json.dumps([args[k] for k in book["keys"][kind]], ensure_ascii=False)

    def caller() -> tuple:
        rec = Recorder()
        return rec, factory(connection(rec), tenant=sc["tenant"])

    main = caller()

    def call(c: tuple, op: dict, step: int, caller_no: int | None) -> dict:
        rec, b = c
        requests: list = []
        rec.current = requests
        try:
            calls = getattr(b, op["kind"])
            if op["op"] == "do":
                r = calls.do(**op["args"])
            elif op["op"] == "hold":
                r = calls.hold(**op["args"])
            elif op["op"] == "post":
                r = calls.post(**op["args"], **op.get("amounts", {}))
            else:
                r = calls.void(**op["args"])
        finally:
            rec.current = None
        at = {"step": step}
        if caller_no is not None:
            at["caller"] = caller_no
        with sent_lock:
            sent.append({**at, "op": op["op"], "kind": op["kind"], "requests": requests})
            if r.result == "done" and op["op"] == "hold":
                done_holds.append(time.monotonic())
                chains = [q["create_transfers"] for q in requests if "create_transfers" in q]
                ps = [(x["debit_account_id"], int(x["amount"]), x["timeout"]) for x in (chains[-1] if chains else []) if "pending" in x["flags"] and x["user_data_32"] != 2]
                debited.update(p[0] for p in ps)
                pending[hold_key(op["kind"], op["args"])] = ps
            if r.result == "done" and op["op"] in ("post", "void"):
                pending.pop(hold_key(op["kind"], op["args"]), None)
        out = {"op": op["op"], "kind": op["kind"], "result": r.result}
        if r.result == "refused":
            out["reason"] = r.reason
        return out

    def pass_() -> None:
        until = max(done_holds, default=0) + book["expiry"] + 0.3
        time.sleep(max(0.0, until - time.monotonic()))
        if INPUT["backend"] == "postgres":
            # one expire() at a time, as one job would call it: an expire() skips the holds another
            # is giving back, and returns before that one commits
            with EXPIRING:
                main[1].expire()
            return
        for k in list(pending):
            left = [p for p in pending[k] if p[2] == 0]
            if not left:
                del pending[k]
            else:
                pending[k] = left
        want = {d: 0 for d in debited}
        for ps in pending.values():
            for d, amount, _ in ps:
                want[d] += amount
        ids = list(debited)
        for tries in itertools.count():
            found = CLIENTS[0].lookup_accounts([int(d, 16) for d in ids]) if ids else []
            if all(a.debits_pending == want[hex32(a.id)] for a in found):
                return
            if tries > 200:
                raise RuntimeError("the holds past their expiry were not given back within 10 seconds")
            time.sleep(0.05)

    steps: list = []
    for i, step in enumerate(sc["steps"]):
        if step["op"] == "pass":
            pass_()
            steps.append({"op": "pass"})
        elif step["op"] == "together":
            callers = [caller() for _ in step["callers"]]
            outs: list = [None] * len(callers)

            def run_caller(ci: int) -> None:
                outs[ci] = [call(callers[ci], op, i + 1, ci + 1) for op in step["callers"][ci]]

            threads = [threading.Thread(target=run_caller, args=(ci,)) for ci in range(len(callers))]
            for t in threads:
                t.start()
            for t in threads:
                t.join()
            if any(o is None for o in outs):
                raise RuntimeError("a caller of together failed")
            steps.append({"op": "together", "callers": outs})
        else:
            steps.append(call(main, step, i + 1, None))
    accounts = []
    for a in sc["accounts"]:
        b = getattr(main[1].balance, a["account"])(**a["params"])
        accounts.append({"account": a["account"], "args": a["args"], "posted": b.posted, "held_in": b.held_in, "held_out": b.held_out})
    holds = []
    for h in sc["holds"]:
        state = getattr(main[1], h["kind"]).status(**h["args"])
        if state is not None:
            holds.append({"kind": h["kind"], "key": h["key"], "state": state})
    return {"tenant": sc["tenant"], "result": {"steps": steps, "accounts": accounts, "holds": holds}, "sent": sent, "error": None}


def load(path: str) -> Any:
    name = "book_" + str(abs(hash(path)))
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    # dataclasses look the module up while they are made
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def guarded(mod: Any, book: dict, sc: dict) -> dict:
    try:
        return run_scenario(mod, book, sc)
    except Exception:
        return {"tenant": sc["tenant"], "result": None, "sent": [], "error": traceback.format_exc()}


def main() -> None:
    global POOL
    if INPUT["backend"] == "postgres":
        c = INPUT["postgres"]
        POOL = Pool({"host": c["host"], "port": c["port"], "dbname": c["database"], "user": c["user"]}, 20)
    else:
        c = INPUT["tigerbeetle"]
        for _ in range(4):
            CLIENTS.append(tb.ClientSync(cluster_id=int(c["cluster"]), replica_addresses=",".join(c["addresses"])))
    try:
        jobs = []
        with ThreadPoolExecutor(max_workers=256) as ex:
            for book in INPUT["books"]:
                mod = load(book["client"])
                jobs.append((book["name"], [ex.submit(guarded, mod, book, sc) for sc in book["scenarios"]]))
            books = [{"name": name, "scenarios": [f.result() for f in fs]} for name, fs in jobs]
        sys.stdout.write(json.dumps({"books": books}, ensure_ascii=False) + "\n")
    finally:
        if POOL is not None:
            POOL.close()
        for c in CLIENTS:
            c.close()


main()
