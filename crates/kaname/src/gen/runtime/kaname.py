# @HEAD@
"""What every module of kaname in this package shares (kaname's DESIGN 10.1): exact fractions, the
arithmetic held to int64, rulec's five ways of rounding, dates, how a value is written in a message,
and the error the modules raise. It imports the standard library alone."""

from __future__ import annotations

import datetime
from dataclasses import dataclass
from fractions import Fraction
from typing import Generic, Literal, NoReturn, TypeVar

# The language the messages are written in: the `--lang` of `ritsu gen`.
LANG = "@LANG@"

# Why a function of kaname stops: the argument a host gave is outside what the function takes
# (`input`), or a place the checks could not show fails when the code runs (the kinds of the
# effects, kaname's DESIGN 4.2).
Kind = Literal["input", "rule", "date", "range", "overflow", "zero"]

# A value that is not a whole number of its unit: an exact fraction.
Q = Fraction


class KanameError(Exception):
    """What a function of kaname raises: the kind, the module and the line of the `.kn`, and why."""

    def __init__(self, kind: Kind, module: str, line: int, message: str) -> None:
        self.kind: Kind = kind
        self.module = module
        self.line = line
        self.message = _spaced(message)
        super().__init__(self.message)


def fail(kind: Kind, module: str, line: int, message: str) -> NoReturn:
    """Raise the error of `kind` at `line` of `module`."""
    raise KanameError(kind, module, line, message)


def _spaced(s: str) -> str:
    """A message as kaname writes it: an English sentence begins with a capital; Japanese has a space
    between an ASCII word and the Japanese beside it, outside backquotes."""
    if LANG != "ja":
        return s[0].upper() + s[1:] if s and "a" <= s[0] <= "z" else s
    out = []
    code = False
    for i, c in enumerate(s):
        if c == "`":
            code = not code
        if i > 0 and not code:
            p = s[i - 1]
            if (_latin(p) and _japanese(c)) or (_japanese(p) and _latin(c)):
                out.append(" ")
        out.append(c)
    return "".join(out)


def _latin(c: str) -> bool:
    return ("a" <= c <= "z") or ("A" <= c <= "Z")


def _japanese(c: str) -> bool:
    u = ord(c)
    return 0x3040 <= u <= 0x30FF or 0x3400 <= u <= 0x4DBF or 0x4E00 <= u <= 0x9FFF or u == 0x3005


# ── Fractions ───────────────────────────────────────────────────────────────

_MAX = (1 << 63) - 1
_MIN = -(1 << 63)


def whole(a: Fraction) -> int:
    """A fraction whose type says it is a whole number of its unit, as that number."""
    if a.denominator != 1:
        raise ValueError(f"kaname: {a} is no whole number")
    return a.numerator


def steps(a: Fraction, step: Fraction) -> int:
    """A rate, as the whole number of its step it travels as."""
    return whole(a / step)


def rate(k: int, step: Fraction) -> Fraction:
    """A rate that travels as `k` steps of `step`."""
    return k * step


# ── The rule of int64 (kaname's DESIGN 5.6) ───────────────────────────────


def held_int(x: int, module: str, line: int, message: str) -> int:
    """A whole number held to int64: the error `overflow` when it is outside."""
    if x < _MIN or x > _MAX:
        fail("overflow", module, line, message)
    return x


def held(x: Fraction, module: str, line: int, message: str) -> Fraction:
    """A fraction held to int64: its numerator in int64, and its denominator from 1 to 2^63 - 1."""
    if x.numerator < _MIN or x.numerator > _MAX or x.denominator > _MAX:
        fail("overflow", module, line, message)
    return x


def over(a: Fraction, b: Fraction, module: str, line: int, message: str) -> Fraction:
    """`a / b`, with the error `zero` when `b` is 0."""
    if b == 0:
        fail("zero", module, line, message)
    return a / b


# ── Rounding (kaname's DESIGN 3.3) ────────────────────────────────────────

Way = Literal["up", "down", "half_up", "half_down", "half_even"]


def round_to(x: Fraction, way: Way, g: Fraction) -> Fraction:
    """`x` rounded `way` to a whole number of the step `g` (above 0): the steps are |n·gd| over d·gn,
    the quotient taken or one past it as `way` says, the sign put back. Python's `//` (toward −∞)
    decides no fraction's way."""
    top = x.numerator * g.denominator
    y = x.denominator * g.numerator
    a = -top if top < 0 else top
    k, r = a // y, a % y
    past = False
    if r != 0:
        if way == "up":
            past = True
        elif way == "half_up":
            past = 2 * r >= y
        elif way == "half_down":
            past = 2 * r > y
        elif way == "half_even":
            past = 2 * r > y or (2 * r == y and k % 2 == 1)
    if past:
        k += 1
    if top < 0:
        k = -k
    return k * g


# Where a value that leaves int64 or a divisor of 0 is said: the module, the line, the message.
At = tuple[str, int, str]


def allocate(t: Fraction, c: Fraction, s: Fraction, overflow: At | None, zero: At | None) -> Fraction:
    """`allocate(t, c, s)`: `t × c / s` rounded toward 0 to a whole unit (rulec's §15.102), each value
    on the way held to int64 where `overflow` says, and the error `zero` where `zero` says."""
    p = t * c
    if overflow is not None:
        p = held(p, *overflow)
    if zero is not None and s == 0:
        fail("zero", *zero)
    part = p / s
    if overflow is not None:
        part = held(part, *overflow)
    r = round_to(part, "down", Fraction(1))
    if overflow is not None:
        r = held(r, *overflow)
    return r


def mod(x: Fraction, n: int) -> int:
    """`mod(x, n)`: the whole part of `x` (toward −∞) modulo the positive whole `n`, from 0 to n − 1."""
    return (x.numerator // x.denominator) % n


def isqrt(x: Fraction) -> int | None:
    """`isqrt(x)`: the largest whole number whose square is not above `x`; None below 0."""
    if x < 0:
        return None
    v = x.numerator // x.denominator
    lo, hi = 0, min(v, 1 << 63)
    while lo < hi:
        mid = lo + (hi - lo + 1) // 2
        if mid * mid <= v:
            lo = mid
        else:
            hi = mid - 1
    return lo


def root(x: Fraction) -> int:
    """`isqrt(x)` where the analysis shows `x` is 0 or more."""
    v = isqrt(x)
    if v is None:
        raise ValueError(f"kaname: isqrt of {x}")
    return v


# ── Dates (kaname's DESIGN 3.4) ───────────────────────────────────────────

_EPOCH = datetime.date(1970, 1, 1).toordinal()


def day_of(d: datetime.date) -> int:
    """The day number of a date (days since 1970-01-01), as rulec takes it."""
    return d.toordinal() - _EPOCH


def date_of(z: int) -> datetime.date:
    """The date of a day number."""
    return datetime.date.fromordinal(z + _EPOCH)


def between(b: datetime.date, a: datetime.date) -> int:
    """`b - a`: the days from the date `a` to the date `b`."""
    return (b - a).days


# ── Values as a message writes them ───────────────────────────────────────


def rat_text(x: Fraction) -> str:
    """A fraction written out: a whole number as it is, a decimal where the denominator is made of 2s
    and 5s, and `n/d` otherwise."""
    n, d = x.numerator, x.denominator
    if d == 1:
        return str(n)
    sign = "-" if n < 0 else ""
    num = -n if n < 0 else n
    rest, twos, fives = d, 0, 0
    while rest % 2 == 0:
        rest //= 2
        twos += 1
    while rest % 5 == 0:
        rest //= 5
        fives += 1
    places = max(twos, fives)
    scale = 10**places
    # as ritsu writes it: a decimal past 128 bits is written as the fraction
    if rest == 1 and places <= 38 and num * scale < 1 << 128:
        v = num * scale // d
        return f"{sign}{v // scale}.{str(v % scale).zfill(places)}"
    return f"{sign}{num}/{d}"


def num_text(x: Fraction, unit: str) -> str:
    """A number as a literal of its unit writes it: `1150GBPc`, `0.5%`, `30`."""
    if unit == "%":
        return rat_text(x * 100) + "%"
    return rat_text(x) + unit


def date_text(d: datetime.date) -> str:
    """A date as kaname writes it, `YYYY-MM-DD`."""
    return f"{d.year:04d}-{d.month:02d}-{d.day:02d}"


def itoa(n: int) -> str:
    """A whole number in decimal: what a message says a count is, where a parameter of the function
    may have the name of Python's `str`."""
    return str(n)


@dataclass(frozen=True)
class Outside:
    """Where a value a function is given or answers is outside what its place declares: the place
    (a parameter, and the field or the element of it), the value as a message writes it, and the
    range; or, for a list, its count."""

    path: str
    shown: str
    range: str
    count: int | None


T = TypeVar("T")
W = TypeVar("W")


@dataclass(frozen=True)
class Running(Generic[T, W]):
    """An element of what `running_totals` answers (kaname's DESIGN 3.6)."""

    item: T
    before: W
    upto: W
    total: W
