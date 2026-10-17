package {{PKG}}

// What follows is the same in every book chobo writes for MySQL, TiDB and Vitess.
//
// A call is one transaction of the database's. The accounts it moves that have bounds are
// locked in the order of their IDs and read, and the read says, row by row, the first move
// whose bound it would break; the writes work the new balances out themselves. The SQL is the
// book's, written by chobo. This part fills in the values (IDs as hex, amounts as digits, the
// caller's strings as the hex of their UTF-8) and picks the next statement from what the last
// one answered.

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"math/rand/v2"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/go-sql-driver/mysql"
)

// Querier is what a client needs of a database: database/sql's *sql.DB runs each call in a
// transaction of its own, on a connection of its pool; *sql.Conn does it on that connection; a
// *sql.Tx of the caller's runs the calls in the caller's transaction, which the caller commits.
type Querier interface {
	QueryContext(ctx context.Context, query string, args ...any) (*sql.Rows, error)
}

// pool is a Querier that hands out a connection for a call: *sql.DB.
type pool interface {
	Conn(ctx context.Context) (*sql.Conn, error)
}

// Result is what a call answers: Outcome is "done", "done_before" or "refused", and Reason says
// why it was refused. A refusal is an answer, not an error: nothing has changed.
type Result struct {
	Outcome string
	Reason  string
}

// HoldState is where a hold is; "" when there is no such hold.
type HoldState string

const (
	Held    HoldState = "held"
	Posted  HoldState = "posted"
	Voided  HoldState = "voided"
	Expired HoldState = "expired"
)

// Balance is an account's balance: what is posted, what holds put in, and what holds take out.
type Balance struct {
	Posted  int64
	HeldIn  int64
	HeldOut int64
}

// Error is a book the database does not hold as written: Code is "CB001" when an account's row
// was made with other bounds than the book's.
type Error struct {
	Code    string
	Message string
}

func (e *Error) Error() string { return e.Message }

// The book's definitions, written by chobo as positional literals.

type argDef struct {
	param   int // -1 for a literal
	literal string
}

type refDef struct {
	kind string
	args []argDef
}

type amountDef struct {
	param   int // -1 for a literal
	literal int64
}

type moveDef struct {
	amount amountDef
	from   refDef
	to     refDef
}

type paramDef struct {
	name   string
	amount bool
}

type checkDef struct {
	n      int
	reason string
	sql    string
}

// transferSQL is what a transfer kind's calls send; lock and apply are "" when no move touches
// an account with a bound. The end of a hold writes the hold's row, then the accounts' rows.
type transferSQL struct {
	key, lock              string
	checks                 []checkDef
	refuse, entries, apply string
	postApply              []string
	postEntries            string
	voidApply              []string
	voidEntries            string
	expireApply            []string
	expireEntries          string
}

type transferDef struct {
	name       string
	params     []paramDef
	key        []int
	pending    bool
	definition string
	moves      []moveDef
	sql        transferSQL
}

type accountDef struct {
	name    string
	params  []string
	bounded bool
	lower   *int64
	upper   *int64
	row     string
}

type bookSQL struct {
	begin, readAccounts, makeAccounts, readCall, holdLock, status, balance string
	due, foldList, foldLock, foldRead, foldApply, foldMark                 string
}

type bookDef struct {
	name      string
	accounts  map[string]accountDef
	transfers map[string]transferDef
	sql       bookSQL
}

func newBook(name string, accounts []accountDef, transfers []transferDef, s bookSQL) bookDef {
	b := bookDef{name: name, accounts: map[string]accountDef{}, transfers: map[string]transferDef{}, sql: s}
	for _, a := range accounts {
		b.accounts[a.name] = a
	}
	for _, t := range transfers {
		b.transfers[t.name] = t
	}
	return b
}

func i64(v int64) *int64 { return &v }

// attempts is how many times a call of its own transaction is made when the database gives up on
// it: about 1.5 seconds of waits between them (DESIGN 4.5).
const attempts = 30

// again are the errors after which such a call is made again with the same arguments (the key
// keeps it from moving twice): a deadlock, a lock not got in time, and TiDB's write conflicts.
var again = map[uint16]bool{1213: true, 1205: true, 9007: true, 8022: true, 8028: true}

const duplicate = 1062

var place = regexp.MustCompile(`\{([A-Z0-9_]+)\}`)

func errno(err error) uint16 {
	var e *mysql.MySQLError
	if errors.As(err, &e) {
		return e.Number
	}
	return 0
}

// backoff waits before the next try: up to 1, 2, 4 … 128 ms, at random, so that the calls that
// failed together do not meet again.
func backoff(ctx context.Context, attempt int) error {
	t := time.NewTimer(time.Duration(rand.Int64N(int64(time.Millisecond) << min(attempt-1, 7))))
	defer t.Stop()
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-t.C:
		return nil
	}
}

var idLimit = new(big.Int).Sub(new(big.Int).Lsh(big.NewInt(1), 128), big.NewInt(1))

// chID is an ID: the first 16 bytes of a SHA-256 over the parts, each with its UTF-8 length in front.
func chID(parts ...string) string {
	h := sha256.New()
	var n [4]byte
	for _, p := range append([]string{"chobo/1"}, parts...) {
		binary.BigEndian.PutUint32(n[:], uint32(len(p)))
		h.Write(n[:])
		h.Write([]byte(p))
	}
	v := new(big.Int).SetBytes(h.Sum(nil)[:16])
	switch {
	case v.Sign() == 0:
		v.SetInt64(1)
	case v.Cmp(idLimit) == 0:
		v.Sub(v, big.NewInt(1))
	}
	return fmt.Sprintf("%032x", v)
}

// idLit is an ID, as 32 hex digits, as SQL.
func idLit(h string) string { return "X'" + h + "'" }

// strLit is a string of the caller's as SQL: the hex of its UTF-8, read as utf8mb4, so that
// nothing in it is ever read as SQL.
func strLit(s string) string { return "_utf8mb4 X'" + hex.EncodeToString([]byte(s)) + "'" }

// idList is IDs as an SQL list: each once, in the order of their hex.
func idList(ids []string) string {
	seen := map[string]bool{}
	var u []string
	for _, id := range ids {
		if !seen[id] {
			seen[id] = true
			u = append(u, id)
		}
	}
	sort.Strings(u)
	out := make([]string, len(u))
	for i, id := range u {
		out[i] = idLit(id)
	}
	return strings.Join(out, ", ")
}

// jsonString is a string as JavaScript's JSON.stringify writes it (PLAN 0.3).
func jsonString(s string) string {
	var b strings.Builder
	b.WriteByte('"')
	for _, c := range s {
		switch {
		case c == '"':
			b.WriteString(`\"`)
		case c == '\\':
			b.WriteString(`\\`)
		case c == '\b':
			b.WriteString(`\b`)
		case c == '\f':
			b.WriteString(`\f`)
		case c == '\n':
			b.WriteString(`\n`)
		case c == '\r':
			b.WriteString(`\r`)
		case c == '\t':
			b.WriteString(`\t`)
		case c < 0x20:
			fmt.Fprintf(&b, `\u%04x`, c)
		default:
			b.WriteRune(c)
		}
	}
	b.WriteByte('"')
	return b.String()
}

func jsonStrings(vs []string) string {
	out := make([]string, len(vs))
	for i, v := range vs {
		out[i] = jsonString(v)
	}
	return "[" + strings.Join(out, ",") + "]"
}

// fillAll puts the values in each of the statements.
func fillAll(stmts []string, values map[string]string) []string {
	out := make([]string, len(stmts))
	for i, s := range stmts {
		out[i] = fill(s, values)
	}
	return out
}

// fill puts the values in a statement: each {NAME} by the value of NAME, in one pass.
func fill(template string, values map[string]string) string {
	return place.ReplaceAllStringFunc(template, func(m string) string {
		if v, ok := values[m[1:len(m)-1]]; ok {
			return v
		}
		return m
	})
}

func merge(ms ...map[string]string) map[string]string {
	out := map[string]string{}
	for _, m := range ms {
		for k, v := range m {
			out[k] = v
		}
	}
	return out
}

// content is what a do or hold is called with, as the key compares it: the arguments in the
// order of the parameters.
func content(t transferDef, v []any) string {
	parts := make([]string, len(t.params))
	for i, p := range t.params {
		if p.amount {
			parts[i] = jsonString(p.name) + ":" + strconv.FormatInt(v[i].(int64), 10)
		} else {
			parts[i] = jsonString(p.name) + ":" + jsonString(v[i].(string))
		}
	}
	return "{" + strings.Join(parts, ",") + "}"
}

func amountsOK(v []any) error {
	for _, x := range v {
		if a, ok := x.(int64); ok && a < 0 {
			return fmt.Errorf("chobo: an amount is %d, and an amount is from 0 to 2^63 - 1", a)
		}
	}
	return nil
}

type row []sql.NullString

// tx is the statements of one call: in a transaction of the call's own (own), or in the caller's.
type tx struct {
	q     Querier
	own   bool
	batch bool
	begin string
}

func readSet(rows *sql.Rows) ([]row, error) {
	cols, err := rows.Columns()
	if err != nil {
		return nil, err
	}
	var out []row
	for rows.Next() {
		r := make(row, len(cols))
		ptrs := make([]any, len(cols))
		for i := range r {
			ptrs[i] = &r[i]
		}
		if err := rows.Scan(ptrs...); err != nil {
			return nil, err
		}
		out = append(out, r)
	}
	return out, rows.Err()
}

// exec runs stmts in order and answers the rows of each: one request (batch), or one request a
// statement, stopping at the first error.
func (x *tx) exec(ctx context.Context, stmts []string) ([][]row, error) {
	if len(stmts) == 0 {
		return nil, nil
	}
	if x.batch && len(stmts) > 1 {
		// go-sql-driver shows only the results that have columns, the selects', and passes over the
		// others: each select takes the next of them
		rows, err := x.q.QueryContext(ctx, strings.Join(stmts, "; "))
		if err != nil {
			return nil, err
		}
		defer rows.Close()
		out := make([][]row, len(stmts))
		first := true
		for i, s := range stmts {
			if !strings.HasPrefix(s, "select ") {
				continue
			}
			if !first && !rows.NextResultSet() {
				if err := rows.Err(); err != nil {
					return nil, err
				}
				return nil, fmt.Errorf("chobo: statement %d of %d did not answer", i+1, len(stmts))
			}
			first = false
			set, err := readSet(rows)
			if err != nil {
				return nil, err
			}
			out[i] = set
		}
		// what the statements after the last select answered: an error, if one of them failed
		for rows.NextResultSet() {
		}
		if err := rows.Err(); err != nil {
			return nil, err
		}
		return out, nil
	}
	out := make([][]row, 0, len(stmts))
	for _, s := range stmts {
		rows, err := x.q.QueryContext(ctx, s)
		if err != nil {
			return nil, err
		}
		set, err := readSet(rows)
		rows.Close()
		if err != nil {
			return nil, err
		}
		out = append(out, set)
	}
	return out, nil
}

// plain runs statements outside the transaction: when the call has its own, before it starts.
func (x *tx) plain(ctx context.Context, stmts ...string) ([][]row, error) { return x.exec(ctx, stmts) }

// run runs statements of the transaction, which starts with the first of them when the call has
// its own, and ends after the last with commit when commit is set.
func (x *tx) run(ctx context.Context, begin, commit bool, stmts ...string) ([][]row, error) {
	all := []string{}
	if begin && x.own {
		all = append(all, x.begin)
	}
	all = append(all, stmts...)
	if commit && x.own {
		all = append(all, "commit")
	}
	rows, err := x.exec(ctx, all)
	if err != nil {
		return nil, err
	}
	start := 0
	if begin && x.own {
		start = 1
	}
	return rows[start : start+len(stmts)], nil
}

// quit ends a call that writes nothing more: its own transaction rolled back.
func (x *tx) quit(ctx context.Context) error {
	if x.own {
		_, err := x.exec(ctx, []string{"rollback"})
		return err
	}
	return nil
}

type placed struct {
	id   string
	kind string
	args []string
}

type myRuntime struct {
	book   *bookDef
	q      Querier
	tenant string
	batch  bool
	base   map[string]string
	mu     sync.Mutex
	// the accounts this value has made sure of in a transaction of a call's own, where their rows
	// were committed at once: rows are never removed, nor their bounds changed
	sure map[string]bool
}

func newMyRuntime(book *bookDef, q Querier, tenant string, batch bool) *myRuntime {
	return &myRuntime{
		book: book, q: q, tenant: tenant, batch: batch, sure: map[string]bool{},
		base: map[string]string{"TENANT": idLit(chID("tenant", book.name, tenant)), "TENANT_NAME": strLit(tenant)},
	}
}

// do runs fn: on a connection of a pool, in a transaction of the call's own, made again with the
// same arguments when the database gives up on it; on a connection, the same; in the caller's
// transaction otherwise.
func (r *myRuntime) do(ctx context.Context, fn func(x *tx) error) error {
	var conn func() (*sql.Conn, func(), error)
	switch q := r.q.(type) {
	case pool:
		conn = func() (*sql.Conn, func(), error) {
			c, err := q.Conn(ctx)
			if err != nil {
				return nil, nil, err
			}
			return c, func() { c.Close() }, nil
		}
	case *sql.Conn:
		conn = func() (*sql.Conn, func(), error) { return q, func() {}, nil }
	default:
		return fn(&tx{q: r.q, own: false, batch: r.batch, begin: r.book.sql.begin})
	}
	for attempt := 1; ; attempt++ {
		c, done, err := conn()
		if err != nil {
			return err
		}
		x := &tx{q: c, own: true, batch: r.batch, begin: r.book.sql.begin}
		err = fn(x)
		if err != nil {
			x.exec(context.WithoutCancel(ctx), []string{"rollback"})
		}
		done()
		if err != nil && again[errno(err)] && attempt < attempts {
			if err := backoff(ctx, attempt); err != nil {
				return err
			}
			continue
		}
		return err
	}
}

// read runs one statement outside any transaction of the call's.
func (r *myRuntime) read(ctx context.Context, stmt string) ([]row, error) {
	var out []row
	err := r.do(ctx, func(x *tx) error {
		rows, err := x.plain(ctx, stmt)
		if err == nil {
			out = rows[0]
		}
		return err
	})
	return out, err
}

func (r *myRuntime) account(ref refDef, v []any) placed {
	args := make([]string, len(ref.args))
	for i, a := range ref.args {
		if a.param >= 0 {
			args[i] = fmt.Sprint(v[a.param])
		} else {
			args[i] = a.literal
		}
	}
	return placed{chID(append([]string{"account", r.book.name, r.tenant, ref.kind}, args...)...), ref.kind, args}
}

func nullInt(s sql.NullString) (*int64, error) {
	if !s.Valid {
		return nil, nil
	}
	v, err := strconv.ParseInt(s.String, 10, 64)
	return &v, err
}

func sameBound(a, b *int64) bool {
	return (a == nil && b == nil) || (a != nil && b != nil && *a == *b)
}

// makeSure makes sure every account is there, with the book's bounds, before a call locks the
// ones with bounds: it reads them, and makes the missing ones. In the caller's transaction, a row
// made goes with the caller's rollback, so every call there reads them all, and what it makes is
// not taken as sure.
func (r *myRuntime) makeSure(ctx context.Context, x *tx, accounts []placed) error {
	r.mu.Lock()
	want := map[string]placed{}
	for _, a := range accounts {
		if !x.own || !r.sure[a.id] {
			want[a.id] = a
		}
	}
	r.mu.Unlock()
	if len(want) == 0 {
		return nil
	}
	ids := make([]string, 0, len(want))
	for id := range want {
		ids = append(ids, id)
	}
	found, err := x.plain(ctx, fill(r.book.sql.readAccounts, merge(r.base, map[string]string{"IDS": idList(ids)})))
	if err != nil {
		return err
	}
	there := map[string]bool{}
	for _, f := range found[0] {
		a, ok := want[strings.ToLower(f[0].String)]
		if !ok {
			continue
		}
		lower, err := nullInt(f[1])
		if err != nil {
			return err
		}
		upper, err := nullInt(f[2])
		if err != nil {
			return err
		}
		d := r.book.accounts[a.kind]
		if !sameBound(lower, d.lower) || !sameBound(upper, d.upper) {
			return &Error{"CB001", fmt.Sprintf("chobo: the account %s(%s) of %s has other bounds in the database than in the book", a.kind, strings.Join(a.args, ", "), r.book.name)}
		}
		there[a.id] = true
	}
	var missing []placed
	for _, a := range want {
		if !there[a.id] {
			missing = append(missing, a)
		}
	}
	sort.Slice(missing, func(i, j int) bool { return missing[i].id < missing[j].id })
	if len(missing) > 0 {
		rows := make([]string, len(missing))
		for i, a := range missing {
			rows[i] = fill(r.book.accounts[a.kind].row, merge(r.base, map[string]string{"ID": idLit(a.id), "ARGS": strLit(jsonStrings(a.args))}))
		}
		if _, err := x.plain(ctx, fill(r.book.sql.makeAccounts, map[string]string{"ROWS": strings.Join(rows, ", ")})); err != nil {
			return err
		}
	}
	if x.own {
		r.mu.Lock()
		for id := range want {
			r.sure[id] = true
		}
		r.mu.Unlock()
	}
	return nil
}

func refused(reason string) Result { return Result{Outcome: "refused", Reason: reason} }

// move is do and hold: v has every argument, in the order of the parameters.
func (r *myRuntime) move(ctx context.Context, kind, op string, v []any) (Result, error) {
	if err := amountsOK(v); err != nil {
		return Result{}, err
	}
	t := r.book.transfers[kind]
	places := make([][2]placed, len(t.moves))
	for i, m := range t.moves {
		places[i] = [2]placed{r.account(m.from, v), r.account(m.to, v)}
		// a move from an account to itself: refused before anything is sent, and the key is not used
		if places[i][0].id == places[i][1].id {
			return refused("same_account"), nil
		}
	}
	key := make([]string, len(t.key))
	for i, k := range t.key {
		key[i] = v[k].(string)
	}
	was := content(t, v)
	vals := merge(r.base, map[string]string{
		"CALL":    idLit(chID(append(append([]string{"transfer", r.book.name, r.tenant, kind, op}, key...), "0")...)),
		"KEY":     strLit(jsonStrings(key)),
		"CONTENT": strLit(was),
	})
	var bounded []string
	for i, m := range t.moves {
		k := strconv.Itoa(i + 1)
		f, to := places[i][0], places[i][1]
		vals["FROM"+k], vals["TO"+k] = idLit(f.id), idLit(to.id)
		vals["FROMX"+k], vals["TOX"+k] = f.id, to.id
		if m.amount.param >= 0 {
			vals["AMOUNT"+k] = strconv.FormatInt(v[m.amount.param].(int64), 10)
		} else {
			vals["AMOUNT"+k] = strconv.FormatInt(m.amount.literal, 10)
		}
		if r.book.accounts[f.kind].bounded {
			bounded = append(bounded, f.id)
		}
		if r.book.accounts[to.kind].bounded {
			bounded = append(bounded, to.id)
		}
	}
	vals["BOUNDED"] = idList(bounded)
	locked := map[string]bool{}
	for _, id := range bounded {
		locked[id] = true
	}
	all := make([]placed, 0, 2*len(places))
	for _, p := range places {
		all = append(all, p[0], p[1])
	}
	var res Result
	err := r.do(ctx, func(x *tx) error {
		if err := r.makeSure(ctx, x, all); err != nil {
			return err
		}
		first := []string{fill(t.sql.key, vals)}
		if t.sql.lock != "" {
			first = append(first, fill(t.sql.lock, vals))
		}
		rows, err := x.run(ctx, true, false, first...)
		if err != nil {
			if errno(err) != duplicate {
				return err
			}
			// the key was used: by this call made before, or by another
			used, err := x.run(ctx, false, false, fill(r.book.sql.readCall, vals))
			if err != nil {
				return err
			}
			if err := x.quit(ctx); err != nil {
				return err
			}
			if len(used[0]) == 0 {
				return fmt.Errorf("chobo: the key of %s.%s is used, and its call is not there", kind, op)
			}
			u := used[0][0]
			switch {
			case u[0].String == "refused":
				res = refused("already_refused")
			case u[2].String == was && u[3].String == t.definition:
				res = Result{Outcome: "done_before"}
			default:
				res = refused("key_conflict")
			}
			return nil
		}
		if t.sql.lock != "" {
			lockRows := rows[1]
			if len(lockRows) != len(locked) {
				// an account this value made sure of is not there: its row was taken away outside
				// the clients; make it again, and lock again
				r.mu.Lock()
				for id := range locked {
					delete(r.sure, id)
				}
				r.mu.Unlock()
				if err := r.makeSure(ctx, x, all); err != nil {
					return err
				}
				relocked, err := x.run(ctx, false, false, fill(t.sql.lock, vals))
				if err != nil {
					return err
				}
				if lockRows = relocked[0]; len(lockRows) != len(locked) {
					return fmt.Errorf("chobo: %d of the %d accounts of %s.%s with bounds are in the database", len(lockRows), len(locked), kind, op)
				}
			}
			first := 0
			for _, lr := range lockRows {
				n, err := strconv.Atoi(lr[1].String)
				if err != nil {
					return err
				}
				if n > 0 && (first == 0 || n < first) {
					first = n
				}
			}
			if first > 0 {
				for _, c := range t.sql.checks {
					if c.n == first {
						if _, err := x.run(ctx, false, true, fill(t.sql.refuse, merge(vals, map[string]string{"REASON": c.sql}))); err != nil {
							return err
						}
						res = refused(c.reason)
						return nil
					}
				}
				return fmt.Errorf("chobo: no check %d of %s.%s", first, kind, op)
			}
		}
		writes := []string{fill(t.sql.entries, vals)}
		if t.sql.apply != "" {
			writes = append(writes, fill(t.sql.apply, vals))
		}
		if _, err := x.run(ctx, false, true, writes...); err != nil {
			return err
		}
		res = Result{Outcome: "done"}
		return nil
	})
	return res, err
}

type heldMove struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Amount string `json:"amount"`
}

// ending is the values of the end of a hold, from its moves as the database has them.
func (r *myRuntime) ending(t transferDef, moves []heldMove, posted []int64) map[string]string {
	vals := map[string]string{}
	var bounded []string
	for i, m := range moves {
		k := strconv.Itoa(i + 1)
		vals["FROM"+k], vals["TO"+k] = idLit(m.From), idLit(m.To)
		vals["HELD"+k] = m.Amount
		p := int64(0)
		if i < len(posted) {
			p = posted[i]
		}
		vals["POSTED"+k] = strconv.FormatInt(p, 10)
		if r.book.accounts[t.moves[i].from.kind].bounded {
			bounded = append(bounded, m.From)
		}
		if r.book.accounts[t.moves[i].to.kind].bounded {
			bounded = append(bounded, m.To)
		}
	}
	vals["BOUNDED"] = idList(bounded)
	return vals
}

// end is post and void: key in the order of the key; amounts in the order of the amounts, or nil
// for all of it.
func (r *myRuntime) end(ctx context.Context, kind, op string, key []string, amounts []int64) (Result, error) {
	for _, a := range amounts {
		if a < 0 {
			return Result{}, fmt.Errorf("chobo: an amount is %d, and an amount is from 0 to 2^63 - 1", a)
		}
	}
	t := r.book.transfers[kind]
	hold := merge(r.base, map[string]string{"HOLD": idLit(chID(append(append([]string{"transfer", r.book.name, r.tenant, kind, "hold"}, key...), "0")...))})
	var res Result
	err := r.do(ctx, func(x *tx) error {
		found, err := x.run(ctx, true, false, fill(r.book.sql.holdLock, hold))
		if err != nil {
			return err
		}
		answer := func(a Result) error {
			res = a
			return x.quit(ctx)
		}
		if len(found[0]) == 0 || found[0][0][0].String == "refused" {
			return answer(refused("no_such_hold"))
		}
		h := found[0][0]
		state, postedJSON, ended, past := h[1].String, h[3].String, h[4].String, h[5].String
		var moves []heldMove
		if err := json.Unmarshal([]byte(h[2].String), &moves); err != nil {
			return err
		}
		holding := make([]int64, len(moves))
		for i, m := range moves {
			if holding[i], err = strconv.ParseInt(m.Amount, 10, 64); err != nil {
				return err
			}
		}
		posted := make([]int64, len(moves))
		if op == "post" {
			if amounts == nil {
				copy(posted, holding)
			} else {
				var amountParams []int
				for i, p := range t.params {
					if p.amount {
						amountParams = append(amountParams, i)
					}
				}
				for i, m := range t.moves {
					if m.amount.param >= 0 {
						for j, pi := range amountParams {
							if pi == m.amount.param {
								posted[i] = amounts[j]
							}
						}
					} else {
						posted[i] = m.amount.literal
					}
				}
			}
		}
		ps := make([]string, len(posted))
		for i, p := range posted {
			ps[i] = `"` + strconv.FormatInt(p, 10) + `"`
		}
		postedText := "[" + strings.Join(ps, ",") + "]"
		switch {
		case state == "posted" && op == "void":
			return answer(refused("already_posted"))
		case state == "posted":
			if h[3].Valid && postedJSON == postedText && h[4].Valid && ended == t.definition {
				return answer(Result{Outcome: "done_before"})
			}
			return answer(refused("key_conflict"))
		case state == "voided" && op == "post":
			return answer(refused("already_voided"))
		case state == "voided":
			if h[4].Valid && ended == t.definition {
				return answer(Result{Outcome: "done_before"})
			}
			return answer(refused("key_conflict"))
		case state == "expired" || past == "1":
			return answer(refused("expired"))
		}
		if op == "post" {
			for i := range posted {
				if posted[i] > holding[i] {
					return answer(refused("over_hold"))
				}
			}
		}
		vals := merge(hold, r.ending(t, moves, posted), map[string]string{
			"CALL": idLit(chID(append(append([]string{"transfer", r.book.name, r.tenant, kind, op}, key...), "0")...)),
			"KEY":  strLit(jsonStrings(key)),
		})
		apply, entries := t.sql.voidApply, t.sql.voidEntries
		if op == "post" {
			apply, entries = t.sql.postApply, t.sql.postEntries
		}
		if _, err := x.run(ctx, false, true, append(fillAll(apply, vals), fill(entries, vals))...); err != nil {
			return err
		}
		res = Result{Outcome: "done"}
		return nil
	})
	return res, err
}

func (r *myRuntime) status(ctx context.Context, kind string, key []string) (HoldState, error) {
	hold := merge(r.base, map[string]string{"HOLD": idLit(chID(append(append([]string{"transfer", r.book.name, r.tenant, kind, "hold"}, key...), "0")...))})
	rows, err := r.read(ctx, fill(r.book.sql.status, hold))
	if err != nil || len(rows) == 0 || rows[0][0].String == "refused" || !rows[0][1].Valid {
		return "", err
	}
	if rows[0][1].String == "held" && rows[0][2].String == "1" {
		return Expired, nil
	}
	return HoldState(rows[0][1].String), nil
}

func (r *myRuntime) balance(ctx context.Context, kind string, args []string) (Balance, error) {
	id := chID(append([]string{"account", r.book.name, r.tenant, kind}, args...)...)
	rows, err := r.read(ctx, fill(r.book.sql.balance, merge(r.base, map[string]string{"ID": idLit(id)})))
	if err != nil || len(rows) == 0 {
		return Balance{}, err
	}
	var b Balance
	for i, p := range []*int64{&b.Posted, &b.HeldIn, &b.HeldOut} {
		v, err := strconv.ParseInt(rows[0][i].String, 10, 64)
		if err != nil {
			return Balance{}, fmt.Errorf("chobo: the balance of %s(%s) is past a 64-bit integer", kind, strings.Join(args, ", "))
		}
		*p = v
	}
	return b, nil
}

// expire gives back what the holds past their expiry hold, each in a transaction of its own, and
// adds the entries of the accounts without bounds to their rows; it answers how many holds it ended.
func (r *myRuntime) expire(ctx context.Context) (int, error) {
	due, err := r.read(ctx, r.book.sql.due)
	if err != nil {
		return 0, err
	}
	ended := 0
	for _, d := range due {
		t, ok := r.book.transfers[d[2].String]
		if !ok {
			continue
		}
		id := strings.ToLower(d[1].String)
		hold := map[string]string{"TENANT": idLit(strings.ToLower(d[0].String)), "HOLD": idLit(id)}
		done := false
		err := r.do(ctx, func(x *tx) error {
			found, err := x.run(ctx, true, false, fill(r.book.sql.holdLock, hold))
			if err != nil {
				return err
			}
			if len(found[0]) == 0 || found[0][0][1].String != "held" || found[0][0][5].String != "1" {
				return x.quit(ctx)
			}
			var moves []heldMove
			if err := json.Unmarshal([]byte(found[0][0][2].String), &moves); err != nil {
				return err
			}
			vals := merge(hold, r.ending(t, moves, nil), map[string]string{"CALL": idLit(chID("expire", id)), "KEY": strLit(d[3].String)})
			if _, err := x.run(ctx, false, true, append(fillAll(t.sql.expireApply, vals), fill(t.sql.expireEntries, vals))...); err != nil {
				return err
			}
			done = true
			return nil
		})
		if err != nil {
			return ended, err
		}
		if done {
			ended++
		}
	}
	accounts, err := r.read(ctx, r.book.sql.foldList)
	if err != nil {
		return ended, err
	}
	for _, a := range accounts {
		at := map[string]string{"TENANT": idLit(strings.ToLower(a[0].String)), "ID": idLit(strings.ToLower(a[1].String))}
		err := r.do(ctx, func(x *tx) error {
			got, err := x.run(ctx, true, false, fill(r.book.sql.foldLock, at), fill(r.book.sql.foldRead, at))
			if err != nil {
				return err
			}
			entries := got[1]
			if len(entries) == 0 {
				return x.quit(ctx)
			}
			sums := [3]*big.Int{new(big.Int), new(big.Int), new(big.Int)}
			marks := make([]string, len(entries))
			for i, e := range entries {
				for j := range 3 {
					v, ok := new(big.Int).SetString(e[2+j].String, 10)
					if !ok {
						return fmt.Errorf("chobo: an entry is %q", e[2+j].String)
					}
					sums[j].Add(sums[j], v)
				}
				marks[i] = "(X'" + strings.ToLower(e[0].String) + "', " + e[1].String + ")"
			}
			_, err = x.run(ctx, false, true,
				fill(r.book.sql.foldApply, merge(at, map[string]string{"POSTED": sums[0].String(), "HELD_IN": sums[1].String(), "HELD_OUT": sums[2].String()})),
				fill(r.book.sql.foldMark, merge(at, map[string]string{"ROWS": strings.Join(marks, ", ")})))
			return err
		})
		if err != nil {
			return ended, err
		}
	}
	return ended, nil
}
