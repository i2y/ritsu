from __future__ import annotations

import hashlib
import secrets
import threading
from dataclasses import dataclass
from typing import Any, Optional, Sequence

import tigerbeetle as tb

#@@BOOK@@
# ── What follows is the same in every book chobo writes for TigerBeetle ──────
#
# A call is one chain of transfers in one request (DESIGN 4.2): TigerBeetle takes the chain
# whole or not at all. The bounds TigerBeetle cannot keep with an account's flags are kept by
# more transfers in the chain (a probe for a lower bound above 0, the room and floor accounts
# for an upper bound and a lower bound below 0), laid out as PLAN 0.3 says.


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


_ROLE_CODES = {"main": 1, "probe": 2, "probe_void": 3, "floor_out": 4, "room_back": 5, "room_in": 6, "floor_back": 7}
_OPENING = 8
# The most events one request takes on every cluster (a replica started with --development).
_REQUEST_MAX = 253
_AMOUNT_LIMIT = 2**63 - 1
_ID_LIMIT = 2**128 - 1
_S = tb.CreateTransferStatus


def _id(*parts: str) -> int:
    """An ID (DESIGN 4.3): the first 16 bytes of a SHA-256 over the parts, each with its UTF-8 length in front."""
    h = hashlib.sha256()
    for p in ("chobo/1",) + parts:
        b = p.encode("utf-8")
        h.update(len(b).to_bytes(4, "big"))
        h.update(b)
    v = int.from_bytes(h.digest()[:16], "big")
    return 1 if v == 0 else _ID_LIMIT - 1 if v == _ID_LIMIT else v


def _hex(v: int) -> str:
    return format(v, "032x")


def _random_id() -> int:
    """An ID no one else has: for the transfers that read a hold and are never kept."""
    v = secrets.randbits(128)
    return 1 if v in (0, _ID_LIMIT) else v


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
    """What a do or hold is called with, as the IDs hash it: the arguments in the order of the parameters."""
    return "{" + ",".join(_json_string(p["name"]) + ":" + (str(v[i]) if p["amount"] else _json_string(v[i])) for i, p in enumerate(t["params"])) + "}"


def _account(id: int, ledger: int, code: int, flagged: bool, user_data_128: int, user_data_32: int) -> tb.Account:
    return tb.Account(
        id=id,
        user_data_128=user_data_128,
        user_data_32=user_data_32,
        ledger=ledger,
        code=code,
        flags=tb.AccountFlags.DEBITS_MUST_NOT_EXCEED_CREDITS if flagged else tb.AccountFlags.NONE,
    )


def _link(chain: list[Any]) -> None:
    """Every transfer of the chain but the last is linked to the next."""
    for x in chain[:-1]:
        x[0].flags |= tb.TransferFlags.LINKED


class _TigerBeetle:
    def __init__(self, book: dict[str, Any], client: Any, tenant: str) -> None:
        self._book = book
        self._client = client
        self.tenant = _str("the tenant", tenant)
        # the accounts and openings this value has made sure of: TigerBeetle never removes them
        self._made: set[Any] = set()
        self._lock = threading.Lock()

    def _place(self, r: dict[str, Any], v: Sequence[Any]) -> tuple[Any, ...]:
        args = [str(v[a["param"]]) if "param" in a else a["literal"] for a in r["args"]]
        return (_id("account", self._book["name"], self.tenant, r["kind"], *args), r["kind"], self._book["accounts"][r["kind"]])

    def _kind_id(self, kind: str) -> int:
        return _id("kind", self._book["name"], self.tenant, kind)

    def _transfer_id(self, kind: str, op: str, key: Sequence[str], position: int) -> int:
        return _id("transfer", self._book["name"], self.tenant, kind, op, *key, str(position))

    def move(self, kind: str, op: str, v: Sequence[Any]) -> Result:
        """do and hold: `v` has every argument, in the order of the parameters."""
        book, t = self._book, self._book["transfers"][kind]
        placed = [(self._place(m["from"], v), self._place(m["to"], v)) for m in t["moves"]]
        # a move from an account to itself: refused before anything is sent, and the key is not used
        if any(f[0] == to[0] for f, to in placed):
            return _refused("same_account")
        key = [v[i] for i in t["key"]]
        content = _id("content", book["name"], self.tenant, kind, op, t["definition"], _content(t, v))
        accounts: list[Any] = []
        openings: list[Any] = []
        chain: list[Any] = []

        def add(a: tb.Account) -> None:
            if not any(x.id == a.id for x in accounts):
                accounts.append(a)

        for i, m in enumerate(t["moves"]):
            (fid, fkind, fdef), (tid, tkind, tdef) = placed[i]
            unit = fdef["unit"]
            ledger = book["units"][unit]["ledger"]
            sink = _id("sink", book["name"], self.tenant, unit)
            add(_account(fid, ledger, fdef["code"], fdef["flagged"], self._kind_id(fkind), 0))
            add(_account(tid, ledger, tdef["code"], tdef["flagged"], self._kind_id(tkind), 0))
            if len(m["roles"]) > 1:
                add(_account(sink, ledger, book["units"][unit]["sink_code"], False, 0, 3))
            # the floor and room accounts the move's bounds need, each with the transfer that opens it
            for role in m["roles"]:
                if role == "floor_out":
                    what, (oid, okind, odef), opening = "floor", placed[i][0], -fdef["lower"]["value"]
                elif role == "room_back":
                    what, (oid, okind, odef), opening = "room", placed[i][0], fdef["upper"]["value"]
                elif role == "room_in":
                    what, (oid, okind, odef), opening = "room", placed[i][1], tdef["upper"]["value"]
                elif role == "floor_back":
                    what, (oid, okind, odef), opening = "floor", placed[i][1], -tdef["lower"]["value"]
                else:
                    continue
                xid = _id(what, _hex(oid))
                if any(x.id == xid for x in accounts):
                    continue
                add(_account(xid, ledger, odef["code"], True, self._kind_id(okind), 1 if what == "floor" else 2))
                openings.append(tb.Transfer(id=_id("opening", _hex(xid)), debit_account_id=sink, credit_account_id=xid, amount=opening, user_data_32=_OPENING, ledger=ledger, code=odef["code"]))
            amount = v[m["amount"]["param"]] if "param" in m["amount"] else m["amount"]["literal"]
            flags = tb.TransferFlags.PENDING if op == "hold" else tb.TransferFlags.NONE
            timeout = t["timeout"] if op == "hold" else 0
            for role in m["roles"]:
                x = tb.Transfer(id=self._transfer_id(kind, op, key, len(chain)), user_data_128=content, user_data_32=_ROLE_CODES[role])
                if role == "main":
                    x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout = fid, tid, amount, flags, timeout
                elif role == "probe":
                    x.debit_account_id, x.credit_account_id, x.amount, x.flags = fid, sink, fdef["lower"]["value"], tb.TransferFlags.PENDING
                elif role == "probe_void":
                    x.pending_id, x.flags = chain[-1][0].id, tb.TransferFlags.VOID_PENDING_TRANSFER
                elif role == "floor_out":
                    x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout = _id("floor", _hex(fid)), sink, amount, flags, timeout
                elif role == "room_back":
                    x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout = sink, _id("room", _hex(fid)), amount, flags, timeout
                elif role == "room_in":
                    x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout = _id("room", _hex(tid)), sink, amount, flags, timeout
                elif role == "floor_back":
                    x.debit_account_id, x.credit_account_id, x.amount, x.flags, x.timeout = sink, _id("floor", _hex(tid)), amount, flags, timeout
                if role != "probe_void":
                    x.ledger, x.code = ledger, t["code"]
                chain.append((x, role, i))
        _link(chain)
        self._ensure(accounts, openings)
        results = self._client.create_transfers([x[0] for x in chain])
        return self._read(kind, op, chain, results)

    def _ensure(self, accounts: list[Any], openings: list[Any]) -> None:
        """Make sure the accounts and the openings are there before a chain needs them: TigerBeetle
        refuses a transfer to an account it does not have, and never takes its ID again."""
        with self._lock:
            new_accounts = [a for a in accounts if a.id not in self._made]
        for i in range(0, len(new_accounts), _REQUEST_MAX):
            batch = new_accounts[i : i + _REQUEST_MAX]
            for a, r in zip(batch, self._client.create_accounts(batch)):
                if r.status not in (tb.CreateAccountStatus.CREATED, tb.CreateAccountStatus.EXISTS):
                    raise RuntimeError(f"chobo: TigerBeetle has the account {_hex(a.id)} made another way ({r.status.name.lower()}): the book's accounts have changed since it was made")
            with self._lock:
                self._made.update(a.id for a in batch)
        with self._lock:
            new_openings = [x for x in openings if x.id not in self._made]
        for i in range(0, len(new_openings), _REQUEST_MAX):
            batch = new_openings[i : i + _REQUEST_MAX]
            for x, r in zip(batch, self._client.create_transfers(batch)):
                if r.status == _S.EXISTS_WITH_DIFFERENT_AMOUNT:
                    raise RuntimeError(f"chobo: the account {_hex(x.credit_account_id)} was opened with another bound: the book's bounds have changed since it was made")
                if r.status not in (_S.CREATED, _S.EXISTS):
                    raise RuntimeError(f"chobo: TigerBeetle answered {r.status.name.lower()} for the opening {_hex(x.id)}")
            with self._lock:
                self._made.update(x.id for x in batch)

    def _read(self, kind: str, op: str, chain: list[Any], results: list[Any]) -> Result:
        """What a chain's results answer: the first that is neither created nor linked_event_failed decides (DESIGN 4.2)."""
        t = self._book["transfers"][kind]
        for i, r in enumerate(results):
            s = r.status
            if s in (_S.CREATED, _S.LINKED_EVENT_FAILED):
                continue
            if s == _S.EXISTS:
                # the first transfer there already: the same call, made before; a later one: a chain of another shape
                return Result("done_before") if i == 0 else _refused("key_conflict")
            if s == _S.ID_ALREADY_FAILED:
                return _refused("already_refused")
            if s == _S.EXCEEDS_CREDITS:
                m = t["moves"][chain[i][2]]
                if chain[i][1] == "room_in":
                    bound = self._book["accounts"][m["to"]["kind"]]["upper"]
                else:
                    bound = self._book["accounts"][m["from"]["kind"]]["lower"]
                if bound is not None:
                    return _refused(bound["reason"])
            elif s == _S.ACCOUNTS_MUST_BE_DIFFERENT:
                return _refused("same_account")
            elif s == _S.PENDING_TRANSFER_ALREADY_POSTED:
                return _refused("already_posted")
            elif s == _S.PENDING_TRANSFER_ALREADY_VOIDED:
                return _refused("already_voided")
            elif s == _S.PENDING_TRANSFER_EXPIRED:
                return _refused("expired")
            elif s == _S.EXCEEDS_PENDING_TRANSFER_AMOUNT:
                return _refused("over_hold")
            elif s.name.startswith("EXISTS_WITH_DIFFERENT_"):
                return _refused("key_conflict")
            raise RuntimeError(f"chobo: TigerBeetle answered {s.name.lower()} for transfer {i} ({chain[i][1]}) of {kind}.{op}")
        return Result("done")

    def _held(self, kind: str, key: Sequence[str]) -> list[Any]:
        """The transfers of the chain that held for `key`: their roles, moves and IDs."""
        out: list[Any] = []
        for i, m in enumerate(self._book["transfers"][kind]["moves"]):
            for role in m["roles"]:
                out.append((role, i, self._transfer_id(kind, "hold", key, len(out))))
        return out

    def end(self, kind: str, op: str, key: Sequence[str], amounts: Optional[list[Any]]) -> Result:
        """post and void: `key` in the order of the key; `amounts` in the order of the amounts, or None for all of it."""
        book, t = self._book, self._book["transfers"][kind]
        held = self._held(kind, key)
        mains = [h for h in held if h[0] == "main"]
        # read the hold first: a post or void of a hold TigerBeetle does not have would use up its ID for good
        found = {x.id: x.amount for x in self._client.lookup_transfers([h[2] for h in mains])}
        if not found:
            return _refused("no_such_hold")
        if len(found) != len(mains):
            raise RuntimeError(f"chobo: TigerBeetle has only part of the hold {kind}({', '.join(key)})")
        holding = [found[h[2]] for h in mains]
        posted: list[Any] = []
        if op == "post":
            amount_params = [i for i, p in enumerate(t["params"]) if p["amount"]]
            if amounts is None:
                posted = holding
            else:
                posted = [amounts[amount_params.index(m["amount"]["param"])] if "param" in m["amount"] else m["amount"]["literal"] for m in t["moves"]]
            # TigerBeetle checks the amount before it checks whether the hold has ended: ask where the hold is instead
            if any(p > h for p, h in zip(posted, holding)):
                state = self._state(mains[0][2])
                answers = {"held": _refused("over_hold"), "posted": _refused("key_conflict"), "voided": _refused("already_voided"), "expired": _refused("expired")}
                return answers.get(state, _refused("no_such_hold")) if state is not None else _refused("no_such_hold")
        content = _id("content", book["name"], self.tenant, kind, op, t["definition"], "[" + ",".join(str(p) for p in posted) + "]" if op == "post" else "null")
        chain: list[Any] = []
        for role, move, pid in held:
            if role in ("probe", "probe_void"):
                continue
            amount = 0 if op == "void" else tb.AMOUNT_MAX if amounts is None else posted[move]
            flags = tb.TransferFlags.POST_PENDING_TRANSFER if op == "post" else tb.TransferFlags.VOID_PENDING_TRANSFER
            chain.append((tb.Transfer(id=self._transfer_id(kind, op, key, len(chain)), pending_id=pid, amount=amount, user_data_128=content, user_data_32=_ROLE_CODES[role], flags=flags), role, move))
        _link(chain)
        results = self._client.create_transfers([x[0] for x in chain])
        return self._read(kind, op, chain, results)

    def _state(self, id: int) -> Optional[str]:
        """Where the hold of the pending transfer `id` is, as TigerBeetle would answer a void of it
        now: a void and a transfer that cannot go through, linked, so that nothing is kept."""
        r = self._client.create_transfers(
            [
                tb.Transfer(id=_random_id(), pending_id=id, flags=tb.TransferFlags.LINKED | tb.TransferFlags.VOID_PENDING_TRANSFER),
                tb.Transfer(id=_random_id(), debit_account_id=1, credit_account_id=1, ledger=1, code=1),
            ]
        )
        s = r[0].status
        if s == _S.LINKED_EVENT_FAILED and r[1].status == _S.ACCOUNTS_MUST_BE_DIFFERENT:
            return "held"
        if s == _S.PENDING_TRANSFER_ALREADY_POSTED:
            return "posted"
        if s == _S.PENDING_TRANSFER_ALREADY_VOIDED:
            return "voided"
        if s == _S.PENDING_TRANSFER_EXPIRED:
            return "expired"
        if s == _S.PENDING_TRANSFER_NOT_FOUND:
            return None
        raise RuntimeError(f"chobo: TigerBeetle answered {s.name.lower()}, {r[1].status.name.lower()} when asked where a hold is")

    def status(self, kind: str, key: Sequence[str]) -> Optional[str]:
        return self._state(self._held(kind, key)[0][2])

    def balance(self, kind: str, args: Sequence[str]) -> Balance:
        found = self._client.lookup_accounts([_id("account", self._book["name"], self.tenant, kind, *args)])
        if not found:
            return Balance(0, 0, 0)
        a = found[0]
        b = Balance(a.credits_posted - a.debits_posted, a.credits_pending, a.debits_pending)
        if any(x > _AMOUNT_LIMIT or x < -_AMOUNT_LIMIT - 1 for x in (b.posted, b.held_in, b.held_out)):
            raise OverflowError(f"chobo: the balance of {kind}({', '.join(args)}) is past a 64-bit integer")
        return b
