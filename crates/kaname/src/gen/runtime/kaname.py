# @HEAD@
"""What every module of kaname in this package shares (kaname's DESIGN 10.1): exact fractions, the
arithmetic held to int64, rulec's five ways of rounding, dates, how a value is written in a message,
and the error the modules raise. It imports the standard library alone."""

from __future__ import annotations

import datetime
import re
from dataclasses import dataclass
from fractions import Fraction
from typing import AbstractSet, Any, Callable, Generic, Iterable, Literal, Mapping, NoReturn, Sequence, TypeVar, Union

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


def eq(a: object, b: object) -> bool:
    """Whether two values of an enum that has fields are the same, compared whole (what `==` of their
    data classes does): an `isinstance` that splits one of them can leave the two of types that do not
    meet, which `mypy --strict` does not let `==` compare."""
    return a == b


# ── Orders, dicts, sets and lists (kaname's DESIGN 24) ───────────────────

K = TypeVar("K")
V = TypeVar("V")
E = TypeVar("E")
T = TypeVar("T")
W = TypeVar("W")


def natural(x: Any) -> Any:
    """The key of a value in its own order: a number, a date, a string by its code points, `False`
    before `True`."""
    return x


def dict_of(pairs: Iterable[tuple[K, V]], key: Callable[[K], Any]) -> dict[K, V]:
    """A dict of these pairs, the later of two of one key kept, in the order of its keys: a `dict`
    goes round in the order its keys are put in."""
    d = dict(pairs)
    return {k: d[k] for k in sorted(d, key=key)}


def keys_of(d: Mapping[K, Any] | AbstractSet[K], key: Callable[[K], Any]) -> list[K]:
    """The keys of a dict or the members of a set, in their order."""
    return sorted(d, key=key)


def values_of(d: Mapping[K, V], key: Callable[[K], Any]) -> list[V]:
    """The values of a dict, in the order of their keys."""
    return [d[k] for k in sorted(d, key=key)]


def entries_of(d: Mapping[K, V], key: Callable[[K], Any]) -> "list[Entry[K, V]]":
    """The pairs of a dict as entries, in the order of their keys."""
    return [Entry(k, d[k]) for k in sorted(d, key=key)]


def put(d: Mapping[K, V], k: K, v: V, key: Callable[[K], Any]) -> dict[K, V]:
    return dict_of([*d.items(), (k, v)], key)


def remove(d: Mapping[K, V], k: K) -> dict[K, V]:
    return {x: y for x, y in d.items() if x != k}


def merge(a: Mapping[K, V], b: Mapping[K, V], key: Callable[[K], Any]) -> dict[K, V]:
    """The pairs of both, `b`'s value for a key of both."""
    return dict_of([*a.items(), *b.items()], key)


def remap(d: Mapping[K, V], fk: Callable[[K], E], fv: Callable[[V], T]) -> dict[E, T]:
    """A dict with each key and each value read anew."""
    return {fk(k): fv(v) for k, v in d.items()}


def _cut(i: int, n: int) -> int:
    """A whole number cut to 0 and to `n`: where a list is cut (kaname's DESIGN 24.3)."""
    return 0 if i < 0 else n if i > n else i


def at(xs: Sequence[T], i: int) -> T | None:
    """The element at `i` (from 0); None past either end."""
    return xs[i] if 0 <= i < len(xs) else None


def first(xs: Sequence[T]) -> T | None:
    return xs[0] if xs else None


def last(xs: Sequence[T]) -> T | None:
    return xs[-1] if xs else None


def slice_of(xs: Sequence[T], a: int, b: int) -> list[T]:
    """The elements from `a` up to before `b`, both cut to the list."""
    i, j = _cut(a, len(xs)), _cut(b, len(xs))
    return list(xs[i:j]) if i < j else []


def take(xs: Sequence[T], n: int) -> list[T]:
    return list(xs[: _cut(n, len(xs))])


def drop(xs: Sequence[T], n: int) -> list[T]:
    return list(xs[_cut(n, len(xs)) :])


def flatten(xss: Sequence[Sequence[T]]) -> list[T]:
    return [x for xs in xss for x in xs]


def by_key(ps: Sequence[tuple[K, T]], key: Callable[[K], Any], desc: bool) -> list[T]:
    """The elements in the order of their keys, from the greatest where `desc`, two of one key in the
    order of the list either way (`sorted` keeps them so, `reverse` too)."""
    return [p[1] for p in sorted(ps, key=lambda p: key(p[0]), reverse=desc)]


def distinct(xs: Sequence[T]) -> list[T]:
    """The first of each key, in the order of the list."""
    return list(dict.fromkeys(xs))


def numbers(a: int, b: int, over: At | None) -> list[int]:
    """`numbers(a, b)`: `a` to `b`, both in; none where `b` is before `a`; the count held to int64
    where `over` says."""
    if b < a:
        return []
    if over is not None and b - a + 1 > _MAX:
        fail("overflow", *over)
    return list(range(a, b + 1))


def mod_of(x: Fraction, n: int, zero: At | None) -> int:
    """`mod(x, n)` of a divisor that is a value: the whole part of `x` (toward −∞) modulo `n`, from 0
    to |n| − 1 (Euclid's, kaname's DESIGN 24.3); the error `zero` where `zero` says and `n` is 0."""
    if zero is not None and n == 0:
        fail("zero", *zero)
    return (x.numerator // x.denominator) % abs(n)


def itoa(n: int | None) -> str:
    """A whole number in decimal: what a message says a count is, where a parameter of the function
    may have the name of Python's `str`. The check that finds a count outside sets it."""
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
    # Whether the count is of a string's code points (kaname's DESIGN 25.1).
    chars: bool = False
    # Whether the value is a string that is not well formed: a lone surrogate.
    bad: bool = False


@dataclass(frozen=True)
class Entry(Generic[K, V]):
    """An entry of a dict: an element of what `entries` answers (kaname's DESIGN 24.3)."""

    key: K
    value: V


@dataclass(frozen=True)
class Indexed(Generic[T]):
    """An element of what `indexed` answers: the index from 0, and the element."""

    index: int
    item: T


@dataclass(frozen=True)
class Running(Generic[T, W]):
    """An element of what `running_totals` answers (kaname's DESIGN 3.6)."""

    item: T
    before: W
    upto: W
    total: W


@dataclass(frozen=True)
class Ok(Generic[T]):
    """The value `ok` of a result (kaname's DESIGN 26.3): what it holds."""

    value: T


@dataclass(frozen=True)
class Err(Generic[E]):
    """The value `err` of a result: why it is no `ok`."""

    error: E


# A value of `result[T, E]` (kaname's DESIGN 26.3): `Ok` with its value, or `Err` with its error.
Result = Union[Ok[T], Err[E]]


# ── Strings (kaname's DESIGN 25) ──────────────────────────────────────────

# A `str` is a sequence of code points already: its length, cuts, places and order are kaname's. The
# white space, the cases and what is read as a number or a date are kaname's own, here: `str.strip`,
# `str.lower` and `int` take more than they do.

# The code points of Unicode's White_Space, which `trim` and `words` take for white space.
_SPACES = "".join(
    chr(c)
    for c in (0x9, 0xA, 0xB, 0xC, 0xD, 0x20, 0x85, 0xA0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000)
)
_LOWER = {c: c + 32 for c in range(ord("A"), ord("Z") + 1)}
_UPPER = {c: c - 32 for c in range(ord("a"), ord("z") + 1)}
_NUMBER = re.compile(r"[+-]?[0-9]+")
_DATE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")


def is_text(s: str) -> bool:
    """Whether a string is well formed: no surrogate, which a host may give."""
    try:
        s.encode("utf-8")
    except UnicodeEncodeError:
        return False
    return True


def text_slice(s: str, a: int, b: int) -> str:
    """The code points from `a` up to before `b`, both cut to the string."""
    i, j = _cut(a, len(s)), _cut(b, len(s))
    return s[i:j] if i < j else ""


def text_take(s: str, n: int) -> str:
    return s[: _cut(n, len(s))]


def text_drop(s: str, n: int) -> str:
    return s[_cut(n, len(s)) :]


def index_of(s: str, t: str) -> int | None:
    """Where `t` first stands in `s`; 0 for an empty `t`, None where it is not there."""
    i = s.find(t)
    return None if i < 0 else i


def to_text(ns: Sequence[int]) -> str | None:
    """The string of these code points; None where one is no Unicode scalar value."""
    if any(n < 0 or n > 0x10FFFF or 0xD800 <= n <= 0xDFFF for n in ns):
        return None
    return "".join(chr(n) for n in ns)


def split(s: str, sep: str) -> list[str]:
    """`s` cut at each `sep`, the empty strings at the ends kept; each code point where `sep` is empty."""
    return list(s) if sep == "" else s.split(sep)


def words(s: str) -> list[str]:
    """The runs of what is not white space."""
    out: list[str] = []
    w: list[str] = []
    for c in s:
        if c in _SPACES:
            if w:
                out.append("".join(w))
            w = []
        else:
            w.append(c)
    if w:
        out.append("".join(w))
    return out


def replace(s: str, old: str, new: str) -> str:
    """Every `old` from the left made `new`; `s` as it is where `old` is empty."""
    return s if old == "" else s.replace(old, new)


def trim(s: str) -> str:
    return s.strip(_SPACES)


def trim_start(s: str) -> str:
    return s.lstrip(_SPACES)


def trim_end(s: str) -> str:
    return s.rstrip(_SPACES)


def lower_ascii(s: str) -> str:
    """A to Z made a to z, nothing else changed."""
    return s.translate(_LOWER)


def upper_ascii(s: str) -> str:
    """a to z made A to Z, nothing else changed."""
    return s.translate(_UPPER)


def pad(s: str, width: int, fill: str, at_end: bool) -> str:
    """`fill` repeated before `s` (after it, with `at_end`) to make it `width` code points long, the
    last repetition cut; `s` as it is where `fill` is empty or `s` is that long already."""
    if fill == "" or len(s) >= width:
        return s
    k = width - len(s)
    added = (fill * (k // len(fill) + 1))[:k]
    return s + added if at_end else added + s


def parse_number(s: str) -> int | None:
    """The number `s` writes: one `+` or `-` at most, then ASCII digits, nothing else, within int64."""
    if _NUMBER.fullmatch(s) is None:
        return None
    v = int(s)
    return v if _MIN <= v <= _MAX else None


def parse_date(s: str) -> datetime.date | None:
    """The date `s` writes: `YYYY-MM-DD` alone, a day of 0001-01-01 to 9999-12-31."""
    if _DATE.fullmatch(s) is None:
        return None
    try:
        return datetime.date(int(s[0:4]), int(s[5:7]), int(s[8:10]))
    except ValueError:
        return None
