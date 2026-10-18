// @HEAD@

// Package kaname is what every module of kaname in this package shares (kaname's DESIGN 10.1): exact
// fractions, the arithmetic held to int64, rulec's five ways of rounding, dates, how a value is
// written in a message, and the error the modules return. It imports the standard library alone.
//
// A function of a module that can fail returns its *KanameError as an error; inside the module, a
// place that fails panics with it, and the function the host called recovers it (Recover).
package kaname

import (
	"fmt"
	"math"
	"math/big"
	"strconv"
	"strings"
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
}

// Running is an element of what running_totals answers (kaname's DESIGN 3.6).
type Running[T, W any] struct {
	Item   T
	Before W
	Upto   W
	Total  W
}

// Itoa writes a count.
func Itoa(n int) string { return strconv.Itoa(n) }

// Elem is the path of the i-th element of what: what[i].
func Elem(what string, i int) string { return what + "[" + strconv.Itoa(i) + "]" }

// BoolText writes true or false.
func BoolText(b bool) string { return strconv.FormatBool(b) }

// StartsWith is whether s begins with p.
func StartsWith(s, p string) bool { return strings.HasPrefix(s, p) }
