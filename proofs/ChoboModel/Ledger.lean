/-
  What a chobo book means: its transfers, carried out one call at a time (chobo's DESIGN 2).

  This is the reference interpreter of chobo (`crates/chobo/src/interp.rs`) written as functions,
  and it is the thing `ChoboModel.Bounds` proves its theorems about and `ritsu-model` runs. The
  book arrives already resolved: accounts and transfer kinds by their place in the book, a move's
  accounts and amount by the call's parameters, the expiry in seconds. What the Rust tests compare
  is this file and `interp.rs`, call by call, on the scenarios chobo writes for every book.

  The state is kept as association lists that grow at the front: a later entry for the same key
  hides the earlier ones. That is what makes the proofs short — an update is one `::` — and the
  lists only ever hold what the calls of one scenario touched.
-/

namespace ChoboModel

/-- A value a call passes: a string, or an amount counted in the unit's smallest unit. -/
inductive Val where
  | str (s : String)
  | amt (a : Int)
  deriving DecidableEq, Repr

/-- The value as it goes into an account's arguments or a key. -/
def Val.text : Val → String
  | .str s => s
  | .amt a => toString a

/-- A bound and the reason a call it refuses is given (`at least 0 refused as 在庫切れ`). -/
structure Bound where
  value : Int
  refusal : String
  deriving DecidableEq, Repr

/-- An account kind: its name and its bounds. An `outside` account has neither. -/
structure AccountKind where
  name : String
  lower : Option Bound
  upper : Option Bound
  deriving Repr

/-- One argument of an account a move names: a parameter of the transfer, or a literal. -/
inductive Arg where
  | param (i : Nat)
  | lit (s : String)
  deriving Repr

/-- What a move moves: a parameter of the transfer, or a literal amount. -/
inductive Amount where
  | param (i : Nat)
  | lit (a : Int)
  deriving Repr

/-- An account as a move names it. -/
structure Ref where
  kind : Nat
  args : List Arg
  deriving Repr

/-- `move <amount> from <src> to <dst>`. -/
structure Move where
  amount : Amount
  src : Ref
  dst : Ref
  deriving Repr

/-- How a hold ends on its own: after so many seconds, or never. -/
inductive Expiry where
  | after (secs : Nat)
  | never
  deriving Repr

/-- A transfer kind: for each parameter whether it is an amount, the key, whether it holds, and
    the moves in the order they are written. -/
structure TransferKind where
  name : String
  amounts : List Bool
  key : List Nat
  pending : Option Expiry
  moves : List Move
  deriving Repr

structure Book where
  accounts : List AccountKind
  transfers : List TransferKind
  deriving Repr

/-- One account: its kind and the values of its arguments (DESIGN 1.3). -/
structure AccountId where
  kind : Nat
  args : List String
  deriving DecidableEq, Repr

/-- What an account holds: posted, held coming in, held going out (DESIGN 2.1). -/
structure Bal where
  posted : Int := 0
  heldIn : Int := 0
  heldOut : Int := 0
  deriving DecidableEq, Repr

inductive Op where
  | «do»
  | hold
  | post
  | void
  deriving DecidableEq, Repr

def Op.name : Op → String
  | .«do» => "do"
  | .hold => "hold"
  | .post => "post"
  | .void => "void"

inductive HoldState where
  | held
  | posted
  | voided
  | expired
  deriving DecidableEq, Repr

def HoldState.name : HoldState → String
  | .held => "held"
  | .posted => "posted"
  | .voided => "voided"
  | .expired => "expired"

/-- One move of a hold, its accounts and amount fixed when it was made. -/
structure HeldMove where
  src : AccountId
  dst : AccountId
  amount : Int
  deriving DecidableEq, Repr

structure Hold where
  moves : List HeldMove
  state : HoldState
  created : Nat
  /-- When it expires, in seconds from the start; none for `never expires`. -/
  deadline : Option Nat
  /-- What was posted, move by move, once it is. -/
  posted : Option (List Int)
  deriving DecidableEq, Repr

/-- What a key was used for (DESIGN 2.3): a call that went through, with the arguments a second
    call under the key is compared with, or a call a bound refused. -/
inductive KeyRec where
  | done (content : List (Option Val))
  | refused
  deriving DecidableEq, Repr

/-- A hold is known by its transfer kind and the values of its key. -/
abbrev HoldKey := Nat × List String
/-- A key is used once per transfer kind and operation. -/
abbrev KeySlot := Nat × Op × List String

structure State where
  now : Nat := 0
  accounts : List (AccountId × Bal) := []
  holds : List (HoldKey × Hold) := []
  keys : List (KeySlot × KeyRec) := []
  deriving DecidableEq, Repr

/-- One call: an operation on a transfer kind, its arguments by parameter (every one for `do`
    and `hold`, the key's for `post` and `void`), and for a `post` in part, the amounts. -/
structure Call where
  op : Op
  kind : Nat
  args : List (Option Val)
  amounts : Option (List (Nat × Int))
  deriving DecidableEq, Repr

inductive Outcome where
  | done
  | doneBefore
  | refused (reason : String)
  deriving DecidableEq, Repr

/-! ## Looking things up -/

/-- The first entry under a key: the latest one, since the lists grow at the front. -/
def find? {α β : Type} [DecidableEq α] : List (α × β) → α → Option β
  | [], _ => none
  | (k, v) :: rest, a => if k = a then some v else find? rest a

/-- The balance a list grown at the front gives an account: zero for one not used yet. -/
def bal (accts : List (AccountId × Bal)) (a : AccountId) : Bal := (find? accts a).getD {}

def State.balance (s : State) (a : AccountId) : Bal := bal s.accounts a

def lowerOf (b : Book) (a : AccountId) : Option Bound := (b.accounts[a.kind]?).bind (·.lower)
def upperOf (b : Book) (a : AccountId) : Option Bound := (b.accounts[a.kind]?).bind (·.upper)

/-- The largest amount and the largest balance either way: what PostgreSQL's `bigint` holds. -/
def maxAmount : Int := 9223372036854775807

/-! ## A call's own values -/

/-- A parameter as a string: the account arguments and the key read it this way. -/
def Call.str (c : Call) (i : Nat) : String :=
  match c.args[i]? with
  | some (some v) => v.text
  | _ => ""

def Call.account (c : Call) (r : Ref) : AccountId :=
  { kind := r.kind, args := r.args.map (fun a => match a with
      | .param i => c.str i
      | .lit s => s) }

def Call.amount (c : Call) (m : Move) : Int :=
  match m.amount with
  | .param i => match c.args[i]? with
    | some (some (.amt v)) => v
    | _ => 0
  | .lit v => v

def Call.key (c : Call) (t : TransferKind) : List String := t.key.map c.str

/-- The moves of a `do` or a `hold`, with their accounts and amounts. -/
def Call.moves (c : Call) (t : TransferKind) : List HeldMove :=
  t.moves.map (fun m => { src := c.account m.src, dst := c.account m.dst, amount := c.amount m })

/-- Whether the call fits the transfer kind: the operation, the number of arguments, and every
    amount from 0 to 2⁶³ − 1. A call that does not fit is a mistake of the caller, not a refusal
    (DESIGN 1.5), and the interpreter answers it with an error. -/
def fits (t : TransferKind) (c : Call) : Bool :=
  (match c.op with
   | .«do» => t.pending.isNone
   | _ => t.pending.isSome) &&
  c.args.length == t.amounts.length &&
  c.args.all (fun v => match v with
    | some (.amt a) => decide (0 ≤ a) && decide (a ≤ maxAmount)
    | _ => true) &&
  (match c.amounts with
   | none => true
   | some a => c.op == .post && a.all (fun p => decide (0 ≤ p.2) && decide (p.2 ≤ maxAmount))) &&
  t.moves.all (fun m => decide (0 ≤ c.amount m))

/-! ## The moves of a `do` or a `hold` (DESIGN 2.2) -/

/-- An account's balance as the moves so far left it: from the scratch list, or the state's. -/
def scratchBal (scratch : List (AccountId × Bal)) (s : State) (a : AccountId) : Bal :=
  (find? scratch a).getD (s.balance a)

/-- Taking `a` from an account: refused by its lower bound when what is posted, less what is
    held going out, less `a` falls below it. What is held coming in does not count. -/
def lowRefuses (b : Book) (bal : Bal) (m : HeldMove) : Option String :=
  match lowerOf b m.src with
  | some l => if bal.posted - bal.heldOut - m.amount < l.value then some l.refusal else none
  | none => none

/-- Putting `a` into an account: refused by its upper bound when what is posted, plus what is
    held coming in, plus `a` goes over it. What is held going out does not count. -/
def highRefuses (b : Book) (bal : Bal) (m : HeldMove) : Option String :=
  match upperOf b m.dst with
  | some u => if bal.posted + bal.heldIn + m.amount > u.value then some u.refusal else none
  | none => none

def takeOut (hold : Bool) (bal : Bal) (a : Int) : Bal :=
  if hold then { bal with heldOut := bal.heldOut + a } else { bal with posted := bal.posted - a }

def putIn (hold : Bool) (bal : Bal) (a : Int) : Bal :=
  if hold then { bal with heldIn := bal.heldIn + a } else { bal with posted := bal.posted + a }

/-- What the moves come to: refused by a bound, or the balances they leave. -/
inductive Moved where
  | refusedAt (reason : String)
  | moved (scratch : List (AccountId × Bal))

/-- The moves in the order they are written, each checked against the balances the moves before
    it left. The first that a bound refuses refuses them all. -/
def runMoves (b : Book) (hold : Bool) (s : State) :
    List HeldMove → List (AccountId × Bal) → Moved
  | [], scratch => .moved scratch
  | m :: ms, scratch =>
    match lowRefuses b (scratchBal scratch s m.src) m with
    | some r => .refusedAt r
    | none =>
      match highRefuses b (scratchBal scratch s m.dst) m with
      | some r => .refusedAt r
      | none =>
        let scratch1 := (m.src, takeOut hold (scratchBal scratch s m.src) m.amount) :: scratch
        let scratch2 := (m.dst, putIn hold (scratchBal scratch1 s m.dst) m.amount) :: scratch1
        runMoves b hold s ms scratch2

def inRange (bal : Bal) : Bool :=
  decide (-maxAmount - 1 ≤ bal.posted) && decide (bal.posted ≤ maxAmount) &&
  decide (-maxAmount - 1 ≤ bal.heldIn) && decide (bal.heldIn ≤ maxAmount) &&
  decide (-maxAmount - 1 ≤ bal.heldOut) && decide (bal.heldOut ≤ maxAmount)

/-- When a hold of this kind made now expires. -/
def deadlineOf (t : TransferKind) (now : Nat) : Option Nat :=
  match t.pending with
  | some (.after secs) => some (now + secs)
  | _ => none

/-- A `do` or a `hold` (DESIGN 2.2, 2.3): the same account on both sides of a move, then the key,
    then the moves. -/
def State.moveOrHold (s : State) (b : Book) (t : TransferKind) (c : Call) :
    Except String (Outcome × State) :=
  let moves := c.moves t
  if moves.any (fun m => decide (m.src = m.dst)) then .ok (.refused "same_account", s) else
  let slot : KeySlot := (c.kind, c.op, c.key t)
  match find? s.keys slot with
  | some .refused => .ok (.refused "already_refused", s)
  | some (.done before) => .ok (if before = c.args then .doneBefore else .refused "key_conflict", s)
  | none =>
    match runMoves b (c.op == .hold) s moves [] with
    | .refusedAt r => .ok (.refused r, { s with keys := (slot, .refused) :: s.keys })
    | .moved scratch =>
      if moves.all (fun m => inRange (scratchBal scratch s m.src) && inRange (scratchBal scratch s m.dst)) then
        let s1 : State := { s with accounts := scratch ++ s.accounts }
        let s2 : State := if c.op == .hold then
            { s1 with holds := ((c.kind, c.key t),
                { moves := moves, state := .held, created := s.now, deadline := deadlineOf t s.now, posted := none }) :: s1.holds }
          else s1
        .ok (.done, { s2 with keys := (slot, .done c.args) :: s2.keys })
      else .error "a balance would leave the range of a 64-bit integer"

/-! ## How a hold ends (DESIGN 2.4) -/

/-- What settling one move of a hold does to the account it takes from: the held amount comes
    off, and what it posts goes. -/
def settleOut (m : HeldMove) (p : Int) (b : Bal) : Bal :=
  { b with heldOut := b.heldOut - m.amount, posted := b.posted - p }

/-- …and to the account it puts into: the held amount comes off, and what it posts arrives. -/
def settleIn (m : HeldMove) (p : Int) (b : Bal) : Bal :=
  { b with heldIn := b.heldIn - m.amount, posted := b.posted + p }

/-- Moving what a hold holds, move by move, each posting what the list says. -/
def settle : List (HeldMove × Int) → List (AccountId × Bal) → List (AccountId × Bal)
  | [], accts => accts
  | (m, p) :: rest, accts =>
    let a1 := (m.src, settleOut m p (bal accts m.src)) :: accts
    settle rest ((m.dst, settleIn m p (bal a1 m.dst)) :: a1)

/-- What a hold gives back when it is voided or expires: every move, posting nothing. -/
def release (h : Hold) : List (HeldMove × Int) := h.moves.map (fun m => (m, 0))

def expiredAt (h : Hold) (now : Nat) : Bool :=
  match h.deadline with
  | some d => decide (d ≤ now)
  | none => false

/-- What a `post` moves, move by move: all of it, or what the amounts make of each move. -/
def postAmounts (t : TransferKind) (c : Call) (h : Hold) : List Int :=
  match c.amounts with
  | none => h.moves.map (·.amount)
  | some a => t.moves.map (fun m => match m.amount with
    | .param i => (find? a i).getD 0
    | .lit v => v)

def overHold (amounts : List Int) (moves : List HeldMove) : Bool :=
  (moves.zip amounts).any (fun p => decide (p.2 > p.1.amount))

def State.post (s : State) (t : TransferKind) (c : Call) : Outcome × State :=
  let hk : HoldKey := (c.kind, c.key t)
  match find? s.holds hk with
  | none => (.refused "no_such_hold", s)
  | some h =>
    let amounts := postAmounts t c h
    match h.state with
    | .held =>
      if expiredAt h s.now then (.refused "expired", s)
      else if overHold amounts h.moves then (.refused "over_hold", s)
      else (.done, { s with accounts := settle (h.moves.zip amounts) s.accounts,
                            holds := (hk, { h with state := .posted, posted := some amounts }) :: s.holds })
    | .posted => (if h.posted = some amounts then .doneBefore else .refused "key_conflict", s)
    | .voided => (.refused "already_voided", s)
    | .expired => (.refused "expired", s)

def State.void (s : State) (t : TransferKind) (c : Call) : Outcome × State :=
  let hk : HoldKey := (c.kind, c.key t)
  match find? s.holds hk with
  | none => (.refused "no_such_hold", s)
  | some h =>
    match h.state with
    | .held =>
      if expiredAt h s.now then (.refused "expired", s)
      else (.done, { s with accounts := settle (release h) s.accounts,
                            holds := (hk, { h with state := .voided }) :: s.holds })
    | .posted => (.refused "already_posted", s)
    | .voided => (.doneBefore, s)
    | .expired => (.refused "expired", s)

/-- **One call.** An error is a mistake in the call (DESIGN 1.5: a failure, not a refusal), and
    then nothing has changed. -/
def State.apply (s : State) (b : Book) (c : Call) : Except String (Outcome × State) :=
  match b.transfers[c.kind]? with
  | none => .error "no such transfer kind"
  | some t =>
    if fits t c then
      match c.op with
      | .«do» => s.moveOrHold b t c
      | .hold => s.moveOrHold b t c
      | .post => .ok (s.post t c)
      | .void => .ok (s.void t c)
    else .error "the call does not fit its transfer kind"

/-! ## Time (DESIGN 2.5) -/

/-- Each key once, the latest entry under it. -/
def latest {α β : Type} [DecidableEq α] : List (α × β) → List (α × β)
  | [] => []
  | (k, v) :: rest => (k, v) :: (latest rest).filter (fun e => decide (e.1 ≠ k))

/-- One hold expiring: what it held goes back. -/
def State.expire (st : State) (e : HoldKey × Hold) : State :=
  { st with accounts := settle (release e.2) st.accounts,
            holds := (e.1, { e.2 with state := .expired }) :: st.holds }

/-- Let time pass. A hold whose expiry the clock reaches expires then, and what it held goes
    back. -/
def State.pass (s : State) (secs : Nat) : State :=
  let now := s.now + secs
  let due := (latest s.holds).filter (fun e => decide (e.2.state = .held) && expiredAt e.2 now)
  due.foldl State.expire { s with now := now }

end ChoboModel
