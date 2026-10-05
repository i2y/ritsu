// Package harness runs scenarios through the Go clients chobo writes, for tests/backends.rs. Its
// input and output are those of ../../runner.ts (see there).
//
// The test builds one binary of this package and every generated package it runs, with a main.go
// of its own that adapts each package's typed calls to Book, and calls Main.
package harness

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
	tb "github.com/tigerbeetle/tigerbeetle-go"
)

// Book is a generated package's book value, called by the names of the book.
type Book interface {
	Call(ctx context.Context, kind, op string, args, amounts map[string]any) (outcome, reason string, err error)
	Balance(ctx context.Context, account string, args map[string]any) (posted, heldIn, heldOut int64, err error)
	Status(ctx context.Context, kind string, args map[string]any) (string, error)
	Expire(ctx context.Context) error
}

// Querier is what the PostgreSQL clients take.
type Querier interface {
	QueryRow(ctx context.Context, sql string, args ...any) pgx.Row
}

// TBClient is what the TigerBeetle clients take.
type TBClient interface {
	CreateAccounts(accounts []tb.Account) ([]tb.CreateAccountResult, error)
	CreateTransfers(transfers []tb.Transfer) ([]tb.CreateTransferResult, error)
	LookupAccounts(accountIDs []tb.Uint128) ([]tb.Account, error)
	LookupTransfers(transferIDs []tb.Uint128) ([]tb.Transfer, error)
}

// Entry is how main.go makes a book value of one generated package.
type Entry struct {
	Postgres    func(q Querier, tenant string) Book
	TigerBeetle func(c TBClient, tenant string) Book
}

// Str and Amt read an argument of the scenario: a string, or an amount.
func Str(m map[string]any, k string) string {
	s, _ := m[k].(string)
	return s
}

func Amt(m map[string]any, k string) int64 {
	switch v := m[k].(type) {
	case json.Number:
		n, _ := v.Int64()
		return n
	case float64:
		return int64(v)
	}
	return 0
}

type input struct {
	Backend  string `json:"backend"`
	Postgres *struct {
		Host     string `json:"host"`
		Port     int    `json:"port"`
		Database string `json:"database"`
		User     string `json:"user"`
	} `json:"postgres"`
	TigerBeetle *struct {
		Cluster   string   `json:"cluster"`
		Addresses []string `json:"addresses"`
	} `json:"tigerbeetle"`
	Books []struct {
		Name      string              `json:"name"`
		Client    string              `json:"client"`
		Expiry    float64             `json:"expiry"`
		Keys      map[string][]string `json:"keys"`
		Scenarios []scenarioIn        `json:"scenarios"`
	} `json:"books"`
}

type scenarioIn struct {
	Tenant   string           `json:"tenant"`
	Steps    []map[string]any `json:"steps"`
	Accounts []struct {
		Account string         `json:"account"`
		Args    []string       `json:"args"`
		Params  map[string]any `json:"params"`
	} `json:"accounts"`
	Holds []struct {
		Kind string         `json:"kind"`
		Key  []string       `json:"key"`
		Args map[string]any `json:"args"`
	} `json:"holds"`
}

var accountFlags = []string{"linked", "debits_must_not_exceed_credits", "credits_must_not_exceed_debits", "history", "imported", "closed"}
var transferFlags = []string{"linked", "pending", "post_pending_transfer", "void_pending_transfer", "balancing_debit", "balancing_credit", "closing_debit", "closing_credit", "imported"}

func flagNames(names []string, bits uint16) []string {
	out := []string{}
	for i, n := range names {
		if bits&(1<<i) != 0 {
			out = append(out, n)
		}
	}
	return out
}

func hex32(v tb.Uint128) string {
	return fmt.Sprintf("%032x", v.BigInt())
}

func accountJSON(a tb.Account) map[string]any {
	return map[string]any{"id": hex32(a.ID), "ledger": a.Ledger, "code": a.Code, "flags": flagNames(accountFlags, a.Flags), "user_data_128": hex32(a.UserData128), "user_data_64": a.UserData64, "user_data_32": a.UserData32}
}

func transferJSON(x tb.Transfer) map[string]any {
	return map[string]any{
		"id": hex32(x.ID), "debit_account_id": hex32(x.DebitAccountID), "credit_account_id": hex32(x.CreditAccountID), "amount": x.Amount.BigInt().String(),
		"pending_id": hex32(x.PendingID), "user_data_128": hex32(x.UserData128), "user_data_64": x.UserData64, "user_data_32": x.UserData32,
		"timeout": x.Timeout, "ledger": x.Ledger, "code": x.Code, "flags": flagNames(transferFlags, x.Flags),
	}
}

// recorder writes down the requests of one operation, as the client sends them.
type recorder struct {
	mu      sync.Mutex
	current *[]any
}

func (r *recorder) note(what any) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.current != nil {
		*r.current = append(*r.current, what)
	}
}

type pgConn struct {
	pool *pgxpool.Pool
	rec  *recorder
}

func param(v any) any {
	switch x := v.(type) {
	case int64:
		return fmt.Sprint(x)
	}
	return v
}

func (c pgConn) QueryRow(ctx context.Context, sql string, args ...any) pgx.Row {
	ps := make([]any, len(args))
	for i, a := range args {
		ps[i] = param(a)
	}
	c.rec.note(map[string]any{"sql": sql, "params": ps})
	return c.pool.QueryRow(ctx, sql, args...)
}

type tbConn struct {
	c   tb.Client
	rec *recorder
}

func (t tbConn) CreateAccounts(batch []tb.Account) ([]tb.CreateAccountResult, error) {
	xs := make([]any, len(batch))
	for i, a := range batch {
		xs[i] = accountJSON(a)
	}
	t.rec.note(map[string]any{"create_accounts": xs})
	return t.c.CreateAccounts(batch)
}

func (t tbConn) CreateTransfers(batch []tb.Transfer) ([]tb.CreateTransferResult, error) {
	xs := make([]any, len(batch))
	for i, x := range batch {
		xs[i] = transferJSON(x)
	}
	t.rec.note(map[string]any{"create_transfers": xs})
	return t.c.CreateTransfers(batch)
}

func (t tbConn) LookupAccounts(ids []tb.Uint128) ([]tb.Account, error) {
	xs := make([]any, len(ids))
	for i, id := range ids {
		xs[i] = hex32(id)
	}
	t.rec.note(map[string]any{"lookup_accounts": xs})
	return t.c.LookupAccounts(ids)
}

func (t tbConn) LookupTransfers(ids []tb.Uint128) ([]tb.Transfer, error) {
	xs := make([]any, len(ids))
	for i, id := range ids {
		xs[i] = hex32(id)
	}
	t.rec.note(map[string]any{"lookup_transfers": xs})
	return t.c.LookupTransfers(ids)
}

type pendingTransfer struct {
	debit   string
	amount  *big.Int
	timeout uint32
}

type runner struct {
	in       *input
	pool     *pgxpool.Pool
	clients  []tb.Client
	next     atomic.Int64
	expiring sync.Mutex
}

func (r *runner) connection(rec *recorder, e Entry, tenant string) Book {
	if r.in.Backend == "postgres" {
		return e.Postgres(pgConn{r.pool, rec}, tenant)
	}
	c := r.clients[r.next.Add(1)%int64(len(r.clients))]
	return e.TigerBeetle(tbConn{c, rec}, tenant)
}

type caller struct {
	rec  *recorder
	book Book
}

func toMap(v any) map[string]any {
	m, _ := v.(map[string]any)
	return m
}

func (r *runner) scenario(ctx context.Context, e Entry, expiry float64, keys map[string][]string, sc scenarioIn) (out map[string]any, err error) {
	var mu sync.Mutex
	sent := []any{}
	var doneHolds []time.Time
	// when the first hold since the last pass was asked for, and whether a step or a read came
	// after its expiry (runner.ts says why)
	var heldSince time.Time
	late := false
	outlived := func() {
		if !heldSince.IsZero() && time.Since(heldSince) >= time.Duration(expiry*float64(time.Second)) {
			late = true
		}
	}
	pending := map[string][]pendingTransfer{}
	debited := map[string]bool{}
	holdKey := func(kind string, args map[string]any) string {
		vals := []any{}
		for _, k := range keys[kind] {
			vals = append(vals, args[k])
		}
		b, _ := json.Marshal(vals)
		return kind + string(b)
	}
	newCaller := func() caller {
		rec := &recorder{}
		return caller{rec, r.connection(rec, e, sc.Tenant)}
	}
	main := newCaller()
	call := func(c caller, op map[string]any, step int, callerNo int) (map[string]any, error) {
		requests := []any{}
		c.rec.mu.Lock()
		c.rec.current = &requests
		c.rec.mu.Unlock()
		asked := time.Now()
		kind, opName := op["kind"].(string), op["op"].(string)
		args := toMap(op["args"])
		var amounts map[string]any
		if a, ok := op["amounts"]; ok {
			amounts = toMap(a)
		}
		outcome, reason, err := c.book.Call(ctx, kind, opName, args, amounts)
		c.rec.mu.Lock()
		c.rec.current = nil
		c.rec.mu.Unlock()
		if err != nil {
			return nil, fmt.Errorf("step %d: %s.%s: %w", step, kind, opName, err)
		}
		at := map[string]any{"step": step, "op": opName, "kind": kind, "requests": requests}
		if callerNo > 0 {
			at["caller"] = callerNo
		}
		mu.Lock()
		sent = append(sent, at)
		if outcome == "done" && opName == "hold" {
			doneHolds = append(doneHolds, time.Now())
			if heldSince.IsZero() {
				heldSince = asked
			}
			var chain []any
			for _, q := range requests {
				if xs, ok := q.(map[string]any)["create_transfers"]; ok {
					chain = xs.([]any)
				}
			}
			var ps []pendingTransfer
			for _, x := range chain {
				t := x.(map[string]any)
				flags := t["flags"].([]string)
				isPending := false
				for _, f := range flags {
					if f == "pending" {
						isPending = true
					}
				}
				if isPending && t["user_data_32"].(uint32) != 2 {
					amount, _ := new(big.Int).SetString(t["amount"].(string), 10)
					ps = append(ps, pendingTransfer{t["debit_account_id"].(string), amount, t["timeout"].(uint32)})
					debited[t["debit_account_id"].(string)] = true
				}
			}
			pending[holdKey(kind, args)] = ps
		}
		if outcome == "done" && (opName == "post" || opName == "void") {
			delete(pending, holdKey(kind, args))
		}
		mu.Unlock()
		res := map[string]any{"op": opName, "kind": kind, "result": outcome}
		if outcome == "refused" {
			res["reason"] = reason
		}
		return res, nil
	}
	var expireAll func() error
	pass := func() error {
		outlived()
		err := expireAll()
		heldSince = time.Time{}
		return err
	}
	expireAll = func() error {
		var until time.Time
		for _, t := range doneHolds {
			if t.After(until) {
				until = t
			}
		}
		until = until.Add(time.Duration(expiry*float64(time.Second)) + 300*time.Millisecond)
		time.Sleep(time.Until(until))
		if r.in.Backend == "postgres" {
			// one expire() at a time, as one job would call it: an expire() skips the holds another
			// is giving back, and returns before that one commits
			r.expiring.Lock()
			defer r.expiring.Unlock()
			return main.book.Expire(ctx)
		}
		for k, ps := range pending {
			var left []pendingTransfer
			for _, p := range ps {
				if p.timeout == 0 {
					left = append(left, p)
				}
			}
			if len(left) == 0 {
				delete(pending, k)
			} else {
				pending[k] = left
			}
		}
		want := map[string]*big.Int{}
		var ids []tb.Uint128
		for d := range debited {
			want[d] = new(big.Int)
			id, _ := tb.HexStringToUint128(d)
			ids = append(ids, id)
		}
		for _, ps := range pending {
			for _, p := range ps {
				want[p.debit].Add(want[p.debit], p.amount)
			}
		}
		for tries := 0; ; tries++ {
			ok := true
			if len(ids) > 0 {
				found, err := r.clients[0].LookupAccounts(ids)
				if err != nil {
					return err
				}
				for _, a := range found {
					if a.DebitsPending.BigInt().Cmp(want[hex32(a.ID)]) != 0 {
						ok = false
					}
				}
			}
			if ok {
				return nil
			}
			if tries > 200 {
				return errors.New("the holds past their expiry were not given back within 10 seconds")
			}
			time.Sleep(50 * time.Millisecond)
		}
	}
	steps := []any{}
	for i, step := range sc.Steps {
		switch step["op"] {
		case "pass":
			if err := pass(); err != nil {
				return nil, err
			}
			steps = append(steps, map[string]any{"op": "pass"})
		case "together":
			callerOps := step["callers"].([]any)
			callers := make([]caller, len(callerOps))
			for ci := range callers {
				callers[ci] = newCaller()
			}
			outs := make([][]any, len(callerOps))
			errs := make([]error, len(callerOps))
			var wg sync.WaitGroup
			for ci, ops := range callerOps {
				wg.Add(1)
				go func(ci int, ops []any) {
					defer wg.Done()
					rs := []any{}
					for _, op := range ops {
						res, err := call(callers[ci], op.(map[string]any), i+1, ci+1)
						if err != nil {
							errs[ci] = err
							return
						}
						rs = append(rs, res)
					}
					outs[ci] = rs
				}(ci, ops.([]any))
			}
			wg.Wait()
			if err := errors.Join(errs...); err != nil {
				return nil, err
			}
			steps = append(steps, map[string]any{"op": "together", "callers": outs})
		default:
			res, err := call(main, step, i+1, 0)
			if err != nil {
				return nil, err
			}
			steps = append(steps, res)
		}
	}
	accounts := []any{}
	for _, a := range sc.Accounts {
		posted, heldIn, heldOut, err := main.book.Balance(ctx, a.Account, a.Params)
		if err != nil {
			return nil, err
		}
		accounts = append(accounts, map[string]any{"account": a.Account, "args": a.Args, "posted": posted, "held_in": heldIn, "held_out": heldOut})
	}
	holds := []any{}
	for _, h := range sc.Holds {
		state, err := main.book.Status(ctx, h.Kind, h.Args)
		if err != nil {
			return nil, err
		}
		if state != "" {
			holds = append(holds, map[string]any{"kind": h.Kind, "key": h.Key, "state": state})
		}
	}
	outlived()
	return map[string]any{"tenant": sc.Tenant, "result": map[string]any{"steps": steps, "accounts": accounts, "holds": holds}, "sent": sent, "late": late, "error": nil}, nil
}

// Main runs the scenarios the input (os.Args[1]) names through the packages of entries.
func Main(entries map[string]Entry) {
	if err := run(entries); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(entries map[string]Entry) error {
	f, err := os.Open(os.Args[1])
	if err != nil {
		return err
	}
	dec := json.NewDecoder(f)
	dec.UseNumber()
	var in input
	if err := dec.Decode(&in); err != nil {
		return err
	}
	ctx := context.Background()
	r := &runner{in: &in}
	if in.Backend == "postgres" {
		c := in.Postgres
		cfg, err := pgxpool.ParseConfig(fmt.Sprintf("host=%s port=%d dbname=%s user=%s pool_max_conns=20", c.Host, c.Port, c.Database, c.User))
		if err != nil {
			return err
		}
		r.pool, err = pgxpool.NewWithConfig(ctx, cfg)
		if err != nil {
			return err
		}
		defer r.pool.Close()
	} else {
		c := in.TigerBeetle
		var cluster big.Int
		cluster.SetString(c.Cluster, 10)
		for range 4 {
			client, err := tb.NewClient(tb.BigIntToUint128(&cluster), c.Addresses)
			if err != nil {
				return err
			}
			defer client.Close()
			r.clients = append(r.clients, client)
		}
	}
	type bookOut struct {
		Name      string `json:"name"`
		Scenarios []any  `json:"scenarios"`
	}
	books := make([]bookOut, len(in.Books))
	var wg sync.WaitGroup
	for bi, book := range in.Books {
		e, ok := entries[book.Client]
		if !ok {
			return fmt.Errorf("the runner has no package %q", book.Client)
		}
		books[bi] = bookOut{Name: book.Name, Scenarios: make([]any, len(book.Scenarios))}
		for si, sc := range book.Scenarios {
			wg.Add(1)
			go func(bi, si int, sc scenarioIn, expiry float64, keys map[string][]string) {
				defer wg.Done()
				out, err := r.scenario(ctx, e, expiry, keys, sc)
				if err != nil {
					out = map[string]any{"tenant": sc.Tenant, "result": nil, "sent": []any{}, "error": err.Error()}
				}
				books[bi].Scenarios[si] = out
			}(bi, si, sc, book.Expiry, book.Keys)
		}
	}
	wg.Wait()
	b, err := json.Marshal(map[string]any{"books": books})
	if err != nil {
		return err
	}
	_, err = fmt.Println(strings.TrimSpace(string(b)))
	return err
}
