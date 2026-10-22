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

/** Whether two values of one type are the same (kaname's DESIGN 23.5): a record and a value of a sum
 * field by field, a list element by element, a dict pair by pair, a set member by member, a fraction
 * by its numerator and denominator (each in lowest terms), anything else as `===` compares it. */
export function eq(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  // a dict by its pairs, a set by its members (kaname's DESIGN 24.2): their keys are compared by value
  if (a instanceof Map || b instanceof Map) {
    if (!(a instanceof Map) || !(b instanceof Map) || a.size !== b.size) return false;
    for (const [k, v] of a) if (!b.has(k) || !eq(v, b.get(k))) return false;
    return true;
  }
  if (a instanceof Set || b instanceof Set) {
    if (!(a instanceof Set) || !(b instanceof Set) || a.size !== b.size) return false;
    for (const x of a) if (!b.has(x)) return false;
    return true;
  }
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (!eq(a[i], b[i])) return false;
    return true;
  }
  const x = a as Record<string, unknown>;
  const y = b as Record<string, unknown>;
  const kx = Object.keys(x);
  if (kx.length !== Object.keys(y).length) return false;
  for (const k of kx) if (!Object.prototype.hasOwnProperty.call(y, k) || !eq(x[k], y[k])) return false;
  return true;
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

/** Whether `s` is a day of the calendar written `YYYY-MM-DD`, from 0001-01-01 to 9999-12-31: what a
 * date whose range is not written takes (kaname's DESIGN 22.5). */
export function isDate(s: string): boolean {
  if (!/^[0-9]{4}-[0-9]{2}-[0-9]{2}$/.test(s) || s < "0001-01-01") return false;
  return dateOf(dayOf(s)) === s;
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
  /** Whether the count is of a string's code points (kaname's DESIGN 25.1). */
  readonly chars?: boolean;
  /** Whether the value is a string that is not well formed: a lone surrogate. */
  readonly bad?: boolean;
}

// ── Orders, dicts, sets and lists (kaname's DESIGN 24) ───────────────────

/** How two values compare: below 0, 0 or above 0, as `a` is before, with or after `b`. */
export type Order<T> = (a: T, b: T) => number;

/** The order of two whole numbers. */
export function cmpInt(a: bigint, b: bigint): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** The order of two strings by their code points (kaname's DESIGN 24.1): JavaScript's `<` compares
 * UTF-16 units, which puts a character past U+FFFF before U+E000 to U+FFFF. A date written
 * `YYYY-MM-DD` is in its order too. */
export function cmpStr(a: string, b: string): number {
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    const x = a.codePointAt(i) ?? 0;
    const y = b.codePointAt(j) ?? 0;
    if (x !== y) return x < y ? -1 : 1;
    i += x > 0xffff ? 2 : 1;
    j += y > 0xffff ? 2 : 1;
  }
  return i < a.length ? 1 : j < b.length ? -1 : 0;
}

/** `false` before `true`. */
export function cmpBool(a: boolean, b: boolean): number {
  return a === b ? 0 : a ? 1 : -1;
}

/** The order of the values of an enum: the order they are declared in. */
export function byRank<T>(rank: (v: T) => number): Order<T> {
  return (a: T, b: T) => rank(a) - rank(b);
}

/** A fraction as a key a `Set` tells apart by its value: its numerator and denominator. */
export function qKey(a: Q): string {
  return `${a.n}/${a.d}`;
}

/** A value as the key a `Set` tells apart: itself. */
export function same<T>(a: T): T {
  return a;
}

/** The keys of a dict or the members of a set, in their order. */
export function sortedKeys<K>(m: ReadonlyMap<K, unknown> | ReadonlySet<K>, cmp: Order<K>): K[] {
  return [...m.keys()].sort(cmp);
}

/** The values of a dict, in the order of their keys. */
export function valuesOf<K, V>(m: ReadonlyMap<K, V>, cmp: Order<K>): V[] {
  return sortedKeys(m, cmp).map((k: K) => m.get(k) as V);
}

/** The pairs of a dict as entries, in the order of their keys. */
export function entriesOf<K, V>(m: ReadonlyMap<K, V>, cmp: Order<K>): Entry<K, V>[] {
  return sortedKeys(m, cmp).map((k: K) => ({ key: k, value: m.get(k) as V }));
}

/** A dict of these pairs, the later of two of one key kept, in the order of its keys: a `Map` goes
 * round in the order its keys are put in. */
export function mapOf<K, V>(pairs: Iterable<readonly [K, V]>, cmp: Order<K>): ReadonlyMap<K, V> {
  const m = new Map<K, V>(pairs);
  return new Map<K, V>([...m.entries()].sort((a: [K, V], b: [K, V]) => cmp(a[0], b[0])));
}

/** A set of these members, each once, in their order. */
export function setOf<T>(xs: Iterable<T>, cmp: Order<T>): ReadonlySet<T> {
  return new Set<T>([...new Set<T>(xs)].sort(cmp));
}

/** The value of a key; null where the dict has none. */
export function get<K, V>(m: ReadonlyMap<K, V>, k: K): V | null {
  const v = m.get(k);
  return v === undefined ? null : v;
}

export function put<K, V>(m: ReadonlyMap<K, V>, k: K, v: V, cmp: Order<K>): ReadonlyMap<K, V> {
  return mapOf<K, V>([...m.entries(), [k, v]], cmp);
}

export function remove<K, V>(m: ReadonlyMap<K, V>, k: K, cmp: Order<K>): ReadonlyMap<K, V> {
  return mapOf<K, V>([...m.entries()].filter((p: [K, V]) => p[0] !== k), cmp);
}

/** The pairs of both, `b`'s value for a key of both. */
export function merge<K, V>(a: ReadonlyMap<K, V>, b: ReadonlyMap<K, V>, cmp: Order<K>): ReadonlyMap<K, V> {
  return mapOf<K, V>([...a.entries(), ...b.entries()], cmp);
}

export function insert<T>(s: ReadonlySet<T>, x: T, cmp: Order<T>): ReadonlySet<T> {
  return setOf<T>([...s, x], cmp);
}

export function without<T>(s: ReadonlySet<T>, x: T, cmp: Order<T>): ReadonlySet<T> {
  return setOf<T>([...s].filter((y: T) => y !== x), cmp);
}

export function union<T>(a: ReadonlySet<T>, b: ReadonlySet<T>, cmp: Order<T>): ReadonlySet<T> {
  return setOf<T>([...a, ...b], cmp);
}

export function intersect<T>(a: ReadonlySet<T>, b: ReadonlySet<T>, cmp: Order<T>): ReadonlySet<T> {
  return setOf<T>([...a].filter((x: T) => b.has(x)), cmp);
}

export function difference<T>(a: ReadonlySet<T>, b: ReadonlySet<T>, cmp: Order<T>): ReadonlySet<T> {
  return setOf<T>([...a].filter((x: T) => !b.has(x)), cmp);
}

/** A dict with each key and each value read anew: a key in another step of its rate, … */
export function remap<K, V, L, W>(m: ReadonlyMap<K, V>, fk: (k: K) => L, fv: (v: V) => W): ReadonlyMap<L, W> {
  return new Map<L, W>([...m.entries()].map((p: [K, V]): [L, W] => [fk(p[0]), fv(p[1])]));
}

/** A set with each member read anew. */
export function remapSet<T, U>(s: ReadonlySet<T>, f: (x: T) => U): ReadonlySet<U> {
  return new Set<U>([...s].map(f));
}

/** An element put among those of its key: what `group_by` makes, the elements in the order of the
 * list. */
export function groupPush<K, T>(g: Map<K, T[]>, k: K, x: T): void {
  const l = g.get(k);
  if (l === undefined) g.set(k, [x]);
  else l.push(x);
}

/** A whole number cut to 0 and to `n`: where a list is cut (kaname's DESIGN 24.3). */
function cut(i: bigint, n: number): number {
  return i < 0n ? 0 : i > BigInt(n) ? n : Number(i);
}

/** The element at `i` (from 0); null past either end. */
export function at<T>(xs: readonly T[], i: bigint): T | null {
  return i >= 0n && i < BigInt(xs.length) ? xs[Number(i)] : null;
}

export function first<T>(xs: readonly T[]): T | null {
  return xs.length > 0 ? xs[0] : null;
}

export function last<T>(xs: readonly T[]): T | null {
  return xs.length > 0 ? xs[xs.length - 1] : null;
}

/** The elements from `a` up to before `b`, both cut to the list. */
export function slice<T>(xs: readonly T[], a: bigint, b: bigint): T[] {
  const i = cut(a, xs.length);
  const j = cut(b, xs.length);
  return i < j ? xs.slice(i, j) : [];
}

export function take<T>(xs: readonly T[], n: bigint): T[] {
  return xs.slice(0, cut(n, xs.length));
}

export function drop<T>(xs: readonly T[], n: bigint): T[] {
  return xs.slice(cut(n, xs.length));
}

export function reverse<T>(xs: readonly T[]): T[] {
  return [...xs].reverse();
}

export function append<T>(xs: readonly T[], x: T): T[] {
  return [...xs, x];
}

/** Lists one after another. */
export function concat<T>(...xss: (readonly T[])[]): T[] {
  const out: T[] = [];
  for (const xs of xss) out.push(...xs);
  return out;
}

export function flatten<T>(xss: readonly (readonly T[])[]): T[] {
  return concat<T>(...xss);
}

/** The elements in their order: `sort` keeps two of one key in the order of the list. */
export function sorted<T>(xs: readonly T[], cmp: Order<T>): T[] {
  return [...xs].sort(cmp);
}

/** The elements in the order of their keys, from the greatest where `desc`, two of one key in the
 * order of the list either way. */
export function byKey<K, T>(ps: [K, T][], cmp: Order<K>, desc: boolean): T[] {
  return ps.sort((a: [K, T], b: [K, T]) => (desc ? cmp(b[0], a[0]) : cmp(a[0], b[0]))).map((p: [K, T]) => p[1]);
}

/** The first of each key, in the order of the list. */
export function distinct<T>(xs: readonly T[], key: (x: T) => unknown): T[] {
  const seen = new Set<unknown>();
  const out: T[] = [];
  for (const x of xs) {
    const k = key(x);
    if (!seen.has(k)) {
      seen.add(k);
      out.push(x);
    }
  }
  return out;
}

/** `numbers(a, b)`: `a` to `b`, both in; none where `b` is before `a`; the count held to int64
 * where `over` says. */
export function numbers(a: bigint, b: bigint, over: At | null): bigint[] {
  if (b < a) return [];
  if (over !== null && b - a + 1n > MAX) fail("overflow", ...over);
  const out: bigint[] = [];
  for (let i = a; i <= b; i++) out.push(i);
  return out;
}

/** `mod(x, n)` of a divisor that is a value: the whole part of `x` (toward −∞) modulo `n`, from 0
 * to |n| − 1 (Euclid's, kaname's DESIGN 24.3); the error `zero` where `zero` says and `n` is 0. */
export function modOf(x: Q, n: bigint, zero: At | null): bigint {
  if (zero !== null && n === 0n) fail("zero", ...zero);
  let w = x.n / x.d;
  if (x.n % x.d !== 0n && x.n < 0n) w -= 1n;
  const m = n < 0n ? -n : n;
  const r = w % m;
  return r < 0n ? r + m : r;
}

/** An entry of a dict: an element of what `entries` answers (kaname's DESIGN 24.3). */
export interface Entry<K, V> {
  readonly key: K;
  readonly value: V;
}

/** An element of what `indexed` answers: the index from 0, and the element. */
export interface Indexed<T> {
  readonly index: bigint;
  readonly item: T;
}

/** An element of what `running_totals` answers (kaname's DESIGN 3.6). */
export interface Running<T, W> {
  readonly item: T;
  readonly before: W;
  readonly upto: W;
  readonly total: W;
}

/** A value of `result[T, E]` (kaname's DESIGN 26.3): `ok` with its value, or `err` with its error. */
export type Result<T, E> = Ok<T> | Err<E>;

/** The value `ok` of a result: what it holds. */
export interface Ok<T> {
  readonly kind: "ok";
  readonly value: T;
}

/** The value `err` of a result: why it is no `ok`. */
export interface Err<E> {
  readonly kind: "err";
  readonly error: E;
}

// ── Strings (kaname's DESIGN 25) ──────────────────────────────────────────

// A string is a sequence of code points: JavaScript counts UTF-16 units, so the length, the cuts and
// the places are counted here with `for … of` and `Array.from`, which go a code point at a time. A
// search for a string in a well-formed string (`indexOf`, `includes`, `endsWith`, `split`) finds it
// at the start of a code point, and is used as it is.

/** The code points of Unicode's White_Space, which `trim` and `words` take for white space. */
const SPACES: ReadonlySet<number> = new Set([
  0x9, 0xa, 0xb, 0xc, 0xd, 0x20, 0x85, 0xa0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007,
  0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000,
]);

function isSpace(c: string): boolean {
  return SPACES.has(c.codePointAt(0) ?? 0);
}

/** Whether a string is well formed: no surrogate without its pair. A host may give one that is not. */
export function isText(s: string): boolean {
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      const d = s.charCodeAt(i + 1);
      if (!(d >= 0xdc00 && d <= 0xdfff)) return false;
      i++;
    } else if (c >= 0xdc00 && c <= 0xdfff) {
      return false;
    }
  }
  return true;
}

/** The number of code points. */
export function cpCount(s: string): bigint {
  let n = 0;
  for (const _ of s) n++;
  return BigInt(n);
}

/** The code points from `a` up to before `b`, both cut to the string. */
export function cpSlice(s: string, a: bigint, b: bigint): string {
  const cs = Array.from(s);
  const i = cut(a, cs.length);
  const j = cut(b, cs.length);
  return i < j ? cs.slice(i, j).join("") : "";
}

export function cpTake(s: string, n: bigint): string {
  return cpSlice(s, 0n, n);
}

export function cpDrop(s: string, n: bigint): string {
  const cs = Array.from(s);
  return cs.slice(cut(n, cs.length)).join("");
}

/** Where `t` first stands in `s`, in code points; 0 for an empty `t`, null where it is not there. */
export function cpIndexOf(s: string, t: string): bigint | null {
  const i = s.indexOf(t);
  return i < 0 ? null : cpCount(s.slice(0, i));
}

export function chars(s: string): string[] {
  return Array.from(s);
}

export function codePoints(s: string): bigint[] {
  return Array.from(s, (c) => BigInt(c.codePointAt(0) ?? 0));
}

/** The string of these code points; null where one is no Unicode scalar value. */
export function toText(ns: readonly bigint[]): string | null {
  let out = "";
  for (const n of ns) {
    if (n < 0n || n > 0x10ffffn || (n >= 0xd800n && n <= 0xdfffn)) return null;
    out += String.fromCodePoint(Number(n));
  }
  return out;
}

/** `s` cut at each `sep`, the empty strings at the ends kept; each code point where `sep` is empty. */
export function split(s: string, sep: string): string[] {
  return sep === "" ? Array.from(s) : s.split(sep);
}

/** The runs of what is not white space. */
export function words(s: string): string[] {
  const out: string[] = [];
  let w = "";
  for (const c of s) {
    if (isSpace(c)) {
      if (w !== "") out.push(w);
      w = "";
    } else {
      w += c;
    }
  }
  if (w !== "") out.push(w);
  return out;
}

/** Every `old` from the left made `nw`; `s` as it is where `old` is empty. Not `replaceAll`, which
 * reads `$&` and the like in what it puts. */
export function replace(s: string, old: string, nw: string): string {
  return old === "" ? s : s.split(old).join(nw);
}

export function trimStart(s: string): string {
  const cs = Array.from(s);
  let i = 0;
  while (i < cs.length && isSpace(cs[i])) i++;
  return cs.slice(i).join("");
}

export function trimEnd(s: string): string {
  const cs = Array.from(s);
  let j = cs.length;
  while (j > 0 && isSpace(cs[j - 1])) j--;
  return cs.slice(0, j).join("");
}

export function trim(s: string): string {
  return trimEnd(trimStart(s));
}

/** A to Z made a to z, nothing else changed. */
export function lowerAscii(s: string): string {
  return s.replace(/[A-Z]/g, (c) => String.fromCharCode(c.charCodeAt(0) + 32));
}

/** a to z made A to Z, nothing else changed. */
export function upperAscii(s: string): string {
  return s.replace(/[a-z]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 32));
}

/** `fill` repeated before `s` (after it, with `atEnd`) to make it `width` code points long, the last
 * repetition cut; `s` as it is where `fill` is empty or `s` is that long already. */
export function pad(s: string, width: bigint, fill: string, atEnd: boolean): string {
  const n = cpCount(s);
  if (fill === "" || n >= width) return s;
  const fs = Array.from(fill);
  const k = Number(width - n);
  let added = "";
  for (let i = 0; i < k; i++) added += fs[i % fs.length];
  return atEnd ? s + added : added + s;
}

/** The number `s` writes: one `+` or `-` at most, then ASCII digits, nothing else, within int64. */
export function parseNumber(s: string): bigint | null {
  if (!/^[+-]?[0-9]+$/.test(s)) return null;
  const v = BigInt(s);
  return v < MIN || v > MAX ? null : v;
}

/** The date `s` writes: `YYYY-MM-DD` alone, a day of 0001-01-01 to 9999-12-31. */
export function parseDate(s: string): string | null {
  return isDate(s) ? s : null;
}
