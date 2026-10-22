// @HEAD@

// Package kaname is what every module of kaname in this package shares (kaname's DESIGN 10.1): exact
// fractions, the arithmetic held to int64, rulec's five ways of rounding, dates, how a value is
// written in a message, and the error the modules return. It imports the standard library alone.
//
// A function of a module that can fail returns its *KanameError as an error; inside the module, a
// place that fails panics with it, and the function the host called recovers it (Recover).
package kaname

import (
	"cmp"
	"fmt"
	"math"
	"math/big"
	"reflect"
	"slices"
	"strconv"
	"strings"
	"unicode/utf8"
)

// lang is the language the messages are written in: the --lang of ritsu gen.
const lang = "@LANG@"

// KanameError is why a function of kaname stops. Kind is input (the argument a host gave is outside
// what the function takes), or a kind of the effects: rule, date, range, overflow, zero (kaname's
// DESIGN 4.2). Module and Line are where in the .kn it stopped; Err is what another language
// returned, when it is that language's refusal.
type KanameError struct {
	Kind    string
	Module  string
	Line    int
	Message string
	Err     error
}

func (e *KanameError) Error() string { return e.Message }

// Unwrap is the error of the rule or the date that refused, when there is one.
func (e *KanameError) Unwrap() error { return e.Err }

// Fail stops the function with the error of kind at line of module (a panic, which the function
// the host called recovers).
func Fail(kind, module string, line int, message string) {
	panic(&KanameError{Kind: kind, Module: module, Line: line, Message: spaced(message)})
}

// Refused stops the function with the error of kind at line of module, caused by err: what a
// rule's or a date's generated code returned.
func Refused(kind, module string, line int, message string, err error) {
	panic(&KanameError{Kind: kind, Module: module, Line: line, Message: spaced(message), Err: err})
}

// Recover turns the panic of a *KanameError into the error *err, deferred by a function that
// returns one; any other panic goes on.
func Recover(err *error) {
	if r := recover(); r != nil {
		if k, ok := r.(*KanameError); ok {
			*err = k
			return
		}
		panic(r)
	}
}

// spaced writes a message as kaname does: an English sentence begins with a capital; Japanese has a
// space between an ASCII word and the Japanese beside it, outside backquotes.
func spaced(s string) string {
	if lang != "ja" {
		if s != "" && s[0] >= 'a' && s[0] <= 'z' {
			return string(s[0]-'a'+'A') + s[1:]
		}
		return s
	}
	cs := []rune(s)
	var b strings.Builder
	code := false
	for i, c := range cs {
		if c == '`' {
			code = !code
		}
		if i > 0 && !code {
			p := cs[i-1]
			if (latin(p) && japanese(c)) || (japanese(p) && latin(c)) {
				b.WriteRune(' ')
			}
		}
		b.WriteRune(c)
	}
	return b.String()
}

func latin(c rune) bool { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') }

func japanese(c rune) bool {
	return (c >= 0x3040 && c <= 0x30ff) || (c >= 0x3400 && c <= 0x4dbf) || (c >= 0x4e00 && c <= 0x9fff) || c == 0x3005
}

// ── Whole numbers and fractions ──────────────────────────────────────────────

// Q is a value that is not a whole number of its unit: an exact fraction, kept in lowest terms.
type Q = *big.Rat

// Frac is n/d.
func Frac(n, d int64) Q { return big.NewRat(n, d) }

// Of is a whole number as a fraction.
func Of(x int64) Q { return new(big.Rat).SetInt64(x) }

func QAdd(a, b Q) Q { return new(big.Rat).Add(a, b) }
func QSub(a, b Q) Q { return new(big.Rat).Sub(a, b) }
func QMul(a, b Q) Q { return new(big.Rat).Mul(a, b) }
func QNeg(a Q) Q    { return new(big.Rat).Neg(a) }

// QDiv is a / b, b not 0: where it can be, the module asks Over first.
func QDiv(a, b Q) Q { return new(big.Rat).Quo(a, b) }

// Cmp is -1, 0 or 1, as a is below, at or above b.
func Cmp(a, b Q) int { return a.Cmp(b) }

// Whole is a fraction whose type says it is a whole number of its unit, as that number.
func Whole(a Q) int64 {
	if !a.IsInt() || !a.Num().IsInt64() {
		panic(fmt.Sprintf("kaname: %s is no whole number of int64", a.RatString()))
	}
	return a.Num().Int64()
}

// Steps is a rate, as the whole number of its step it travels as.
func Steps(a, step Q) int64 { return Whole(QDiv(a, step)) }

// Rate is the rate that travels as k steps of step.
func Rate(k int64, step Q) Q { return QMul(Of(k), step) }

// Ints is a list of whole numbers in another type of them (an amount of a unit, a plain int64).
func Ints[B, A ~int64](xs []A) []B {
	out := make([]B, len(xs))
	for i, x := range xs {
		out[i] = B(x)
	}
	return out
}

// Int is a whole number that may be absent, in another type of them.
func Int[B, A ~int64](p *A) *B {
	if p == nil {
		return nil
	}
	v := B(*p)
	return &v
}

// Some is a value that is there, where one may be absent.
func Some[T any](v T) *T { return &v }

// ── The rule of int64 (kaname's DESIGN 5.6) ──────────────────────────────────

// Add is a + b, held to int64.
func Add(a, b int64, module string, line int, message string) int64 {
	c := a + b
	if (b > 0 && c < a) || (b < 0 && c > a) {
		Fail("overflow", module, line, message)
	}
	return c
}

// Sub is a - b, held to int64.
func Sub(a, b int64, module string, line int, message string) int64 {
	c := a - b
	if (b < 0 && c < a) || (b > 0 && c > a) {
		Fail("overflow", module, line, message)
	}
	return c
}

// Mul is a × b, held to int64.
func Mul(a, b int64, module string, line int, message string) int64 {
	p := new(big.Int).Mul(big.NewInt(a), big.NewInt(b))
	if !p.IsInt64() {
		Fail("overflow", module, line, message)
	}
	return p.Int64()
}

// Neg is -a, held to int64.
func Neg(a int64, module string, line int, message string) int64 {
	if a == math.MinInt64 {
		Fail("overflow", module, line, message)
	}
	return -a
}

// Held is a fraction held to int64: its numerator in int64, and its denominator from 1 to 2^63 - 1.
func Held(x Q, module string, line int, message string) Q {
	if !x.Num().IsInt64() || !x.Denom().IsInt64() {
		Fail("overflow", module, line, message)
	}
	return x
}

// Over is a / b, with the error zero when b is 0.
func Over(a, b Q, module string, line int, message string) Q {
	if b.Sign() == 0 {
		Fail("zero", module, line, message)
	}
	return QDiv(a, b)
}

// ── Rounding (kaname's DESIGN 3.3) ──────────────────────────────────────────

// Round is x rounded way (up, down, half_up, half_down or half_even) to a whole number of the step g
// (above 0): the steps are |n·gd| over d·gn, the quotient taken or one past it as way says, the sign
// put back. Go's own division, which goes toward 0, decides no fraction's way.
func Round(x Q, way string, g Q) Q {
	top := new(big.Int).Mul(x.Num(), g.Denom())
	y := new(big.Int).Mul(x.Denom(), g.Num())
	a := new(big.Int).Abs(top)
	k, r := new(big.Int).QuoRem(a, y, new(big.Int))
	past := false
	if r.Sign() != 0 {
		twice := new(big.Int).Lsh(r, 1)
		switch way {
		case "up":
			past = true
		case "half_up":
			past = twice.Cmp(y) >= 0
		case "half_down":
			past = twice.Cmp(y) > 0
		case "half_even":
			past = twice.Cmp(y) > 0 || (twice.Cmp(y) == 0 && k.Bit(0) == 1)
		}
	}
	if past {
		k.Add(k, big.NewInt(1))
	}
	if top.Sign() < 0 {
		k.Neg(k)
	}
	return QMul(new(big.Rat).SetInt(k), g)
}

// At is where a value that leaves int64 or a divisor of 0 is said: the module, the line, the message.
type At struct {
	Module  string
	Line    int
	Message string
}

// Allocate is allocate(t, c, s): t × c / s rounded toward 0 to a whole unit (rulec's §15.102), each
// value on the way held to int64 where overflow says, and the error zero where zero says.
func Allocate(t, c, s Q, overflow, zero *At) Q {
	held := func(x Q) Q {
		if overflow != nil {
			return Held(x, overflow.Module, overflow.Line, overflow.Message)
		}
		return x
	}
	p := held(QMul(t, c))
	if zero != nil && s.Sign() == 0 {
		Fail("zero", zero.Module, zero.Line, zero.Message)
	}
	part := held(QDiv(p, s))
	return held(Round(part, "down", Of(1)))
}

// Mod is mod(x, n): the whole part of x (toward −∞) modulo the positive whole n, from 0 to n − 1.
func Mod(x Q, n int64) int64 {
	w := new(big.Int).Div(x.Num(), x.Denom()) // Euclidean: toward −∞ for a positive denominator
	r := new(big.Int).Mod(w, big.NewInt(n))
	return r.Int64()
}

// Isqrt is isqrt(x): the largest whole number whose square is not above x; false below 0.
func Isqrt(x Q) (int64, bool) {
	if x.Sign() < 0 {
		return 0, false
	}
	v := new(big.Int).Div(x.Num(), x.Denom())
	return new(big.Int).Sqrt(v).Int64(), true
}

// Root is isqrt(x) where the analysis shows x is 0 or more.
func Root(x Q) int64 {
	v, ok := Isqrt(x)
	if !ok {
		panic("kaname: isqrt of " + x.RatString())
	}
	return v
}

// Must is the answer of a function of another module, its error (a *KanameError) going on as the
// panic the function the host called recovers.
func Must[T any](v T, err error) T {
	if err != nil {
		panic(err)
	}
	return v
}

// ── Dates (kaname's DESIGN 3.4) ──────────────────────────────────────────────

// Date is a day of the calendar, with no time zone, from 0001-01-01 to 9999-12-31.
type Date struct {
	Year, Month, Day int
}

// Days is the day number of the date: days since 1970-01-01 (Howard Hinnant's days_from_civil).
func (d Date) Days() int64 {
	y, m, dd := int64(d.Year), int64(d.Month), int64(d.Day)
	if m <= 2 {
		y--
	}
	era := y / 400
	if y < 0 && y%400 != 0 {
		era--
	}
	yoe := y - era*400
	mp := (m + 9) % 12
	doy := (153*mp+2)/5 + dd - 1
	doe := yoe*365 + yoe/4 - yoe/100 + doy
	return era*146097 + doe - 719468
}

// DateOf is the date of a day number (civil_from_days).
func DateOf(z int64) Date {
	z += 719468
	era := z / 146097
	if z < 0 && z%146097 != 0 {
		era--
	}
	doe := z - era*146097
	yoe := (doe - doe/1460 + doe/36524 - doe/146096) / 365
	y := yoe + era*400
	doy := doe - (365*yoe + yoe/4 - yoe/100)
	mp := (5*doy + 2) / 153
	d := doy - (153*mp+2)/5 + 1
	m := mp + 3
	if mp >= 10 {
		m = mp - 9
	}
	if m <= 2 {
		y++
	}
	return Date{Year: int(y), Month: int(m), Day: int(d)}
}

// String writes the date YYYY-MM-DD.
func (d Date) String() string { return fmt.Sprintf("%04d-%02d-%02d", d.Year, d.Month, d.Day) }

// ParseDate reads YYYY-MM-DD: a day of 0001-01-01..9999-12-31.
func ParseDate(s string) (Date, error) {
	var d Date
	if len(s) != 10 || s[4] != '-' || s[7] != '-' {
		return d, fmt.Errorf("kaname: %q is not a date written YYYY-MM-DD", s)
	}
	if _, err := fmt.Sscanf(s, "%04d-%02d-%02d", &d.Year, &d.Month, &d.Day); err != nil {
		return d, fmt.Errorf("kaname: %q is not a date written YYYY-MM-DD", s)
	}
	if d.Year < 1 || d.Month < 1 || d.Month > 12 || d.Day < 1 || DateOf(d.Days()) != d {
		return d, fmt.Errorf("kaname: %q is no day of the calendar", s)
	}
	return d, nil
}

// Valid is whether the date is a day of the calendar from 0001-01-01 to 9999-12-31: what a date
// whose range is not written takes (kaname's DESIGN 22.5).
func (d Date) Valid() bool {
	return d.Year >= 1 && d.Year <= 9999 && d.Month >= 1 && d.Month <= 12 && d.Day >= 1 && DateOf(d.Days()) == d
}

// Compare is -1, 0 or 1, as the date a is before, on or after b.
func Compare(a, b Date) int {
	x, y := a.Days(), b.Days()
	switch {
	case x < y:
		return -1
	case x > y:
		return 1
	}
	return 0
}

// Between is b - a: the days from the date a to the date b.
func Between(b, a Date) int64 { return b.Days() - a.Days() }

// ── Values as a message writes them ──────────────────────────────────────────

// RatText writes a fraction: a whole number as it is, a decimal where the denominator is made of 2s
// and 5s, and n/d otherwise.
func RatText(x Q) string {
	if x.IsInt() {
		return x.Num().String()
	}
	sign := ""
	num := new(big.Int).Set(x.Num())
	if num.Sign() < 0 {
		sign = "-"
		num.Neg(num)
	}
	den := x.Denom()
	rest := new(big.Int).Set(den)
	twos, fives := 0, 0
	two, five := big.NewInt(2), big.NewInt(5)
	for new(big.Int).Mod(rest, two).Sign() == 0 {
		rest.Quo(rest, two)
		twos++
	}
	for new(big.Int).Mod(rest, five).Sign() == 0 {
		rest.Quo(rest, five)
		fives++
	}
	places := twos
	if fives > places {
		places = fives
	}
	scale := new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(places)), nil)
	scaled := new(big.Int).Mul(num, scale)
	// as ritsu writes it: a decimal past 128 bits is written as the fraction
	if rest.Cmp(big.NewInt(1)) == 0 && places <= 38 && scaled.BitLen() <= 128 {
		v := new(big.Int).Quo(scaled, den)
		whole, frac := new(big.Int).QuoRem(v, scale, new(big.Int))
		f := frac.String()
		for len(f) < places {
			f = "0" + f
		}
		return sign + whole.String() + "." + f
	}
	return sign + num.String() + "/" + den.String()
}

// NumText writes a number as a literal of its unit: 1150GBPc, 0.5%, 30.
func NumText(x Q, unit string) string {
	if unit == "%" {
		return RatText(QMul(x, Of(100))) + "%"
	}
	return RatText(x) + unit
}

// IntText writes a whole number with its unit.
func IntText(x int64, unit string) string { return NumText(Of(x), unit) }

// Outside is where a value a function is given or answers is outside what its place declares: the
// place (a parameter, and the field or the element of it), the value as a message writes it, and
// the range; or, for a list, its count (Count is -1 for a value).
type Outside struct {
	Path  string
	Shown string
	Range string
	Count int
	// Chars is whether the count is of a string's code points (kaname's DESIGN 25.1).
	Chars bool
	// Bad is whether the value is a string that is not UTF-8.
	Bad bool
}

// Running is an element of what running_totals answers (kaname's DESIGN 3.6).
type Running[T, W any] struct {
	Item   T
	Before W
	Upto   W
	Total  W
}

// Result is a value of result[T, E] (kaname's DESIGN 26.3): an Ok with its value, or an Err with its
// error. Its mark takes the type arguments, so that two results of other types are not one interface.
type Result[T, E any] interface {
	isResult(T, E)
}

// Ok is the value ok of a result: what it holds.
type Ok[T, E any] struct {
	Value T
}

// Err is the value err of a result: why it is no ok.
type Err[T, E any] struct {
	Error E
}

func (Ok[T, E]) isResult(T, E) {}

func (Err[T, E]) isResult(T, E) {}

// Equal is whether two values of one type are the same by their contents (kaname's DESIGN 23.5): what
// a generic function compares, whose type it does not know (DESIGN 26.7). A list, a dict and a set
// with nothing in them are the same whether they are nil or not; a value that may be none by what it
// points to; a value of a sum and a result by the type of its value and its fields.
func Equal(a, b any) bool {
	return equal(reflect.ValueOf(a), reflect.ValueOf(b))
}

func equal(x, y reflect.Value) bool {
	if !x.IsValid() || !y.IsValid() {
		return x.IsValid() == y.IsValid()
	}
	if x.Type() != y.Type() {
		return false
	}
	switch x.Kind() {
	case reflect.Pointer, reflect.Interface:
		if x.IsNil() || y.IsNil() {
			return x.IsNil() == y.IsNil()
		}
		return equal(x.Elem(), y.Elem())
	case reflect.Slice:
		if x.Len() != y.Len() {
			return false
		}
		for i := range x.Len() {
			if !equal(x.Index(i), y.Index(i)) {
				return false
			}
		}
		return true
	case reflect.Map:
		if x.Len() != y.Len() {
			return false
		}
		for it := x.MapRange(); it.Next(); {
			v := y.MapIndex(it.Key())
			if !v.IsValid() || !equal(it.Value(), v) {
				return false
			}
		}
		return true
	case reflect.Struct:
		for i := range x.NumField() {
			if !equal(x.Field(i), y.Field(i)) {
				return false
			}
		}
		return true
	default:
		return x.Equal(y)
	}
}

// Itoa writes a count.
func Itoa(n int) string { return strconv.Itoa(n) }

// Elem is the path of the i-th element of what: what[i].
func Elem(what string, i int) string { return what + "[" + strconv.Itoa(i) + "]" }

// BoolText writes true or false.
func BoolText(b bool) string { return strconv.FormatBool(b) }

// StartsWith is whether s begins with p.
func StartsWith(s, p string) bool { return strings.HasPrefix(s, p) }

// EndsWith is whether s ends with t.
func EndsWith(s, t string) bool { return strings.HasSuffix(s, t) }

// TextContains is whether t stands somewhere in s.
func TextContains(s, t string) bool { return strings.Contains(s, t) }

// ── Orders, dicts, sets and lists (kaname's DESIGN 24) ──────────────────────

// CmpInt is the order of two whole numbers: -1, 0 or 1.
func CmpInt[T ~int64](a, b T) int { return cmp.Compare(a, b) }

// CmpStr is the order of two strings by their code points: Go compares the bytes of UTF-8, whose order
// is theirs (kaname's DESIGN 24.1).
func CmpStr(a, b string) int { return strings.Compare(a, b) }

// CmpBool puts false before true.
func CmpBool(a, b bool) int {
	switch {
	case a == b:
		return 0
	case a:
		return 1
	}
	return -1
}

// ByRank is the order of the values of an enum: the order they are declared in.
func ByRank[T any](rank func(T) int) func(a, b T) int {
	return func(a, b T) int { return cmp.Compare(rank(a), rank(b)) }
}

// QKey is a fraction as a key a map tells apart by its value.
func QKey(a Q) string { return a.RatString() }

// Same is a value as the key a map tells apart: itself.
func Same[T comparable](a T) T { return a }

// SortedKeys is the keys of a dict, or the members of a set, in their order: a map has none of its
// own.
func SortedKeys[K comparable, V any](m map[K]V, c func(a, b K) int) []K {
	ks := make([]K, 0, len(m))
	for k := range m {
		ks = append(ks, k)
	}
	slices.SortFunc(ks, c)
	return ks
}

// ValuesOf is the values of a dict, in the order of their keys.
func ValuesOf[K comparable, V any](m map[K]V, c func(a, b K) int) []V {
	ks := SortedKeys(m, c)
	out := make([]V, len(ks))
	for i, k := range ks {
		out[i] = m[k]
	}
	return out
}

// Entry is an entry of a dict: an element of what entries answers (kaname's DESIGN 24.3).
type Entry[K, V any] struct {
	Key   K
	Value V
}

// Indexed is an element of what indexed answers: the index from 0, and the element.
type Indexed[T any] struct {
	Index int64
	Item  T
}

// EntriesOf is the pairs of a dict as entries, in the order of their keys.
func EntriesOf[K comparable, V any](m map[K]V, c func(a, b K) int) []Entry[K, V] {
	ks := SortedKeys(m, c)
	out := make([]Entry[K, V], len(ks))
	for i, k := range ks {
		out[i] = Entry[K, V]{Key: k, Value: m[k]}
	}
	return out
}

// Get is the value of a key; nil where the dict has none.
func Get[K comparable, V any](m map[K]V, k K) *V {
	if v, ok := m[k]; ok {
		return &v
	}
	return nil
}

// GetOpt is the value of a key of a dict whose values may be none; nil where it has none.
func GetOpt[K comparable, V any](m map[K]*V, k K) *V { return m[k] }

// Has is whether a dict has a key, or a set a member.
func Has[K comparable, V any](m map[K]V, k K) bool {
	_, ok := m[k]
	return ok
}

// Put is a dict with the key put in, or its value replaced.
func Put[K comparable, V any](m map[K]V, k K, v V) map[K]V {
	out := make(map[K]V, len(m)+1)
	for a, b := range m {
		out[a] = b
	}
	out[k] = v
	return out
}

// Remove is a dict with the key taken out, or a set with the member taken out.
func Remove[K comparable, V any](m map[K]V, k K) map[K]V {
	out := make(map[K]V, len(m))
	for a, b := range m {
		if a != k {
			out[a] = b
		}
	}
	return out
}

// Merge is the pairs of both, b's value for a key of both.
func Merge[K comparable, V any](a, b map[K]V) map[K]V {
	out := make(map[K]V, len(a)+len(b))
	for k, v := range a {
		out[k] = v
	}
	for k, v := range b {
		out[k] = v
	}
	return out
}

// Remap is a dict with each key and each value read anew.
func Remap[K comparable, V any, L comparable, W any](m map[K]V, fk func(K) L, fv func(V) W) map[L]W {
	out := make(map[L]W, len(m))
	for k, v := range m {
		out[fk(k)] = fv(v)
	}
	return out
}

// SetOf is a set of these members.
func SetOf[T comparable](xs []T) map[T]struct{} {
	out := make(map[T]struct{}, len(xs))
	for _, x := range xs {
		out[x] = struct{}{}
	}
	return out
}

// Insert is a set with the member put in.
func Insert[T comparable](s map[T]struct{}, x T) map[T]struct{} { return Put(s, x, struct{}{}) }

// Union is the members of either.
func Union[T comparable](a, b map[T]struct{}) map[T]struct{} { return Merge(a, b) }

// Intersect is the members of both.
func Intersect[T comparable](a, b map[T]struct{}) map[T]struct{} {
	out := map[T]struct{}{}
	for x := range a {
		if _, ok := b[x]; ok {
			out[x] = struct{}{}
		}
	}
	return out
}

// Difference is the members of a that are not of b.
func Difference[T comparable](a, b map[T]struct{}) map[T]struct{} {
	out := map[T]struct{}{}
	for x := range a {
		if _, ok := b[x]; !ok {
			out[x] = struct{}{}
		}
	}
	return out
}

// RemapSet is a set with each member read anew.
func RemapSet[T comparable, U comparable](s map[T]struct{}, f func(T) U) map[U]struct{} {
	out := make(map[U]struct{}, len(s))
	for x := range s {
		out[f(x)] = struct{}{}
	}
	return out
}

// cut is a whole number cut to 0 and to n: where a list is cut (kaname's DESIGN 24.3).
func cut(i int64, n int) int {
	if i < 0 {
		return 0
	}
	if i > int64(n) {
		return n
	}
	return int(i)
}

// ElemAt is the element at i (from 0); nil past either end.
func ElemAt[T any](xs []T, i int64) *T {
	if i < 0 || i >= int64(len(xs)) {
		return nil
	}
	v := xs[i]
	return &v
}

// ElemAtOpt is the element at i of a list whose elements may be none; nil past either end.
func ElemAtOpt[T any](xs []*T, i int64) *T {
	if i < 0 || i >= int64(len(xs)) {
		return nil
	}
	return xs[i]
}

// First is the first element; nil for none.
func First[T any](xs []T) *T { return ElemAt(xs, 0) }

// FirstOpt is the first element of a list whose elements may be none.
func FirstOpt[T any](xs []*T) *T { return ElemAtOpt(xs, 0) }

// Last is the last element; nil for none.
func Last[T any](xs []T) *T { return ElemAt(xs, int64(len(xs))-1) }

// LastOpt is the last element of a list whose elements may be none.
func LastOpt[T any](xs []*T) *T { return ElemAtOpt(xs, int64(len(xs))-1) }

// Slice is the elements from a up to before b, both cut to the list: a list of its own.
func Slice[T any](xs []T, a, b int64) []T {
	i, j := cut(a, len(xs)), cut(b, len(xs))
	if i >= j {
		return []T{}
	}
	return slices.Clone(xs[i:j])
}

// Take is the first n elements.
func Take[T any](xs []T, n int64) []T { return Slice(xs, 0, n) }

// Drop is the elements past the first n.
func Drop[T any](xs []T, n int64) []T { return Slice(xs, n, int64(len(xs))) }

// Reverse is the elements from the last.
func Reverse[T any](xs []T) []T {
	out := slices.Clone(xs)
	slices.Reverse(out)
	if out == nil {
		return []T{}
	}
	return out
}

// Append is the list with x after its elements: a list of its own, which no other shares.
func Append[T any](xs []T, x T) []T {
	out := make([]T, len(xs), len(xs)+1)
	copy(out, xs)
	return append(out, x)
}

// Concat is lists one after another.
func Concat[T any](xss ...[]T) []T {
	out := []T{}
	for _, xs := range xss {
		out = append(out, xs...)
	}
	return out
}

// Flatten is the lists of a list one after another.
func Flatten[T any](xss [][]T) []T { return Concat(xss...) }

// Sorted is the elements in their order, two of one key in the order of the list.
func Sorted[T any](xs []T, c func(a, b T) int) []T {
	out := slices.Clone(xs)
	slices.SortStableFunc(out, c)
	if out == nil {
		return []T{}
	}
	return out
}

// Pair is an element with its key, for an order of keys a function gives.
type Pair[K, T any] struct {
	Key  K
	Item T
}

// ByKey is the elements in the order of their keys, from the greatest where desc, two of one key in
// the order of the list either way (Go's sort.Slice keeps no such order; SortStableFunc does).
func ByKey[K, T any](ps []Pair[K, T], c func(a, b K) int, desc bool) []T {
	slices.SortStableFunc(ps, func(a, b Pair[K, T]) int {
		if desc {
			return c(b.Key, a.Key)
		}
		return c(a.Key, b.Key)
	})
	out := make([]T, len(ps))
	for i, p := range ps {
		out[i] = p.Item
	}
	return out
}

// Distinct is the first of each key, in the order of the list.
func Distinct[T any, K comparable](xs []T, key func(T) K) []T {
	seen := map[K]struct{}{}
	out := []T{}
	for _, x := range xs {
		k := key(x)
		if _, ok := seen[k]; !ok {
			seen[k] = struct{}{}
			out = append(out, x)
		}
	}
	return out
}

// Numbers is numbers(a, b): a to b, both in; none where b is before a; the count held to int64
// where over says.
func Numbers(a, b int64, over *At) []int64 {
	if b < a {
		return []int64{}
	}
	if over != nil && new(big.Int).Sub(big.NewInt(b), big.NewInt(a)).Cmp(big.NewInt(math.MaxInt64-1)) > 0 {
		Fail("overflow", over.Module, over.Line, over.Message)
	}
	out := make([]int64, 0, b-a+1)
	for i := a; ; i++ {
		out = append(out, i)
		if i == b {
			break
		}
	}
	return out
}

// ModOf is mod(x, n) of a divisor that is a value: the whole part of x (toward −∞) modulo n, from 0
// to |n| − 1 (Euclid's, kaname's DESIGN 24.3); the error zero where zero says and n is 0.
func ModOf(x Q, n int64, zero *At) int64 {
	if zero != nil && n == 0 {
		Fail("zero", zero.Module, zero.Line, zero.Message)
	}
	w := new(big.Int).Div(x.Num(), x.Denom())
	m := new(big.Int).Abs(big.NewInt(n))
	return new(big.Int).Mod(w, m).Int64()
}

// Join is the strings one after another, sep between each two.
func Join(xs []string, sep string) string { return strings.Join(xs, sep) }

// ── Strings (kaname's DESIGN 25) ──────────────────────────────────────────

// A string is a sequence of code points: Go counts the bytes of UTF-8, so the length, the cuts and the
// places are counted here in runes. The bytes of UTF-8 are in the order of their code points, so `<`
// compares two strings as kaname does, and a search in a string that is UTF-8 finds what it looks
// for at the start of a code point.

// isSpace is whether a code point is one of Unicode's White_Space, which trim and words take for
// white space.
func isSpace(r rune) bool {
	switch r {
	case 0x9, 0xa, 0xb, 0xc, 0xd, 0x20, 0x85, 0xa0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000:
		return true
	}
	return false
}

// IsText is whether a string is UTF-8, which a host's string may not be.
func IsText(s string) bool { return utf8.ValidString(s) }

// CpCount is the number of code points.
func CpCount(s string) int64 { return int64(utf8.RuneCountInString(s)) }

// TextSlice is the code points from a up to before b, both cut to the string.
func TextSlice(s string, a, b int64) string {
	rs := []rune(s)
	i, j := cut(a, len(rs)), cut(b, len(rs))
	if i >= j {
		return ""
	}
	return string(rs[i:j])
}

// TextTake is the first n code points.
func TextTake(s string, n int64) string { return TextSlice(s, 0, n) }

// TextDrop is the code points after the first n.
func TextDrop(s string, n int64) string {
	rs := []rune(s)
	return string(rs[cut(n, len(rs)):])
}

// IndexOf is where t first stands in s, in code points; 0 for an empty t, nil where it is not there.
func IndexOf(s, t string) *int64 {
	i := strings.Index(s, t)
	if i < 0 {
		return nil
	}
	v := int64(utf8.RuneCountInString(s[:i]))
	return &v
}

// Chars is the strings of one code point each.
func Chars(s string) []string {
	out := []string{}
	for _, r := range s {
		out = append(out, string(r))
	}
	return out
}

// CodePoints is the code points, as numbers.
func CodePoints(s string) []int64 {
	out := []int64{}
	for _, r := range s {
		out = append(out, int64(r))
	}
	return out
}

// ToText is the string of these code points; nil where one is no Unicode scalar value.
func ToText(ns []int64) *string {
	var b strings.Builder
	for _, n := range ns {
		if n < 0 || n > 0x10ffff || (n >= 0xd800 && n <= 0xdfff) {
			return nil
		}
		b.WriteRune(rune(n))
	}
	out := b.String()
	return &out
}

// Split is s cut at each sep, the empty strings at the ends kept; each code point where sep is empty.
func Split(s, sep string) []string {
	if sep == "" {
		return Chars(s)
	}
	return strings.Split(s, sep)
}

// Words is the runs of what is not white space.
func Words(s string) []string {
	out := strings.FieldsFunc(s, isSpace)
	if out == nil {
		return []string{}
	}
	return out
}

// Replace is every old from the left made nw; s as it is where old is empty.
func Replace(s, old, nw string) string {
	if old == "" {
		return s
	}
	return strings.ReplaceAll(s, old, nw)
}

func Trim(s string) string      { return strings.TrimFunc(s, isSpace) }
func TrimStart(s string) string { return strings.TrimLeftFunc(s, isSpace) }
func TrimEnd(s string) string   { return strings.TrimRightFunc(s, isSpace) }

// LowerASCII is s with A to Z made a to z, nothing else changed.
func LowerASCII(s string) string {
	return strings.Map(func(r rune) rune {
		if r >= 'A' && r <= 'Z' {
			return r + 32
		}
		return r
	}, s)
}

// UpperASCII is s with a to z made A to Z, nothing else changed.
func UpperASCII(s string) string {
	return strings.Map(func(r rune) rune {
		if r >= 'a' && r <= 'z' {
			return r - 32
		}
		return r
	}, s)
}

// Pad is fill repeated before s (after it, with atEnd) to make it width code points long, the last
// repetition cut; s as it is where fill is empty or s is that long already.
func Pad(s string, width int64, fill string, atEnd bool) string {
	n := CpCount(s)
	if fill == "" || n >= width {
		return s
	}
	fs := []rune(fill)
	added := make([]rune, 0, width-n)
	for i := int64(0); i < width-n; i++ {
		added = append(added, fs[i%int64(len(fs))])
	}
	if atEnd {
		return s + string(added)
	}
	return string(added) + s
}

// digits is whether s is one ASCII digit or more, and nothing else.
func digits(s string) bool {
	if s == "" {
		return false
	}
	for i := 0; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return false
		}
	}
	return true
}

// ParseNumber is the number s writes: one + or - at most, then ASCII digits, nothing else, within
// int64; nil otherwise.
func ParseNumber(s string) *int64 {
	d := s
	if len(d) > 0 && (d[0] == '+' || d[0] == '-') {
		d = d[1:]
	}
	if !digits(d) {
		return nil
	}
	v, err := strconv.ParseInt(s, 10, 64)
	if err != nil {
		return nil
	}
	return &v
}

// ParseText is the date s writes: YYYY-MM-DD alone, a day of 0001-01-01 to 9999-12-31; nil otherwise.
func ParseText(s string) *Date {
	if len(s) != 10 || s[4] != '-' || s[7] != '-' || !digits(s[0:4]) || !digits(s[5:7]) || !digits(s[8:10]) {
		return nil
	}
	d, err := ParseDate(s)
	if err != nil {
		return nil
	}
	return &d
}
