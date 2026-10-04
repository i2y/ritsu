from __future__ import annotations

import random
import time
from dataclasses import dataclass
from typing import Any, Optional, Sequence

#@@BOOK@@
# ── What follows is the same in every book chobo writes for PostgreSQL ───────
#
# A call is one SQL function, which does it in one transaction. The function
# answers a refusal as a row; an error is a mistake in the call, or the database's own.


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
# How many times a call is made when PostgreSQL answers a serialization failure or a deadlock.
_ATTEMPTS = 10


def _backoff(attempt: int) -> None:
    """A wait before the next try: up to 1, 2, 4 … 128 ms, at random, so that the calls that
    failed together do not meet again."""
    time.sleep(random.random() * 2 ** min(attempt - 1, 7) / 1000)


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


def _amounts(names: Sequence[str], values: Sequence[Any]) -> list[Any]:
    """The amounts of a post: every one, or none (all of it)."""
    if all(v is None for v in values):
        return [None] * len(values)
    if any(v is None for v in values):
        raise ValueError(f"chobo: post takes every amount ({', '.join(names)}) or none")
    return [_amt(n, v) for n, v in zip(names, values)]


def _sqlstate(e: BaseException) -> Optional[str]:
    # psycopg 3 names it sqlstate, psycopg2 pgcode
    return getattr(e, "sqlstate", None) or getattr(e, "pgcode", None)


class _Postgres:
    def __init__(self, conn: Any, tenant: str) -> None:
        self._conn = conn
        self.tenant = _str("the tenant", tenant)

    def _row(self, sql: str, params: Sequence[Any]) -> tuple[Any, ...]:
        """One row of `sql`. A serialization failure (40001) or a deadlock (40P01) is tried again with
        the same arguments: the key keeps the call from moving twice. Inside a transaction of the
        caller's, the second try finds the transaction aborted (25P02), and the first error is raised."""
        first: Optional[BaseException] = None
        attempt = 0
        while True:
            attempt += 1
            try:
                with self._conn.cursor() as cur:
                    cur.execute(sql, list(params))
                    row: tuple[Any, ...] = cur.fetchone()
                    return row
            except Exception as e:
                code = _sqlstate(e)
                if first is not None and code == "25P02":
                    raise first
                if code in ("40001", "40P01") and attempt < _ATTEMPTS:
                    if first is None:
                        first = e
                    _backoff(attempt)
                    continue
                raise

    def call(self, sql: str, params: Sequence[Any]) -> Result:
        result, reason = self._row(sql, params)
        if result in ("done", "done_before"):
            return Result(result)
        if result == "refused":
            return Result("refused", reason)
        raise RuntimeError(f"chobo: the function answered {result!r}, {reason!r}")

    def status(self, sql: str, params: Sequence[Any]) -> Optional[str]:
        state: Optional[str] = self._row(sql, params)[0]
        return state

    def balance(self, sql: str, params: Sequence[Any]) -> Balance:
        posted, held_in, held_out = self._row(sql, params)
        return Balance(int(posted), int(held_in), int(held_out))

    def expire(self, sql: str) -> int:
        """Give back what the holds past their expiry hold; answers how many holds it ended."""
        return int(self._row(sql, [])[0])
