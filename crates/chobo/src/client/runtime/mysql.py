from __future__ import annotations

import hashlib
import json
import random
import re
import time
from dataclasses import dataclass
from typing import Any, Callable, Optional, Sequence, TypeVar

#@@BOOK@@
# ── What follows is the same in every book chobo writes for MySQL, TiDB and Vitess ───────
#
# A call is one transaction of the database's. The accounts it moves that have bounds are
# locked in the order of their IDs and read, and the read says, row by row, the first move
# whose bound it would break; the writes work the new balances out themselves. The SQL is the
# book's, written by chobo. This part fills in the values (IDs as hex, amounts as digits, the
# caller's strings as the hex of their UTF-8) and picks the next statement from what the last
# one answered.


@dataclass(frozen=True)
class Result:
    """What a call answers: "done", "done_before", or "refused" with the reason. A refusal is
    an answer, not an error: nothing has changed, and the reason says why."""

    result: str
    reason: Optional[str] = None


@dataclass(frozen=True)
class Balance:
    """An account's balance: what is posted, what holds put in, and what holds take out."""

    posted: int
    held_in: int
    held_out: int


_AMOUNT_LIMIT = 2**63 - 1
_ID_LIMIT = 2**128 - 1
# How many times a call of its own transaction is made when the database gives up on it: about
# 1.5 seconds of waits between them (DESIGN 4.5).
_ATTEMPTS = 30
# The errors after which such a call is made again with the same arguments (the key keeps it from
# moving twice): a deadlock, a lock not got in time, and TiDB's write conflicts.
_AGAIN = {1213, 1205, 9007, 8022, 8028}
_DUPLICATE = 1062
# The server's status in an answer, as PyMySQL keeps it: a transaction is open.
_IN_TRANS = 1
_PLACE = re.compile(r"\{([A-Z0-9_]+)\}")
_T = TypeVar("_T")


class ChoboError(Exception):
    """A book the database does not hold as written: `code` is `CB001` when an account's row
    was made with other bounds than the book's."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _id(*parts: str) -> int:
    """An ID: the first 16 bytes of a SHA-256 over the parts, each with its UTF-8 length in front."""
    h = hashlib.sha256()
    for p in ("chobo/1",) + parts:
        b = p.encode("utf-8")
        h.update(len(b).to_bytes(4, "big"))
        h.update(b)
    v = int.from_bytes(h.digest()[:16], "big")
    return 1 if v == 0 else _ID_LIMIT - 1 if v == _ID_LIMIT else v


def _hex(v: int) -> str:
    return format(v, "032x")


def _id_lit(v: int) -> str:
    """An ID as SQL."""
    return f"X'{_hex(v)}'"


def _str_lit(s: str) -> str:
    """A string of the caller's as SQL: the hex of its UTF-8, read as utf8mb4, so that nothing in it
    is ever read as SQL."""
    return "_utf8mb4 X'" + s.encode("utf-8").hex() + "'"


def _id_list(ids: Sequence[int]) -> str:
    """IDs as an SQL list: each once, in the order of their hex."""
    return ", ".join(_id_lit(i) for i in sorted(set(ids)))


def _json_string(s: str) -> str:
    """A string as JavaScript's JSON.stringify writes it (PLAN 0.3)."""
    out = ['"']
    for c in s:
        if c == '"':
            out.append('\\"')
        elif c == "\\":
            out.append("\\\\")
        elif c == "\b":
            out.append("\\b")
        elif c == "\f":
            out.append("\\f")
        elif c == "\n":
            out.append("\\n")
        elif c == "\r":
            out.append("\\r")
        elif c == "\t":
            out.append("\\t")
        elif ord(c) < 0x20:
            out.append("\\u%04x" % ord(c))
        else:
            out.append(c)
    out.append('"')
    return "".join(out)


def _json_strings(vs: Sequence[str]) -> str:
    return "[" + ",".join(_json_string(v) for v in vs) + "]"


def _fill(template: str, values: dict[str, str]) -> str:
    """Put the values in a statement: each `{NAME}` by the value of NAME, in one pass."""
    return _PLACE.sub(lambda m: values.get(m.group(1), m.group(0)), template)


def _str(name: str, v: Any) -> str:
    if not isinstance(v, str):
        raise TypeError(f"chobo: {name} is a string")
    return v


def _amt(name: str, v: Any) -> int:
    if not isinstance(v, int) or isinstance(v, bool):
        raise TypeError(f"chobo: {name} is an amount, an int")
    if v < 0 or v > _AMOUNT_LIMIT:
        raise ValueError(f"chobo: {name} is {v}, and an amount is from 0 to 2^63 - 1")
    return v


def _amounts(names: Sequence[str], values: Sequence[Any]) -> Optional[list[Any]]:
    """The amounts of a post: every one, or none (all of it)."""
    if all(v is None for v in values):
        return None
    if any(v is None for v in values):
        raise ValueError(f"chobo: post takes every amount ({', '.join(names)}) or none")
    return [_amt(n, v) for n, v in zip(names, values)]


def _refused(reason: str) -> Result:
    return Result("refused", reason)


def _content(t: dict[str, Any], v: Sequence[Any]) -> str:
    """What a do or hold is called with, as the key compares it: the arguments in the order of the parameters."""
    return "{" + ",".join(_json_string(p["name"]) + ":" + (str(v[i]) if p["amount"] else _json_string(v[i])) for i, p in enumerate(t["params"])) + "}"


def _errno(e: BaseException) -> Optional[int]:
    # PyMySQL's and mysqlclient's errors carry the number first
    first = e.args[0] if e.args else None
    return first if isinstance(first, int) else None


def _backoff(attempt: int) -> None:
    """A wait before the next try: up to 1, 2, 4 … 128 ms, at random, so that the calls that
    failed together do not meet again."""
    time.sleep(random.random() * 2 ** min(attempt - 1, 7) / 1000)


_Rows = list[tuple[Any, ...]]


class _Tx:
    """The statements of one call: in a transaction of the call's own (`own`), or in the caller's."""

    def __init__(self, conn: Any, own: bool, batch: bool, begin: str) -> None:
        self._conn, self.own, self._batch, self._begin = conn, own, batch, begin

    def _exec(self, sql: list[str]) -> list[_Rows]:
        """Run `sql` in order and answer the rows of each: one request (`batch`), or one request
        a statement, stopping at the first error."""
        if not sql:
            return []
        out: list[_Rows] = []
        cur = self._conn.cursor()
        try:
            if self._batch and len(sql) > 1:
                cur.execute("; ".join(sql))
                out.append(list(cur.fetchall() or []))
                while cur.nextset():
                    out.append(list(cur.fetchall() or []))
                return out
            for s in sql:
                cur.execute(s)
                out.append(list(cur.fetchall() or []) if cur.description else [])
            return out
        finally:
            cur.close()

    def plain(self, sql: list[str]) -> list[_Rows]:
        """Statements outside the transaction: when the call has its own, before it starts."""
        return self._exec(sql)

    def run(self, sql: list[str], begin: bool = False, commit: bool = False) -> list[_Rows]:
        """Statements of the transaction, which starts with the first of them when the call has its
        own, and ends after the last with `commit`."""
        start = [self._begin] if begin and self.own else []
        end = ["commit"] if commit and self.own else []
        rows = self._exec(start + sql + end)
        return rows[len(start) : len(start) + len(sql)]

    def quit(self) -> None:
        """The end of a call that writes nothing more: its own transaction rolled back."""
        if self.own:
            self._exec(["rollback"])


class _MySql:
    def __init__(self, book: dict[str, Any], conn: Any, tenant: str, batch: bool) -> None:
        self._book = book
        self._conn = conn
        self.tenant = _str("the tenant", tenant)
        self._batch = batch
        # the accounts this value has made sure of in a transaction of a call's own, where their rows
        # were committed at once: rows are never removed, nor their bounds changed
        self._sure: set[int] = set()
        self._base = {"TENANT": _id_lit(_id("tenant", book["name"], self.tenant)), "TENANT_NAME": _str_lit(self.tenant)}

    def _own(self) -> bool:
        """Whether a call has a transaction of its own: on a connection with autocommit on and no
        transaction open. In a transaction, one the caller began or any with autocommit off, the call
        is part of it, and the caller commits it. PyMySQL keeps the server's status of its last
        answer (`server_status`), which says whether begin() opened one; mysqlclient keeps none, so
        on its connection a transaction of the caller's is one with autocommit off."""
        get = getattr(self._conn, "get_autocommit", None)
        autocommit = bool(get()) if callable(get) else bool(getattr(self._conn, "autocommit", False))
        status = getattr(self._conn, "server_status", None)
        return autocommit and not (isinstance(status, int) and status & _IN_TRANS)

    def _tx(self, fn: Callable[[_Tx], _T]) -> _T:
        """Run `fn`: in a transaction of the call's own, made again with the same arguments when the
        database gives up on it; or in the caller's."""
        begin = self._book["sql"]["begin"]
        if not self._own():
            return fn(_Tx(self._conn, False, self._batch, begin))
        attempt = 0
        while True:
            attempt += 1
            x = _Tx(self._conn, True, self._batch, begin)
            try:
                return fn(x)
            except Exception as e:
                try:
                    x.plain(["rollback"])
                except Exception:
                    pass
                if _errno(e) in _AGAIN and attempt < _ATTEMPTS:
                    _backoff(attempt)
                    continue
                raise

    def _read(self, sql: str) -> _Rows:
        """One statement outside any transaction of the call's."""
        return self._tx(lambda x: x.plain([sql])[0])

    def _place(self, r: dict[str, Any], v: Sequence[Any]) -> tuple[int, str, list[str]]:
        args = [str(v[a["param"]]) if "param" in a else a["literal"] for a in r["args"]]
        return _id("account", self._book["name"], self.tenant, r["kind"], *args), r["kind"], args

    def _make_sure(self, x: _Tx, accounts: Sequence[tuple[int, str, list[str]]]) -> None:
        """Make sure every account is there, with the book's bounds, before a call locks the ones
        with bounds: read them, and make the missing ones. In the caller's transaction, a row made
        goes with the caller's rollback, so every call there reads them all, and what it makes is
        not taken as sure."""
        want = {}
        for a in accounts:
            if not x.own or a[0] not in self._sure:
                want[a[0]] = a
        if not want:
            return
        sql = self._book["sql"]
        found = x.plain([_fill(sql["readAccounts"], {**self._base, "IDS": _id_list(list(want))})])[0]
        there = set()
        for h, lower, upper in found:
            got = want.get(int(h, 16))
            if got is None:
                continue
            d = self._book["accounts"][got[1]]
            if (None if lower is None else int(lower)) != d["lower"] or (None if upper is None else int(upper)) != d["upper"]:
                raise ChoboError("CB001", f"chobo: the account {got[1]}({', '.join(got[2])}) of {self._book['name']} has other bounds in the database than in the book")
            there.add(got[0])
        missing = sorted((a for a in want.values() if a[0] not in there), key=lambda a: a[0])
        if missing:
            rows = [_fill(self._book["accounts"][a[1]]["row"], {**self._base, "ID": _id_lit(a[0]), "ARGS": _str_lit(_json_strings(a[2]))}) for a in missing]
            x.plain([_fill(sql["makeAccounts"], {"ROWS": ", ".join(rows)})])
        if x.own:
            self._sure.update(want)

    def move(self, kind: str, op: str, v: Sequence[Any]) -> Result:
        """`do` and `hold`: `v` has every argument, in the order of the parameters."""
        t = self._book["transfers"][kind]
        placed = [(self._place(m["from"], v), self._place(m["to"], v)) for m in t["moves"]]
        # a move from an account to itself: refused before anything is sent, and the key is not used
        if any(f[0] == to[0] for f, to in placed):
            return _refused("same_account")
        key = [str(v[i]) for i in t["key"]]
        content = _content(t, v)
        vals = {
            **self._base,
            "CALL": _id_lit(_id("transfer", self._book["name"], self.tenant, kind, op, *key, "0")),
            "KEY": _str_lit(_json_strings(key)),
            "CONTENT": _str_lit(content),
        }
        bounded: list[int] = []
        for i, m in enumerate(t["moves"]):
            k = i + 1
            f, to = placed[i]
            vals[f"FROM{k}"], vals[f"TO{k}"] = _id_lit(f[0]), _id_lit(to[0])
            vals[f"FROMX{k}"], vals[f"TOX{k}"] = _hex(f[0]), _hex(to[0])
            vals[f"AMOUNT{k}"] = str(v[m["amount"]["param"]] if "param" in m["amount"] else m["amount"]["literal"])
            if self._book["accounts"][f[1]]["bounded"]:
                bounded.append(f[0])
            if self._book["accounts"][to[1]]["bounded"]:
                bounded.append(to[0])
        vals["BOUNDED"] = _id_list(bounded)
        locked = len(set(bounded))
        sql = t["sql"]

        def go(x: _Tx) -> Result:
            self._make_sure(x, [a for pair in placed for a in pair])
            first = [_fill(sql["key"], vals)]
            if sql["lock"] is not None:
                first.append(_fill(sql["lock"], vals))
            try:
                rows = x.run(first, begin=True)
            except Exception as e:
                if _errno(e) != _DUPLICATE:
                    raise
                # the key was used: by this call made before, or by another
                used = x.run([_fill(self._book["sql"]["readCall"], vals)])[0][0]
                x.quit()
                result, _, was, definition = used
                if result == "refused":
                    return _refused("already_refused")
                return Result("done_before") if was == content and definition == t["definition"] else _refused("key_conflict")
            if sql["lock"] is not None:
                lock_rows = rows[1]
                if len(lock_rows) != locked:
                    # an account this value made sure of is not there: its row was taken away outside
                    # the clients; make it again, and lock again
                    self._sure.difference_update(bounded)
                    self._make_sure(x, [a for pair in placed for a in pair])
                    lock_rows = x.run([_fill(sql["lock"], vals)])[0]
                    if len(lock_rows) != locked:
                        raise RuntimeError(f"chobo: {len(lock_rows)} of the {locked} accounts of {kind}.{op} with bounds are in the database")
                worst = [int(r[1]) for r in lock_rows if int(r[1]) > 0]
                if worst:
                    check = next(c for c in sql["checks"] if c["n"] == min(worst))
                    x.run([_fill(sql["refuse"], {**vals, "REASON": check["sql"]})], commit=True)
                    return _refused(check["reason"])
            writes = [_fill(sql["entries"], vals)]
            if sql["apply"] is not None:
                writes.append(_fill(sql["apply"], vals))
            x.run(writes, commit=True)
            return Result("done")

        return self._tx(go)

    def _ending(self, t: dict[str, Any], moves: list[dict[str, str]], posted: Sequence[int]) -> dict[str, str]:
        """The values of the end of a hold, from its moves as the database has them."""
        vals: dict[str, str] = {}
        bounded: list[int] = []
        for i, m in enumerate(moves):
            k = i + 1
            vals[f"FROM{k}"], vals[f"TO{k}"] = f"X'{m['from']}'", f"X'{m['to']}'"
            vals[f"HELD{k}"] = m["amount"]
            vals[f"POSTED{k}"] = str(posted[i] if i < len(posted) else 0)
            if self._book["accounts"][t["moves"][i]["from"]["kind"]]["bounded"]:
                bounded.append(int(m["from"], 16))
            if self._book["accounts"][t["moves"][i]["to"]["kind"]]["bounded"]:
                bounded.append(int(m["to"], 16))
        vals["BOUNDED"] = _id_list(bounded)
        return vals

    def end(self, kind: str, op: str, key: Sequence[str], amounts: Optional[list[Any]]) -> Result:
        """`post` and `void`: `key` in the order of the key; `amounts` in the order of the amounts,
        or None for all of it."""
        t = self._book["transfers"][kind]
        hold = {**self._base, "HOLD": _id_lit(_id("transfer", self._book["name"], self.tenant, kind, "hold", *key, "0"))}

        def go(x: _Tx) -> Result:
            found = x.run([_fill(self._book["sql"]["holdLock"], hold)], begin=True)[0]

            def answer(r: Result) -> Result:
                x.quit()
                return r

            if not found or found[0][0] == "refused":
                return answer(_refused("no_such_hold"))
            _, state, moves_json, posted_json, ended_definition, past = found[0]
            moves = json.loads(moves_json)
            holding = [int(m["amount"]) for m in moves]
            posted = [0] * len(holding)
            if op == "post":
                amount_params = [i for i, p in enumerate(t["params"]) if p["amount"]]
                posted = holding if amounts is None else [amounts[amount_params.index(m["amount"]["param"])] if "param" in m["amount"] else m["amount"]["literal"] for m in t["moves"]]
            posted_text = "[" + ",".join(f'"{p}"' for p in posted) + "]"
            if state == "posted":
                if op == "void":
                    return answer(_refused("already_posted"))
                return answer(Result("done_before") if posted_json == posted_text and ended_definition == t["definition"] else _refused("key_conflict"))
            if state == "voided":
                if op == "post":
                    return answer(_refused("already_voided"))
                return answer(Result("done_before") if ended_definition == t["definition"] else _refused("key_conflict"))
            if state == "expired" or int(past) == 1:
                return answer(_refused("expired"))
            if op == "post" and any(p > h for p, h in zip(posted, holding)):
                return answer(_refused("over_hold"))
            vals = {
                **hold,
                **self._ending(t, moves, posted),
                "CALL": _id_lit(_id("transfer", self._book["name"], self.tenant, kind, op, *key, "0")),
                "KEY": _str_lit(_json_strings(list(key))),
            }
            sql = t["sql"]
            apply, entries = (sql["postApply"], sql["postEntries"]) if op == "post" else (sql["voidApply"], sql["voidEntries"])
            x.run([*(_fill(s, vals) for s in apply), _fill(entries, vals)], commit=True)
            return Result("done")

        return self._tx(go)

    def status(self, kind: str, key: Sequence[str]) -> Optional[str]:
        hold = {**self._base, "HOLD": _id_lit(_id("transfer", self._book["name"], self.tenant, kind, "hold", *key, "0"))}
        rows = self._read(_fill(self._book["sql"]["status"], hold))
        if not rows or rows[0][0] == "refused" or rows[0][1] is None:
            return None
        result: str = rows[0][1]
        return "expired" if result == "held" and int(rows[0][2]) == 1 else result

    def balance(self, kind: str, args: Sequence[str]) -> Balance:
        id = _id("account", self._book["name"], self.tenant, kind, *args)
        rows = self._read(_fill(self._book["sql"]["balance"], {**self._base, "ID": _id_lit(id)}))
        if not rows:
            return Balance(0, 0, 0)
        b = Balance(int(rows[0][0]), int(rows[0][1]), int(rows[0][2]))
        for v in (b.posted, b.held_in, b.held_out):
            if v > _AMOUNT_LIMIT or v < -_AMOUNT_LIMIT - 1:
                raise OverflowError(f"chobo: the balance of {kind}({', '.join(args)}) is past a 64-bit integer")
        return b

    def expire(self) -> int:
        """Give back what the holds past their expiry hold, each in a transaction of its own, and add
        the entries of the accounts without bounds to their rows; answers how many holds it ended."""
        sql = self._book["sql"]
        ended = 0
        for tenant, id, kind, key_text in self._read(sql["due"]):
            t = self._book["transfers"].get(kind)
            if t is None:
                continue
            hold = {"TENANT": f"X'{tenant.lower()}'", "HOLD": f"X'{id.lower()}'"}

            def go(x: _Tx, t: dict[str, Any] = t, hold: dict[str, str] = hold, id: str = id, key_text: str = key_text) -> bool:
                found = x.run([_fill(sql["holdLock"], hold)], begin=True)[0]
                if not found or found[0][1] != "held" or int(found[0][5]) != 1:
                    x.quit()
                    return False
                moves = json.loads(found[0][2])
                vals = {**hold, **self._ending(t, moves, []), "CALL": _id_lit(_id("expire", id.lower())), "KEY": _str_lit(key_text)}
                x.run([*(_fill(s, vals) for s in t["sql"]["expireApply"]), _fill(t["sql"]["expireEntries"], vals)], commit=True)
                return True

            if self._tx(go):
                ended += 1
        for tenant, account in self._read(sql["foldList"]):
            at = {"TENANT": f"X'{tenant.lower()}'", "ID": f"X'{account.lower()}'"}

            def fold(x: _Tx, at: dict[str, str] = at) -> None:
                entries = x.run([_fill(sql["foldLock"], at), _fill(sql["foldRead"], at)], begin=True)[1]
                if not entries:
                    x.quit()
                    return
                posted = sum(int(r[2]) for r in entries)
                held_in = sum(int(r[3]) for r in entries)
                held_out = sum(int(r[4]) for r in entries)
                rows = ", ".join(f"(X'{r[0].lower()}', {int(r[1])})" for r in entries)
                x.run(
                    [
                        _fill(sql["foldApply"], {**at, "POSTED": str(posted), "HELD_IN": str(held_in), "HELD_OUT": str(held_out)}),
                        _fill(sql["foldMark"], {**at, "ROWS": rows}),
                    ],
                    commit=True,
                )

            self._tx(fold)
        return ended
