package {{PKG}}

// What follows is the same in every book chobo writes for TigerBeetle.
//
// A call is one chain of transfers in one request: TigerBeetle takes the chain
// whole or not at all. The bounds TigerBeetle cannot keep with an account's flags are kept by
// more transfers in the chain (a probe for a lower bound above 0, the room and floor accounts
// for an upper bound and a lower bound below 0), laid out as PLAN 0.3 says.

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"math/big"
	"strconv"
	"strings"
	"sync"

	tb "github.com/tigerbeetle/tigerbeetle-go"
)

// TigerBeetleClient is what a client needs of a TigerBeetle client: tigerbeetle-go's Client has it.
type TigerBeetleClient interface {
	CreateAccounts(accounts []tb.Account) ([]tb.CreateAccountResult, error)
	CreateTransfers(transfers []tb.Transfer) ([]tb.CreateTransferResult, error)
	LookupAccounts(accountIDs []tb.Uint128) ([]tb.Account, error)
	LookupTransfers(transferIDs []tb.Uint128) ([]tb.Transfer, error)
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
	roles  []string
}

type paramDef struct {
	name   string
	amount bool
}

type transferDef struct {
	name       string
	params     []paramDef
	key        []int
	pending    bool
	timeout    uint32
	code       uint16
	definition string
	moves      []moveDef
}

type boundDef struct {
	value  int64
	reason string
}

type accountDef struct {
	name    string
	params  []string
	unit    string
	code    uint16
	flagged bool
	lower   *boundDef
	upper   *boundDef
}

type unitDef struct {
	name     string
	ledger   uint32
	sinkCode uint16
}

type bookDef struct {
	name      string
	units     map[string]unitDef
	accounts  map[string]accountDef
	transfers map[string]transferDef
}

func newBook(name string, units []unitDef, accounts []accountDef, transfers []transferDef) bookDef {
	b := bookDef{name: name, units: map[string]unitDef{}, accounts: map[string]accountDef{}, transfers: map[string]transferDef{}}
	for _, u := range units {
		b.units[u.name] = u
	}
	for _, a := range accounts {
		b.accounts[a.name] = a
	}
	for _, t := range transfers {
		b.transfers[t.name] = t
	}
	return b
}

var roleCodes = map[string]uint32{"main": 1, "probe": 2, "probe_void": 3, "floor_out": 4, "room_back": 5, "room_in": 6, "floor_back": 7}

const opening = 8

// requestMax is the most events one request takes on every cluster (a replica started with --development).
const requestMax = 253

// id128 is an ID as chobo makes it, big-endian.
type id128 [16]byte

var idMax = id128{0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff}

// chID is an ID: the first 16 bytes of a SHA-256 over the parts, each with its UTF-8 length in front.
func chID(parts ...string) id128 {
	h := sha256.New()
	for _, p := range append([]string{"chobo/1"}, parts...) {
		var n [4]byte
		binary.BigEndian.PutUint32(n[:], uint32(len(p)))
		h.Write(n[:])
		h.Write([]byte(p))
	}
	var v id128
	copy(v[:], h.Sum(nil)[:16])
	if v == (id128{}) {
		v[15] = 1
	} else if v == idMax {
		v[15] = 0xfe
	}
	return v
}

func (v id128) hex() string { return hex.EncodeToString(v[:]) }

// u128 is the ID as TigerBeetle takes it, little-endian.
func (v id128) u128() tb.Uint128 {
	var le [16]byte
	for i := range 16 {
		le[i] = v[15-i]
	}
	return tb.BytesToUint128(le)
}

// randomID is an ID no one else has: for the transfers that read a hold and are never kept.
func randomID() (tb.Uint128, error) {
	var v id128
	if _, err := rand.Read(v[:]); err != nil {
		return tb.Uint128{}, err
	}
	if v == (id128{}) || v == idMax {
		v = id128{15: 1}
	}
	return v.u128(), nil
}

func amount(v int64) tb.Uint128 { return tb.ToUint128(uint64(v)) }

// jsonString writes a string as JavaScript's JSON.stringify does (PLAN 0.3).
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

func refused(reason string) Result { return Result{Outcome: "refused", Reason: reason} }

func tbAccount(id id128, ledger uint32, code uint16, flagged bool, userData128 id128, userData32 uint32) tb.Account {
	var flags uint16
	if flagged {
		flags = tb.AccountFlags{DebitsMustNotExceedCredits: true}.ToUint16()
	}
	return tb.Account{ID: id.u128(), UserData128: userData128.u128(), UserData32: userData32, Ledger: ledger, Code: code, Flags: flags}
}

// planned is a transfer of a chain, with the move (from 0) and the role it is there for.
type planned struct {
	transfer tb.Transfer
	role     string
	move     int
}

var (
	flagLinked  = tb.TransferFlags{Linked: true}.ToUint16()
	flagPending = tb.TransferFlags{Pending: true}.ToUint16()
	flagPost    = tb.TransferFlags{PostPendingTransfer: true}.ToUint16()
	flagVoid    = tb.TransferFlags{VoidPendingTransfer: true}.ToUint16()
)

// link links every transfer of the chain but the last to the next.
func link(chain []planned) {
	for i := 0; i+1 < len(chain); i++ {
		chain[i].transfer.Flags |= flagLinked
	}
}

type placed struct {
	id   id128
	kind string
	def  accountDef
}

type tbRuntime struct {
	book   *bookDef
	client TigerBeetleClient
	tenant string
	mu     sync.Mutex
	// the accounts and openings this value has made sure of: TigerBeetle never removes them
	made map[id128]bool
}

func newTbRuntime(book *bookDef, c TigerBeetleClient, tenant string) *tbRuntime {
	return &tbRuntime{book: book, client: c, tenant: tenant, made: map[id128]bool{}}
}

// check checks the arguments a call is given: an amount is from 0 to 2^63 - 1.
func (r *tbRuntime) check(kind, op string, t transferDef, v []any) error {
	for i, x := range v {
		if a, ok := x.(int64); ok && a < 0 {
			return fmt.Errorf("chobo: %s.%s: %s is %d, and an amount is from 0 to 2^63 - 1", kind, op, t.params[i].name, a)
		}
	}
	return nil
}

func (r *tbRuntime) place(ref refDef, v []any) placed {
	args := []string{"account", r.book.name, r.tenant, ref.kind}
	for _, a := range ref.args {
		if a.param >= 0 {
			args = append(args, text(v[a.param]))
		} else {
			args = append(args, a.literal)
		}
	}
	return placed{id: chID(args...), kind: ref.kind, def: r.book.accounts[ref.kind]}
}

func text(x any) string {
	switch y := x.(type) {
	case string:
		return y
	case int64:
		return strconv.FormatInt(y, 10)
	}
	return fmt.Sprint(x)
}

func (r *tbRuntime) kindID(kind string) id128 { return chID("kind", r.book.name, r.tenant, kind) }

func (r *tbRuntime) transferID(kind, op string, key []string, position int) id128 {
	parts := append([]string{"transfer", r.book.name, r.tenant, kind, op}, key...)
	return chID(append(parts, strconv.Itoa(position))...)
}

// content is what a do or hold is called with, as the IDs hash it: the arguments in the order of the parameters.
func content(t transferDef, v []any) string {
	fields := make([]string, len(t.params))
	for i, p := range t.params {
		if p.amount {
			fields[i] = jsonString(p.name) + ":" + text(v[i])
		} else {
			fields[i] = jsonString(p.name) + ":" + jsonString(text(v[i]))
		}
	}
	return "{" + strings.Join(fields, ",") + "}"
}

// move is do and hold: v has every argument, in the order of the parameters.
func (r *tbRuntime) move(ctx context.Context, kind, op string, v []any) (Result, error) {
	t := r.book.transfers[kind]
	if err := r.check(kind, op, t, v); err != nil {
		return Result{}, err
	}
	places := make([][2]placed, len(t.moves))
	for i, m := range t.moves {
		places[i] = [2]placed{r.place(m.from, v), r.place(m.to, v)}
	}
	// a move from an account to itself: refused before anything is sent, and the key is not used
	for _, p := range places {
		if p[0].id == p[1].id {
			return refused("same_account"), nil
		}
	}
	key := make([]string, len(t.key))
	for i, k := range t.key {
		key[i] = text(v[k])
	}
	contentID := chID("content", r.book.name, r.tenant, kind, op, t.definition, content(t, v))
	var accounts []tb.Account
	var openings []tb.Transfer
	var chain []planned
	has := func(id tb.Uint128) bool {
		for _, a := range accounts {
			if a.ID == id {
				return true
			}
		}
		return false
	}
	add := func(a tb.Account) {
		if !has(a.ID) {
			accounts = append(accounts, a)
		}
	}
	for i, m := range t.moves {
		from, to := places[i][0], places[i][1]
		unit := from.def.unit
		ledger := r.book.units[unit].ledger
		sink := chID("sink", r.book.name, r.tenant, unit)
		add(tbAccount(from.id, ledger, from.def.code, from.def.flagged, r.kindID(from.kind), 0))
		add(tbAccount(to.id, ledger, to.def.code, to.def.flagged, r.kindID(to.kind), 0))
		if len(m.roles) > 1 {
			add(tbAccount(sink, ledger, r.book.units[unit].sinkCode, false, id128{}, 3))
		}
		// the floor and room accounts the move's bounds need, each with the transfer that opens it
		for _, role := range m.roles {
			var what string
			var owner placed
			var open int64
			switch role {
			case "floor_out":
				what, owner, open = "floor", from, -from.def.lower.value
			case "room_back":
				what, owner, open = "room", from, from.def.upper.value
			case "room_in":
				what, owner, open = "room", to, to.def.upper.value
			case "floor_back":
				what, owner, open = "floor", to, -to.def.lower.value
			default:
				continue
			}
			xid := chID(what, owner.id.hex())
			if has(xid.u128()) {
				continue
			}
			userData32 := uint32(2)
			if what == "floor" {
				userData32 = 1
			}
			add(tbAccount(xid, ledger, owner.def.code, true, r.kindID(owner.kind), userData32))
			openings = append(openings, tb.Transfer{ID: chID("opening", xid.hex()).u128(), DebitAccountID: sink.u128(), CreditAccountID: xid.u128(), Amount: amount(open), UserData32: opening, Ledger: ledger, Code: owner.def.code})
		}
		var amt int64
		if m.amount.param >= 0 {
			amt = v[m.amount.param].(int64)
		} else {
			amt = m.amount.literal
		}
		var flags uint16
		var timeout uint32
		if op == "hold" {
			flags, timeout = flagPending, t.timeout
		}
		for _, role := range m.roles {
			x := tb.Transfer{ID: r.transferID(kind, op, key, len(chain)).u128(), UserData128: contentID.u128(), UserData32: roleCodes[role], Ledger: ledger, Code: t.code}
			switch role {
			case "main":
				x.DebitAccountID, x.CreditAccountID, x.Amount, x.Flags, x.Timeout = from.id.u128(), to.id.u128(), amount(amt), flags, timeout
			case "probe":
				x.DebitAccountID, x.CreditAccountID, x.Amount, x.Flags = from.id.u128(), sink.u128(), amount(from.def.lower.value), flagPending
			case "probe_void":
				x.PendingID, x.Flags, x.Ledger, x.Code = chain[len(chain)-1].transfer.ID, flagVoid, 0, 0
			case "floor_out":
				x.DebitAccountID, x.CreditAccountID, x.Amount, x.Flags, x.Timeout = chID("floor", from.id.hex()).u128(), sink.u128(), amount(amt), flags, timeout
			case "room_back":
				x.DebitAccountID, x.CreditAccountID, x.Amount, x.Flags, x.Timeout = sink.u128(), chID("room", from.id.hex()).u128(), amount(amt), flags, timeout
			case "room_in":
				x.DebitAccountID, x.CreditAccountID, x.Amount, x.Flags, x.Timeout = chID("room", to.id.hex()).u128(), sink.u128(), amount(amt), flags, timeout
			case "floor_back":
				x.DebitAccountID, x.CreditAccountID, x.Amount, x.Flags, x.Timeout = sink.u128(), chID("floor", to.id.hex()).u128(), amount(amt), flags, timeout
			}
			chain = append(chain, planned{transfer: x, role: role, move: i})
		}
	}
	link(chain)
	if err := r.ensure(ctx, accounts, openings); err != nil {
		return Result{}, err
	}
	return r.send(ctx, kind, op, chain)
}

// ensure makes sure the accounts and the openings are there before a chain needs them:
// TigerBeetle refuses a transfer to an account it does not have, and never takes its ID again.
func (r *tbRuntime) ensure(ctx context.Context, accounts []tb.Account, openings []tb.Transfer) error {
	r.mu.Lock()
	var newAccounts []tb.Account
	for _, a := range accounts {
		if !r.made[le(a.ID)] {
			newAccounts = append(newAccounts, a)
		}
	}
	r.mu.Unlock()
	for i := 0; i < len(newAccounts); i += requestMax {
		batch := newAccounts[i:min(i+requestMax, len(newAccounts))]
		if err := ctx.Err(); err != nil {
			return err
		}
		results, err := r.client.CreateAccounts(batch)
		if err != nil {
			return err
		}
		for j, res := range results {
			if res.Status != tb.AccountCreated && res.Status != tb.AccountExists {
				return fmt.Errorf("chobo: TigerBeetle has the account %s made another way (%s): the book's accounts have changed since it was made", le(batch[j].ID).hex(), res.Status)
			}
		}
		r.mu.Lock()
		for _, a := range batch {
			r.made[le(a.ID)] = true
		}
		r.mu.Unlock()
	}
	r.mu.Lock()
	var newOpenings []tb.Transfer
	for _, x := range openings {
		if !r.made[le(x.ID)] {
			newOpenings = append(newOpenings, x)
		}
	}
	r.mu.Unlock()
	for i := 0; i < len(newOpenings); i += requestMax {
		batch := newOpenings[i:min(i+requestMax, len(newOpenings))]
		if err := ctx.Err(); err != nil {
			return err
		}
		results, err := r.client.CreateTransfers(batch)
		if err != nil {
			return err
		}
		for j, res := range results {
			if res.Status == tb.TransferExistsWithDifferentAmount {
				return fmt.Errorf("chobo: the account %s was opened with another bound: the book's bounds have changed since it was made", le(batch[j].CreditAccountID).hex())
			}
			if res.Status != tb.TransferCreated && res.Status != tb.TransferExists {
				return fmt.Errorf("chobo: TigerBeetle answered %s for the opening %s", res.Status, le(batch[j].ID).hex())
			}
		}
		r.mu.Lock()
		for _, x := range batch {
			r.made[le(x.ID)] = true
		}
		r.mu.Unlock()
	}
	return nil
}

// le reads TigerBeetle's ID back as chobo writes it.
func le(v tb.Uint128) id128 {
	b := v.Bytes()
	var out id128
	for i := range 16 {
		out[i] = b[15-i]
	}
	return out
}

func (r *tbRuntime) send(ctx context.Context, kind, op string, chain []planned) (Result, error) {
	if err := ctx.Err(); err != nil {
		return Result{}, err
	}
	transfers := make([]tb.Transfer, len(chain))
	for i, x := range chain {
		transfers[i] = x.transfer
	}
	results, err := r.client.CreateTransfers(transfers)
	if err != nil {
		return Result{}, err
	}
	return r.read(kind, op, chain, results)
}

// read reads what a chain's results answer: the first that is neither created nor linked_event_failed decides.
func (r *tbRuntime) read(kind, op string, chain []planned, results []tb.CreateTransferResult) (Result, error) {
	t := r.book.transfers[kind]
	for i, res := range results {
		switch res.Status {
		case tb.TransferCreated, tb.TransferLinkedEventFailed:
			continue
		case tb.TransferExists:
			// the first transfer there already: the same call, made before; a later one: a chain of another shape
			if i == 0 {
				return Result{Outcome: "done_before"}, nil
			}
			return refused("key_conflict"), nil
		case tb.TransferIDAlreadyFailed:
			return refused("already_refused"), nil
		case tb.TransferExceedsCredits:
			m := t.moves[chain[i].move]
			var bound *boundDef
			if chain[i].role == "room_in" {
				bound = r.book.accounts[m.to.kind].upper
			} else {
				bound = r.book.accounts[m.from.kind].lower
			}
			if bound != nil {
				return refused(bound.reason), nil
			}
		case tb.TransferAccountsMustBeDifferent:
			return refused("same_account"), nil
		case tb.TransferPendingTransferAlreadyPosted:
			return refused("already_posted"), nil
		case tb.TransferPendingTransferAlreadyVoided:
			return refused("already_voided"), nil
		case tb.TransferPendingTransferExpired:
			return refused("expired"), nil
		case tb.TransferExceedsPendingTransferAmount:
			return refused("over_hold"), nil
		default:
			if strings.HasPrefix(res.Status.String(), "TransferExistsWithDifferent") {
				return refused("key_conflict"), nil
			}
		}
		return Result{}, fmt.Errorf("chobo: TigerBeetle answered %s for transfer %d (%s) of %s.%s", res.Status, i, chain[i].role, kind, op)
	}
	return Result{Outcome: "done"}, nil
}

type heldTransfer struct {
	role string
	move int
	id   id128
}

// held is the transfers of the chain that held for key: their roles, moves and IDs.
func (r *tbRuntime) held(kind string, key []string) []heldTransfer {
	var out []heldTransfer
	for i, m := range r.book.transfers[kind].moves {
		for _, role := range m.roles {
			out = append(out, heldTransfer{role, i, r.transferID(kind, "hold", key, len(out))})
		}
	}
	return out
}

// end is post and void: key in the order of the key; amounts in the order of the amounts, or nil for all of it.
func (r *tbRuntime) end(ctx context.Context, kind, op string, key []string, amounts []int64) (Result, error) {
	t := r.book.transfers[kind]
	for _, a := range amounts {
		if a < 0 {
			return Result{}, fmt.Errorf("chobo: %s.post: an amount is %d, and an amount is from 0 to 2^63 - 1", kind, a)
		}
	}
	held := r.held(kind, key)
	var mains []heldTransfer
	var ids []tb.Uint128
	for _, h := range held {
		if h.role == "main" {
			mains = append(mains, h)
			ids = append(ids, h.id.u128())
		}
	}
	if err := ctx.Err(); err != nil {
		return Result{}, err
	}
	// read the hold first: a post or void of a hold TigerBeetle does not have would use up its ID for good
	found, err := r.client.LookupTransfers(ids)
	if err != nil {
		return Result{}, err
	}
	if len(found) == 0 {
		return refused("no_such_hold"), nil
	}
	if len(found) != len(mains) {
		return Result{}, fmt.Errorf("chobo: TigerBeetle has only part of the hold %s(%s)", kind, strings.Join(key, ", "))
	}
	holding := make([]int64, len(mains))
	for i, h := range mains {
		for _, x := range found {
			if x.ID == h.id.u128() {
				holding[i] = x.Amount.BigInt().Int64()
			}
		}
	}
	var posted []int64
	if op == "post" {
		if amounts == nil {
			posted = holding
		} else {
			var amountParams []int
			for i, p := range t.params {
				if p.amount {
					amountParams = append(amountParams, i)
				}
			}
			for _, m := range t.moves {
				if m.amount.param >= 0 {
					for j, p := range amountParams {
						if p == m.amount.param {
							posted = append(posted, amounts[j])
						}
					}
				} else {
					posted = append(posted, m.amount.literal)
				}
			}
			// TigerBeetle checks the amount before it checks whether the hold has ended: ask where the hold is instead
			for i := range posted {
				if posted[i] > holding[i] {
					state, err := r.state(ctx, mains[0].id)
					if err != nil {
						return Result{}, err
					}
					switch state {
					case Held:
						return refused("over_hold"), nil
					case Posted:
						return refused("key_conflict"), nil
					case Voided:
						return refused("already_voided"), nil
					case Expired:
						return refused("expired"), nil
					}
					return refused("no_such_hold"), nil
				}
			}
		}
	}
	what := "null"
	if op == "post" {
		parts := make([]string, len(posted))
		for i, p := range posted {
			parts[i] = strconv.FormatInt(p, 10)
		}
		what = "[" + strings.Join(parts, ",") + "]"
	}
	contentID := chID("content", r.book.name, r.tenant, kind, op, t.definition, what)
	var chain []planned
	for _, h := range held {
		if h.role == "probe" || h.role == "probe_void" {
			continue
		}
		x := tb.Transfer{ID: r.transferID(kind, op, key, len(chain)).u128(), PendingID: h.id.u128(), UserData128: contentID.u128(), UserData32: roleCodes[h.role]}
		switch {
		case op == "void":
			x.Flags = flagVoid
		case amounts == nil:
			x.Flags, x.Amount = flagPost, tb.AmountMax
		default:
			x.Flags, x.Amount = flagPost, amount(posted[h.move])
		}
		chain = append(chain, planned{transfer: x, role: h.role, move: h.move})
	}
	link(chain)
	return r.send(ctx, kind, op, chain)
}

// state is where the hold of the pending transfer id is, as TigerBeetle would answer a void of it
// now: a void and a transfer that cannot go through, linked, so that nothing is kept.
func (r *tbRuntime) state(ctx context.Context, id id128) (HoldState, error) {
	a, err := randomID()
	if err != nil {
		return "", err
	}
	b, err := randomID()
	if err != nil {
		return "", err
	}
	if err := ctx.Err(); err != nil {
		return "", err
	}
	results, err := r.client.CreateTransfers([]tb.Transfer{
		{ID: a, PendingID: id.u128(), Flags: flagLinked | flagVoid},
		{ID: b, DebitAccountID: tb.ToUint128(1), CreditAccountID: tb.ToUint128(1), Ledger: 1, Code: 1},
	})
	if err != nil {
		return "", err
	}
	switch results[0].Status {
	case tb.TransferLinkedEventFailed:
		if results[1].Status == tb.TransferAccountsMustBeDifferent {
			return Held, nil
		}
	case tb.TransferPendingTransferAlreadyPosted:
		return Posted, nil
	case tb.TransferPendingTransferAlreadyVoided:
		return Voided, nil
	case tb.TransferPendingTransferExpired:
		return Expired, nil
	case tb.TransferPendingTransferNotFound:
		return "", nil
	}
	return "", fmt.Errorf("chobo: TigerBeetle answered %s, %s when asked where a hold is", results[0].Status, results[1].Status)
}

func (r *tbRuntime) status(ctx context.Context, kind string, key []string) (HoldState, error) {
	return r.state(ctx, r.held(kind, key)[0].id)
}

func (r *tbRuntime) balance(ctx context.Context, kind string, args []string) (Balance, error) {
	if err := ctx.Err(); err != nil {
		return Balance{}, err
	}
	id := chID(append([]string{"account", r.book.name, r.tenant, kind}, args...)...)
	found, err := r.client.LookupAccounts([]tb.Uint128{id.u128()})
	if err != nil {
		return Balance{}, err
	}
	if len(found) == 0 {
		return Balance{}, nil
	}
	a := found[0]
	posted := new(big.Int).Sub(a.CreditsPosted.BigInt(), a.DebitsPosted.BigInt())
	values := []*big.Int{posted, a.CreditsPending.BigInt(), a.DebitsPending.BigInt()}
	for _, x := range values {
		if !x.IsInt64() {
			return Balance{}, fmt.Errorf("chobo: the balance of %s(%s) is past a 64-bit integer", kind, strings.Join(args, ", "))
		}
	}
	return Balance{Posted: values[0].Int64(), HeldIn: values[1].Int64(), HeldOut: values[2].Int64()}, nil
}
