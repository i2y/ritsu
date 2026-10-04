package {{PKG}}

// What follows is the same in every book chobo writes for PostgreSQL.
//
// A call is one SQL function, which does it in one transaction. The function
// answers a refusal as a row; an error is a mistake in the call, or the database's own.

import (
	"context"
	"errors"
	"fmt"
	"math/rand/v2"
	"time"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
)

// Querier is what a client needs of a PostgreSQL connection: pgx's *pgx.Conn, *pgxpool.Pool and
// pgx.Tx have it.
type Querier interface {
	QueryRow(ctx context.Context, sql string, args ...any) pgx.Row
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

// attempts is how many times a call is made when PostgreSQL answers a serialization failure or a deadlock.
const attempts = 10

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

type pgRuntime struct {
	q      Querier
	tenant string
}

func sqlstate(err error) string {
	var e *pgconn.PgError
	if errors.As(err, &e) {
		return e.Code
	}
	return ""
}

// row scans one row of sql into dest. A serialization failure (40001) or a deadlock (40P01) is
// tried again with the same arguments: the key keeps the call from moving twice. Inside a
// transaction of the caller's, the second try finds the transaction aborted (25P02), and the first
// error is returned.
func (r *pgRuntime) row(ctx context.Context, sql string, args []any, dest ...any) error {
	var first error
	for attempt := 1; ; attempt++ {
		err := r.q.QueryRow(ctx, sql, args...).Scan(dest...)
		if err == nil {
			return nil
		}
		code := sqlstate(err)
		if first != nil && code == "25P02" {
			return first
		}
		if (code == "40001" || code == "40P01") && attempt < attempts {
			if first == nil {
				first = err
			}
			if err := backoff(ctx, attempt); err != nil {
				return err
			}
			continue
		}
		return err
	}
}

// amounts checks the amounts a call is given: each is from 0 to 2^63 - 1.
func amounts(args []any) error {
	for _, x := range args {
		if a, ok := x.(int64); ok && a < 0 {
			return fmt.Errorf("chobo: an amount is %d, and an amount is from 0 to 2^63 - 1", a)
		}
	}
	return nil
}

func (r *pgRuntime) call(ctx context.Context, sql string, args []any) (Result, error) {
	if err := amounts(args); err != nil {
		return Result{}, err
	}
	var result string
	var reason *string
	if err := r.row(ctx, sql, args, &result, &reason); err != nil {
		return Result{}, err
	}
	switch result {
	case "done", "done_before":
		return Result{Outcome: result}, nil
	case "refused":
		if reason != nil {
			return Result{Outcome: result, Reason: *reason}, nil
		}
	}
	return Result{}, fmt.Errorf("chobo: the function answered %q", result)
}

func (r *pgRuntime) status(ctx context.Context, sql string, args []any) (HoldState, error) {
	var state *string
	if err := r.row(ctx, sql, args, &state); err != nil {
		return "", err
	}
	if state == nil {
		return "", nil
	}
	return HoldState(*state), nil
}

func (r *pgRuntime) balance(ctx context.Context, sql string, args []any) (Balance, error) {
	var b Balance
	err := r.row(ctx, sql, args, &b.Posted, &b.HeldIn, &b.HeldOut)
	return b, err
}

// expire gives back what the holds past their expiry hold; it answers how many holds it ended.
func (r *pgRuntime) expire(ctx context.Context, sql string) (int, error) {
	var n int
	err := r.row(ctx, sql, nil, &n)
	return n, err
}
