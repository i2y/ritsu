// @HEAD@
// What every module of kaname in this package shares (kaname's DESIGN 10.1): exact fractions, the
// arithmetic held to int64, rulec's five ways of rounding, dates, how a value is written in a
// message, and the error the modules throw. It imports nothing.

/** The language the messages are written in: the `--lang` of `ritsu gen`. */
const LANG: string = "@LANG@";

/**
 * Why a function of kaname stops: the argument a host gave is outside what the function takes
 * (`input`), or a place the checks could not show fails when the code runs (the kinds of the
 * effects, kaname's DESIGN 4.2).
 */
export type Kind = "input" | "rule" | "date" | "range" | "overflow" | "zero";

/** What a function of kaname throws: the kind, the module and the line of the `.kn`, and why. */
export class KanameError extends Error {
  readonly kind: Kind;
  readonly module: string;
  readonly line: number;
  constructor(kind: Kind, module: string, line: number, message: string, cause?: unknown) {
    super(spaced(message), cause === undefined ? undefined : { cause });
    this.name = "KanameError";
    this.kind = kind;
    this.module = module;
    this.line = line;
  }
}

/** Throw the error of `kind` at `line` of `module`; `cause` is what another language threw. */
export function fail(kind: Kind, module: string, line: number, message: string, cause?: unknown): never {
  throw new KanameError(kind, module, line, message, cause);
}

/** A message as kaname writes it: an English sentence begins with a capital; Japanese has a space
 * between an ASCII word and the Japanese beside it, outside backquotes. */
function spaced(s: string): string {
  if (LANG !== "ja") {
    const c = s.charAt(0);
    return c >= "a" && c <= "z" ? c.toUpperCase() + s.slice(1) : s;
  }
  const cs = Array.from(s);
  let out = "";
  let code = false;
  for (let i = 0; i < cs.length; i++) {
    const c = cs[i];
    if (c === "`") code = !code;
    if (i > 0 && !code) {
      const p = cs[i - 1];
      if ((latin(p) && japanese(c)) || (japanese(p) && latin(c))) out += " ";
    }
    out += c;
  }
  return out;
}

function latin(c: string): boolean {
  return (c >= "a" && c <= "z") || (c >= "A" && c <= "Z");
}

function japanese(c: string): boolean {
  const u = c.codePointAt(0) ?? 0;
  return (u >= 0x3040 && u <= 0x30ff) || (u >= 0x3400 && u <= 0x4dbf) || (u >= 0x4e00 && u <= 0x9fff) || u === 0x3005;
}

// ── Fractions ───────────────────────────────────────────────────────────────

/** A value that is not a whole number of its unit: an exact fraction, `d` above 0, in lowest terms. */
export type Q = { readonly n: bigint; readonly d: bigint };

const MAX = (1n << 63n) - 1n;
const MIN = -(1n << 63n);

function gcd(a: bigint, b: bigint): bigint {
  if (a < 0n) a = -a;
  if (b < 0n) b = -b;
  while (b !== 0n) {
    const t = a % b;
    a = b;
    b = t;
  }
  return a === 0n ? 1n : a;
}

/** `n / d`, in lowest terms. */
export function q(n: bigint, d: bigint = 1n): Q {
  if (d < 0n) {
    n = -n;
    d = -d;
  }
  const g = gcd(n, d);
  return { n: n / g, d: d / g };
}

export function add(a: Q, b: Q): Q {
  return q(a.n * b.d + b.n * a.d, a.d * b.d);
}

export function sub(a: Q, b: Q): Q {
  return q(a.n * b.d - b.n * a.d, a.d * b.d);
}

export function mul(a: Q, b: Q): Q {
  return q(a.n * b.n, a.d * b.d);
}

/** `a / b`, `b` not 0: where it can be, the module asks [`over`] first. */
export function div(a: Q, b: Q): Q {
  return q(a.n * b.d, a.d * b.n);
}

export function neg(a: Q): Q {
  return { n: -a.n, d: a.d };
}

/** -1, 0 or 1, as `a` is below, at or above `b`. */
export function cmp(a: Q, b: Q): number {
  const x = a.n * b.d;
  const y = b.n * a.d;
  return x < y ? -1 : x > y ? 1 : 0;
}

/** A fraction whose type says it is a whole number of its unit, as that number. */
export function whole(a: Q): bigint {
  if (a.d !== 1n) throw new Error(`kaname: ${a.n}/${a.d} is no whole number`);
  return a.n;
}

/** A rate, as the whole number of its step it travels as. */
export function steps(a: Q, step: Q): bigint {
  return whole(div(a, step));
}

/** A rate that travels as `k` steps of `step`. */
export function rate(k: bigint, step: Q): Q {
  return mul(q(k), step);
}

// ── The rule of int64 (kaname's DESIGN 5.6) ───────────────────────────────

/** A whole number held to int64: the error `overflow` when it is outside. */
export function heldInt(x: bigint, module: string, line: number, message: string): bigint {
  if (x < MIN || x > MAX) fail("overflow", module, line, message);
  return x;
}

/** A fraction held to int64: its numerator in int64, and its denominator from 1 to 2^63 - 1. */
export function held(x: Q, module: string, line: number, message: string): Q {
  if (x.n < MIN || x.n > MAX || x.d > MAX) fail("overflow", module, line, message);
  return x;
}

/** `a / b`, with the error `zero` when `b` is 0. */
export function over(a: Q, b: Q, module: string, line: number, message: string): Q {
  if (b.n === 0n) fail("zero", module, line, message);
  return div(a, b);
}

// ── Rounding (kaname's DESIGN 3.3) ────────────────────────────────────────

export type Way = "up" | "down" | "half_up" | "half_down" | "half_even";

/** `x` rounded `way` to a whole number of the step `g` (above 0): the steps are |n·gd| over d·gn,
 * the quotient taken or one past it as `way` says, the sign put back. No division of the language
 * decides a fraction's way. */
export function round(x: Q, way: Way, g: Q): Q {
  const top = x.n * g.d;
  const y = x.d * g.n;
  const a = top < 0n ? -top : top;
  let k = a / y;
  const r = a % y;
  let past = false;
  if (r !== 0n) {
    switch (way) {
      case "up":
        past = true;
        break;
      case "down":
        past = false;
        break;
      case "half_up":
        past = 2n * r >= y;
        break;
      case "half_down":
        past = 2n * r > y;
        break;
      case "half_even":
        past = 2n * r > y || (2n * r === y && k % 2n === 1n);
        break;
    }
  }
  if (past) k += 1n;
  if (top < 0n) k = -k;
  return mul(q(k), g);
}

/** Where a value that leaves int64 or a divisor of 0 is said: the module, the line, the message. */
export type At = readonly [string, number, string];

/** `allocate(t, c, s)`: `t × c / s` rounded toward 0 to a whole unit (rulec's §15.102), each value on
 * the way held to int64 where `overflow` says, and the error `zero` where `zero` says. */
export function allocate(t: Q, c: Q, s: Q, overflow: At | null, zero: At | null): Q {
  let p = mul(t, c);
  if (overflow !== null) p = held(p, ...overflow);
  if (zero !== null && s.n === 0n) fail("zero", ...zero);
  let part = div(p, s);
  if (overflow !== null) part = held(part, ...overflow);
  let r = round(part, "down", q(1n));
  if (overflow !== null) r = held(r, ...overflow);
  return r;
}

/** `mod(x, n)`: the whole part of `x` (toward −∞) modulo the positive whole `n`, from 0 to n − 1. */
export function mod(x: Q, n: bigint): bigint {
  let w = x.n / x.d;
  if (x.n % x.d !== 0n && x.n < 0n) w -= 1n;
  const r = w % n;
  return r < 0n ? r + n : r;
}

/** `isqrt(x)`: the largest whole number whose square is not above `x`; null below 0. */
export function isqrt(x: Q): bigint | null {
  if (x.n < 0n) return null;
  const v = x.n / x.d;
  let lo = 0n;
  let hi = v < 1n << 63n ? v : 1n << 63n;
  while (lo < hi) {
    const mid = lo + (hi - lo + 1n) / 2n;
    if (mid * mid <= v) lo = mid;
    else hi = mid - 1n;
  }
  return lo;
}

// ── Dates (kaname's DESIGN 3.4) ───────────────────────────────────────────

/** The day number of a date `YYYY-MM-DD` (days since 1970-01-01): Howard Hinnant's days_from_civil. */
export function dayOf(s: string): number {
  const y0 = Number(s.slice(0, 4));
  const m = Number(s.slice(5, 7));
  const d = Number(s.slice(8, 10));
  const y = m <= 2 ? y0 - 1 : y0;
  const era = Math.floor(y / 400);
  const yoe = y - era * 400;
  const doy = Math.floor((153 * (m > 2 ? m - 3 : m + 9) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

/** A day number as a date, `YYYY-MM-DD`: civil_from_days. */
export function dateOf(z: number): string {
  const zz = z + 719468;
  const era = Math.floor(zz / 146097);
  const doe = zz - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const y = yoe + era * 400;
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const m = mp < 10 ? mp + 3 : mp - 9;
  const yy = m <= 2 ? y + 1 : y;
  return String(yy).padStart(4, "0") + "-" + String(m).padStart(2, "0") + "-" + String(d).padStart(2, "0");
}

/** `b - a`: the days from the date `a` to the date `b`. */
export function between(b: string, a: string): bigint {
  return BigInt(dayOf(b) - dayOf(a));
}

// ── Values as a message writes them ───────────────────────────────────────

/** A fraction written out: a whole number as it is, a decimal where the denominator is made of 2s and
 * 5s, and `n/d` otherwise. */
export function ratText(x: Q): string {
  if (x.d === 1n) return String(x.n);
  const sign = x.n < 0n ? "-" : "";
  const num = x.n < 0n ? -x.n : x.n;
  let rest = x.d;
  let twos = 0;
  let fives = 0;
  while (rest % 2n === 0n) {
    rest /= 2n;
    twos++;
  }
  while (rest % 5n === 0n) {
    rest /= 5n;
    fives++;
  }
  const places = Math.max(twos, fives);
  const scale = 10n ** BigInt(places);
  // as ritsu writes it: a decimal past 128 bits is written as the fraction
  if (rest === 1n && places <= 38 && num * scale < 1n << 128n) {
    const v = (num * scale) / x.d;
    return `${sign}${v / scale}.${String(v % scale).padStart(places, "0")}`;
  }
  return `${sign}${num}/${x.d}`;
}

/** A number as a literal of its unit writes it: `1150GBPc`, `0.5%`, `30`. */
export function numText(x: Q, unit: string): string {
  if (unit === "%") return ratText(mul(x, q(100n))) + "%";
  return ratText(x) + unit;
}

/** Where a value a function is given or answers is outside what its place declares: the place
 * (a parameter, and the field or the element of it), the value as a message writes it, and the
 * range; or, for a list, its count. */
export interface Outside {
  readonly path: string;
  readonly shown: string;
  readonly range: string;
  readonly count: number | null;
}

/** An element of what `running_totals` answers (kaname's DESIGN 3.6). */
export interface Running<T, W> {
  readonly item: T;
  readonly before: W;
  readonly upto: W;
  readonly total: W;
}
