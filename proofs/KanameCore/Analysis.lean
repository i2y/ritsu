/-
  The analysis of ranges (kaname's DESIGN 5, `ranges.rs`): what the values of every expression of a
  function can be, as an estimate from above, worked out from what its parameters declare; and each
  place where the evaluation can fail, with the values it is given and whether they are shown to
  keep what it takes. `Sound.lean` proves the estimate holds every value the evaluation gives, and
  that where every place is shown, the evaluation fails with none of the five kinds.

  It is `ranges.rs` written again, function by function, so that `ritsu-model` can be held to it
  line by line: the same abstract values, the same places, the same order of narrowing.
  - An `if` and a `match` narrow, in each branch, the names the condition is about (a local and
    the fields under it); a comparison of two names is kept as a relation. Where `ranges.rs` reads
    the value an expression came to last, this reads it again in the same scope, which gives the
    same value: the narrowing comes right after the condition, in the same names.
  - A function is analysed once; a call takes what it answers cut to what its result declares, and
    a function in a ring of calls answers what its result declares.
  - Each place is kept by its expression and what it is (`key`), the values of every time it is
    reached joined, as `ranges.rs` keeps it.
  - Each call of a rule keeps the relations between the inputs it gives (`callRels`): what kaname
    hands ritsu-cross for X2 (DESIGN 14.2).
  - The verdicts of the checks across the languages are taken after the analysis, as `ranges.rs`
    takes them, and the effects inferred from them (`analyzeWith`).

  The six places where `ranges.rs` once was not an estimate from above (the sum of values whose type
  says no step, a value read in another unit inside a list, `x in u` of a value that can be anything
  or none, an optional of anything at a place for int64, an upper end that rounds past int64, a
  comparison that reads a side in another unit) are written in `ranges.rs` as here (DESIGN 15.3).
-/
import KanameCore.Eval

namespace KanameCore

/-- What a place takes, as far as whether it is shown reads it (`ranges::What`). -/
inductive What where
  /-- An input of a rule, a field of the elements of a list a rule walks, a whole-number input of
      a date: the range. -/
  | ranged (r : Range)
  /-- A parameter of a function, as a call gives it. -/
  | param (s : Slot)
  /-- What a function answers, or a field of a record of a module as the record is built. -/
  | slot (s : Slot)
  | isqrt
  | overflow
  | zero
  /-- A check across the languages, which ritsu-cross decides (X2, X3, X4, X6), waiting for its
      verdict. -/
  | pending
  /-- A check across the languages, with ritsu-cross's verdict: shown to hold, or not (an example,
      or undecided). The analysis makes none: the verdicts are taken after it (`analyzeWith`). -/
  | across (holds : Bool)
  /-- An input of a rule of an ordered enum held to a range over its values: the values the range
      lets in (`ranges::What::RuleEnum`). -/
  | oneOf (vs : List String)
  deriving Repr, Inhabited

structure Place where
  expr : Nat
  key : String
  kind : Kind
  what : What
  value : Abs
  deriving Repr, Inhabited

mutual
def Slot.beq : Slot → Slot → Bool
  | .mk l c r u rc fs vs ks rf, .mk l' c' r' u' rc' fs' vs' ks' rf' =>
    l == l' && c == c' && r == r' && u == u' && rc == rc' && rf == rf' && Slot.beqFields fs fs' &&
      Slot.beqValues vs vs' && Slot.beqKeys ks ks'
def Slot.beqKeys : Option Slot → Option Slot → Bool
  | none, none => true
  | some a, some b => Slot.beq a b
  | _, _ => false
def Slot.beqFields : List (String × Slot) → List (String × Slot) → Bool
  | [], [] => true
  | (n, s) :: xs, (m, t) :: ys => n == m && Slot.beq s t && Slot.beqFields xs ys
  | _, _ => false
def Slot.beqValues : List (String × List (String × Slot)) → List (String × List (String × Slot)) → Bool
  | [], [] => true
  | (t, f) :: xs, (u, g) :: ys => t == u && Slot.beqFields f g && Slot.beqValues xs ys
  | _, _ => false
end

def What.beq : What → What → Bool
  | .ranged r, .ranged s => r == s
  | .param s, .param t => s.beq t
  | .slot s, .slot t => s.beq t
  | .isqrt, .isqrt => true
  | .overflow, .overflow => true
  | .zero, .zero => true
  | .pending, .pending => true
  | .across a, .across b => a == b
  | .oneOf a, .oneOf b => a == b
  | _, _ => false

/-- What the analysis keeps as it goes: the places, the values of every expression (joined over
    the times it is reached), the functions called, and the relations each call of a rule keeps
    between the inputs it gives (what `Programs` hands ritsu-cross for X2: kaname's DESIGN 14.2). -/
structure Out where
  places : List Place
  vals : List (Nat × Abs)
  calls : List Nat
  /-- Whether a `reduce` let the ends of its running value go (DESIGN 5.5), or the analysis called a
      function value it does not see (kaname's DESIGN 27.4): what the theorems leave out (DESIGN
      15.4). -/
  widened : Bool
  kept : List (Nat × List (String × Rel × String))
  /-- The values of the expressions of the bodies of lambdas the function itself made, and the
      relations their calls of rules keep (kaname's DESIGN 27.4): kept apart, so that a reading
      again that reads one does not take them away with its own. -/
  lvals : List (Nat × Abs) := []
  lkept : List (Nat × List (String × Rel × String)) := []
  /-- The effects written on the types of the function values called that the analysis does not see
      (DESIGN 27.5): the kinds, and whether it may not stop. -/
  thru : List (List Kind × Bool) := []
  /-- Whether the analysis called a function value it does not see (DESIGN 27.4). -/
  unseen : Bool := false
  deriving Repr, Inhabited

def Out.empty : Out := { places := [], vals := [], calls := [], widened := false, kept := [] }

/-- The relations a call of a rule keeps between its inputs, by the call's expression, joined with
    those of the times before: a call reached more than once (in a function given to `reduce`)
    keeps those that hold every time (`ranges.rs`'s `site`). -/
def Out.keep (o : Out) (e : Nat) (rels : List (String × Rel × String)) : Out :=
  if o.kept.any (·.1 == e) then
    { o with kept := o.kept.map fun (i, rs) => if i == e then (i, rs.filter (fun r => rels.contains r)) else (i, rs) }
  else { o with kept := o.kept ++ [(e, rels)] }

/-- Whether a place is the one kept by an expression and a key (and what it takes, which the
    expression and the key decide). -/
def Place.same (p : Place) (e : Nat) (key : String) (kind : Kind) (what : What) : Bool :=
  p.expr == e && p.key == key && p.kind == kind && p.what.beq what

/-- A place, joined with the one kept already by its expression and key. -/
def Out.place (o : Out) (e : Nat) (key : String) (kind : Kind) (what : What) (v : Abs) : Out :=
  if o.places.any (fun p => p.same e key kind what) then
    { o with places := o.places.map fun p =>
        if p.same e key kind what then { p with value := p.value.join v } else p }
  else { o with places := o.places ++ [⟨e, key, kind, what, v⟩] }

/-- The value an expression came to, joined with those of the times before. -/
def Out.record (o : Out) (e : Nat) (v : Abs) : Out :=
  if o.vals.any (·.1 == e) then
    { o with vals := o.vals.map fun (i, x) => if i == e then (i, x.join v) else (i, x) }
  else { o with vals := o.vals ++ [(e, v)] }

/-- The same, of an expression of the body of a lambda the function itself made: kept apart. -/
def Out.recordL (o : Out) (e : Nat) (v : Abs) : Out :=
  if o.lvals.any (·.1 == e) then
    { o with lvals := o.lvals.map fun (i, x) => if i == e then (i, x.join v) else (i, x) }
  else { o with lvals := o.lvals ++ [(e, v)] }

/-- The relations a call of a rule in the body of a lambda the function itself made keeps: apart. -/
def Out.keepL (o : Out) (e : Nat) (rels : List (String × Rel × String)) : Out :=
  if o.lkept.any (·.1 == e) then
    { o with lkept := o.lkept.map fun (i, rs) => if i == e then (i, rs.filter (fun r => rels.contains r)) else (i, rs) }
  else { o with lkept := o.lkept ++ [(e, rels)] }

/-- What the bodies of the lambdas the function itself made gave, put with the rest. -/
def Out.merged (o : Out) : Out :=
  let o' := o.lvals.foldl (fun o (i, a) => o.record i a) { o with lvals := [] }
  o.lkept.foldl (fun o (i, rels) => o.keep i rels) { o' with lkept := [] }

/-- A name and the fields under it, that a branch narrows: `invoice.amount`. -/
abbrev Sym := String × List String

/-- What the conditions in force relate: a name and the fields under it, or the sum of a list that is
    a name by a field of its elements (`sum(xs, x => x.amount)`, kaname's DESIGN 32, 33): the list's
    name and fields, and the path the lambda answers of its parameter. Rust writes the second as a
    name with the step `⅀` between the two (`ranges::SUM`); a branch narrows no such name. -/
inductive Term where
  | name (s : Sym)
  | sum (list : Sym) (path : List String)
  deriving DecidableEq, Repr, Inhabited

/-- The name in scope a term reads: the name, or the list's. -/
def Term.base : Term → String
  | .name s => s.1
  | .sum l _ => l.1

/-- The scope of the analysis of one function: the module and what its functions answer once they
    are analysed, the ring of the function, the names in scope with their values, what the
    conditions in force say of how two names compare, and how many readings again of a body the
    analysis is in (kaname's DESIGN 26.5; none in a function's own analysis). -/
structure ACtx where
  M : Module
  results : List (Option Abs)
  ring : List Nat
  env : List (String × Abs)
  facts : List (Term × Rel × Term)
  depth : Nat
  /-- Whether the analysis is of the function itself, not a reading again of another (what the
      lambdas it makes say they were made in), and whether it reads the body of a lambda the function
      itself made (kaname's DESIGN 27.4). -/
  own : Bool := true
  lam : Bool := false
  deriving Inhabited

namespace ACtx

def lookup (A : ACtx) (x : String) : Option Abs := (A.env.find? (·.1 == x)).map (·.2)

def bind (A : ACtx) (x : String) (a : Abs) : ACtx := { A with env := (x, a) :: A.env }

/-- A name the evaluation gives a value to (a `let`, `some x`, a parameter of a function given to a
    builtin): what the conditions in force said of an earlier one of the name is not of this one.
    The check of types lets no name be given twice in a scope (E006), so nothing is dropped there. -/
def bindNew (A : ACtx) (x : String) (a : Abs) : ACtx :=
  { A with env := (x, a) :: A.env, facts := A.facts.filter fun (p, _, q) => p.base != x && q.base != x }

/-- The parameter of a function given to a builtin that walks a list, bound to `a`: where the list
    is what `running_totals(xs, y => y.amount)` answers, the parameter's `total` is the sum `tot` of
    that list by that field (kaname's DESIGN 32, 33; `ranges::elem_total_fact`). A parameter named as
    the list is not given it: the check of types lets no name be given twice in a scope (E006). -/
def bindLam (A : ACtx) (x : String) (a : Abs) (tot : Option Term) : ACtx :=
  let B := A.bindNew x a
  match tot with
  | some t => if t.base == x then B else { B with facts := B.facts ++ [(.name (x, ["total"]), .eq, t)] }
  | none => B

def symAbs (A : ACtx) (s : Sym) : Option Abs :=
  (A.lookup s.1).map fun a => s.2.foldl (fun a f => a.field f) a

end ACtx

/-- The name and the fields an expression is, where a branch can narrow it. -/
def sym : Expr → Option Sym
  | .var _ _ x => some (x, [])
  | .field _ _ b f => (sym b).map fun (n, fs) => (n, fs ++ [f])
  | _ => none

/-- The list a `count` with no condition counts, when it is a name. -/
def counted : Expr → Option Sym
  | .count1 _ l => sym l
  | _ => none

/-- The fields a function of one parameter answers of it: `x => x.amount` is `[amount]`, `x => x`
    is `[]`; none for any other body. Two such functions are the same whatever their parameters are
    named (`ranges::field_lambda`). -/
def fieldPath (x : String) (body : Expr) : Option (List String) :=
  match sym body with
  | some (n, fs) => if n == x then some fs else none
  | none => none

/-- The sum of a list that is a name by the fields a function answers of each element
    (`ranges::sum_term`): `sum(xs, x => x.amount)` and `xs.sum(l => l.amount)` are one sum. -/
def sumTerm (list : Expr) (x : String) (body : Expr) : Option Term :=
  match sym list, fieldPath x body with
  | some s, some fs => some (.sum s fs)
  | _, _ => none

/-- A `sum` of a list that is a name by a field of its elements, as a term the facts can hold
    (`ranges::summed`). Its answers are read as the function answers them: the check of types gives
    a sum the type of what it adds up. -/
def summed : Expr → Option Term
  | .walk _ _ .sum list x body Option.none _ _ => sumTerm list x body
  | _ => none

/-- The sum each element's `total` is, where a builtin walks the list `running_totals(xs, x =>
    x.amount)` answers (`ranges::totals_of`): `sum(xs, x => x.amount)`. -/
def totalsOf : Expr → Option Term
  | .walk _ _ .running list x body Option.none _ _ => sumTerm list x body
  | _ => none

/-- What the facts hold of one side of a comparison: the name it is, or the sum of a list it is
    (`ranges::narrow_cmp`). -/
def factTerm (e : Expr) : Option Term :=
  match sym e with
  | some s => some (.name s)
  | none => summed e

/-- The values with the field at `path` put to `to` (through optionals, and in each value of an enum
    that has the field). -/
def putD : Nat → Abs → List String → Abs → Abs
  | 0, a, _, _ => a
  | _ + 1, _, [], to => to
  | d + 1, .record fs rels, f :: rest, to =>
    .record (fs.map fun (n, x) => (n, if n == f then putD d x rest to else x)) rels
  | d + 1, .variants vs, f :: rest, to =>
    .variants (vs.map fun (t, fs) => (t, fs.map fun (n, x) => (n, if n == f then putD d x rest to else x)))
  | d + 1, .opt n x, path, to => .opt n (putD d x path to)
  | _ + 1, x, _, _ => x

/-- Narrow a name and its fields to `a` from here to the end of the branch. -/
def ACtx.narrowTo (A : ACtx) (s : Sym) (a : Abs) : ACtx :=
  match A.lookup s.1 with
  | some v => A.bind s.1 (putD D v s.2 a)
  | none => A

-- ── Pieces ──────────────────────────────────────────────────────────────────

/-- The interval of a number or a date, through optionals and lists: what a number read in another
    unit is checked for int64 in. -/
def ivDeep : Abs → Option Iv
  | .num i => some i
  | .date i => some i
  | .opt _ x => ivDeep x
  | .list _ x => ivDeep x
  | _ => none

/-- The numbers of a value, through optionals and lists: what a number read in another unit is. -/
def numPart : Abs → Abs
  | .list _ x => numPart x
  | .opt n x => .opt n (numPart x)
  | x => x

/-- Whether a value can be anything where it holds a number (through optionals and lists). -/
def hasAny : Abs → Bool
  | .any => true
  | .opt _ x => hasAny x
  | .list _ x => hasAny x
  | _ => false

/-- A value read in another unit (`conv`): the place `overflow` where it can leave int64. -/
def convA (o : Out) : Option Conv → Abs → Abs × Out
  | none, a => (a, o)
  | some c, a =>
    let b := a.scaled c.f c.shift
    let o' := o.place c.expr "overflow" .overflow .overflow (numPart b)
    match ivDeep b with
    | some i => if i.fits then (b, o) else (b, o')
    | none => if hasAny b then (b, o') else (b, o)

/-- A number within int64, as the interpreter lets it through. -/
def heldAbs (a : Abs) : Abs :=
  match a with
  | .num i => .num (((i.meet ⟨some i64min, some i64max, none⟩).map fun x => { x with den := i.den }).getD i)
  | x => x

/-- An operation held to int64 at `e`: the place, and the value from here on, which the
    interpreter lets through only within int64. -/
def heldA (o : Out) (e : Nat) (a : Abs) : Abs × Out :=
  (heldAbs a, o.place e "overflow" .overflow .overflow a)

/-- What holds once a place has taken a value and the evaluation goes on: the name the value is
    keeps what the place takes. Not where the value is read in another unit to go there. -/
def assume (A : ACtx) (x : Expr) (r : Range) (c : Option Conv) : ACtx :=
  if c.isSome then A
  else match sym x with
    | none => A
    | some s => match A.symAbs s with
      | some (.num i) => match i.meet ⟨r.min, r.max, none⟩ with
        | some n => A.narrowTo s (.num n)
        | none => A
      | some (.date i) => match i.meet ⟨r.min, r.max, none⟩ with
        | some n => A.narrowTo s (.date n)
        | none => A
      | _ => A

/-- The denominator every value of a type on the step `s` has at most. -/
def stepDen : Option Rat → Option Nat
  | none => none
  | some s => some (if s = 0 then 1 else s.den)

/-- Whether a condition can hold, and whether it can fail. -/
def truths : Abs → Bool × Bool
  | .set (some s) => (s.contains "true", s.contains "false")
  | .never => (false, false)
  | _ => (true, true)

/-- Whether every value of `x` is at most every value of `y`. -/
def Iv.atMost (x y : Iv) : Bool :=
  match x.hi, y.lo with
  | some h, some l => decide (h ≤ l)
  | _, _ => false

/-- Whether `x` and `y` are each one value, the same one. -/
def Iv.sameAs (x y : Iv) : Bool :=
  match x.isExact, y.isExact with
  | some a, some b => a == b
  | _, _ => false

/-- What a comparison of two intervals comes to, when it is decided. -/
def decide' (x : Iv) (op : CmpOp) (y : Iv) : Option Bool :=
  let lt := x.below y
  let gt := y.below x
  let le := x.atMost y
  let ge := y.atMost x
  let same := x.sameAs y
  match op with
  | .lt => if lt then some true else if ge then some false else none
  | .le => if le then some true else if gt then some false else none
  | .gt => if gt then some true else if le then some false else none
  | .ge => if ge then some true else if lt then some false else none
  | .eq => if same then some true else if lt || gt then some false else none
  | .ne => if same then some false else if lt || gt then some true else none

def CmpOp.negate : CmpOp → CmpOp
  | .lt => .ge | .le => .gt | .gt => .le | .ge => .lt | .eq => .ne | .ne => .eq

def CmpOp.flip : CmpOp → CmpOp
  | .lt => .gt | .le => .ge | .gt => .lt | .ge => .le | x => x

/-- The largest value on the step below `v`, and the smallest above it. -/
def belowStep (s : Option Rat) (v : Rat) : Rat :=
  match s with
  | some s => let q := v / s; let f := q.floor; ((if (f : Rat) = q then f - 1 else f : Int) : Rat) * s
  | none => v

def aboveStep (s : Option Rat) (v : Rat) : Rat :=
  match s with
  | some s => (((v / s).floor + 1 : Int) : Rat) * s
  | none => v

/-- `x` where `x op y` holds, its values on the step `step`: `x < 5` for whole numbers is `x <= 4`. -/
def narrowIv (x : Iv) (op : CmpOp) (y : Iv) (step : Option Rat) : Option Iv :=
  let step := step.filter (fun s => s.num > 0)
  let out : Option Iv := match op with
    | .lt => some { x with hi := minHi x.hi (y.hi.map (belowStep step)) }
    | .le => some { x with hi := minHi x.hi y.hi }
    | .gt => some { x with lo := maxLo x.lo (y.lo.map (aboveStep step)) }
    | .ge => some { x with lo := maxLo x.lo y.lo }
    | .eq => x.meet y
    | .ne => match y.isExact with
      | some v =>
        if x.lo == some v && step.isSome then some { x with lo := some (aboveStep step v) }
        else if x.hi == some v && step.isSome then some { x with hi := some (belowStep step v) }
        else some x
      | none => some x
  match out with
  | some o => match o.lo, o.hi with
    | some a, some b => if b < a then none else some o
    | _, _ => some o
  | none => none

/-- The lowest sum of up to `count` values each in `w`, as many values as the count can be. -/
def sumLo (count w : Iv) : End :=
  match w.lo with
  | some l => if l.num < 0 then count.hi.map (· * l) else some ((count.lo.getD 0) * l)
  | none => none

/-- The highest. -/
def sumHi (count w : Iv) : End :=
  match w.hi with
  | some h => if h.num > 0 then count.hi.map (· * h) else some ((count.lo.getD 0) * h)
  | none => none

/-- A lower end above 0 for sums of no value: 0. -/
def adjLo (mayBeEmpty : Bool) (lo : End) : End :=
  match lo with
  | some l => if mayBeEmpty && l.num > 0 then some 0 else some l
  | none => none

def adjHi (mayBeEmpty : Bool) (hi : End) : End :=
  match hi with
  | some h => if mayBeEmpty && h.num < 0 then some 0 else some h
  | none => none

/-- The sums of up to `count` values each in `w` (and of none): what `sum` answers, and every
    partial sum on the way. -/
def sums (count w : Iv) (den : Option Nat) : Iv :=
  let mayBeEmpty := count.lo.all fun l => l.num ≤ 0
  Iv.mk' (adjLo mayBeEmpty (sumLo count w)) (adjHi mayBeEmpty (sumHi count w)) den

/-- The ends of `acc` that moved away from `start`, let go. -/
def widenD : Nat → Abs → Abs → Abs
  | 0, _, a => a
  | d + 1, s, a =>
    match s, a with
    | .num s, .num a => .num ⟨if a.lo == s.lo then a.lo else none, if a.hi == s.hi then a.hi else none, a.den⟩
    | .date s, .date a => .date ⟨if a.lo == s.lo then a.lo else none, if a.hi == s.hi then a.hi else none, a.den⟩
    | .opt _ s, .opt n a => .opt n (widenD d s a)
    | .list sc se, .list ac ae => .list { ac with hi := if ac.hi == sc.hi then ac.hi else none } (widenD d se ae)
    | .members sc se, .members ac ae => .members { ac with hi := if ac.hi == sc.hi then ac.hi else none } (widenD d se ae)
    | .str sc, .str ac => .str { ac with hi := if ac.hi == sc.hi then ac.hi else none }
    | .dict sc sk se, .dict ac ak ae =>
      .dict { ac with hi := if ac.hi == sc.hi then ac.hi else none } (widenD d sk ak) (widenD d se ae)
    | .record s _, .record fs rels =>
      .record (fs.map fun (n, x) => (n, match Abs.fieldOf s n with
        | some y => widenD d y x
        | none => .any)) rels
    -- each value of an enum, its fields as a record's; a value the start was not, every field let go
    | .variants s, .variants a =>
      .variants (a.map fun (v, r) => (v, match Abs.recOf s v with
        | some sr => r.map fun (n, x) => (n, match Abs.fieldOf sr n with
          | some y => widenD d y x
          | none => .any)
        | none => r.map fun (n, _) => (n, .any)))
    | _, a => a

/-- A value cut to what is declared of it: the intervals met, field by field. -/
def cutD : Nat → Abs → Abs → Abs
  | 0, a, _ => a
  | d + 1, a, b =>
    match a, b with
    | .num x, .num y => .num (((x.meet y).map fun i => i.withDen x.den).getD x)
    | .date x, .date y => .date ((x.meet y).getD x)
    | .opt n x, .opt _ y => .opt n (cutD d x y)
    | .list c x, .list e y => .list ((c.meet e).getD c) (cutD d x y)
    | .members c x, .members e y => .members ((c.meet e).getD c) (cutD d x y)
    | .dict c k x, .dict e l y => .dict ((c.meet e).getD c) (cutD d k l) (cutD d x y)
    | .str c, .str e => .str ((c.meet e).getD c)
    | x, _ => x

/-- How deep `keeps` follows the fields of records (`ranges::keeps_in`): to a depth of 8. -/
def KEEPS : Nat := 9

/-- What a value is where it is there. -/
def stripOpt : Abs → Abs
  | .opt _ x => x
  | x => x

/-- What each value a place holds to its range can be: an element of a list, a member of a set, a
    value of a dict, none where their count can be outside the place's, or the value itself. -/
def countedAbs (count : Option Range) (c : Iv) (e : Abs) : Option Abs :=
  if count.any (fun r => !c.within r.min r.max) then none else some (stripOpt e)

def elemAbs (list : Bool) (count : Option Range) (a : Abs) : Option Abs :=
  if list then
    match a with
    | .list c e => countedAbs count c e
    | .members c e => countedAbs count c e
    | .dict c _ e => countedAbs count c e
    | _ => none
  else some a

/-- Whether a string a place holds itself (no list) keeps the count of code points it declares
    (`ranges::keeps_in`): every string where it declares none. -/
def strOK (list : Bool) (count : Option Range) (elem : Abs) : Bool :=
  match list, count with
  | false, some c => match elem with
    | .str i => i.within c.min c.max
    | _ => false
  | _, _ => true

/-- Whether every number or date of `elem` is in a range. -/
def rangeOK (r : Option Range) (elem : Abs) : Bool :=
  match r with
  | some r => (elem.iv.map fun i => i.within r.min r.max).getD false
  | none => true

/-- Whether the fields of each value of an enum a place holds keep what they declare, each by its
    place (`keep`): the fields of each value it can be (of every value, where the analysis knows
    none of them). -/
def variantsKeep (keep : Abs → Slot → Bool) (elem : Abs) (values : List (String × List (String × Slot))) : Bool :=
  values.all fun x => match elem with
    | .variants vs => match Abs.recOf vs x.1 with
      | some r => x.2.all fun y => keep ((Abs.fieldOf r y.1).getD .any) y.2
      | none => true
    | _ => x.2.all fun y => keep .any y.2

/-- Whether the keys of a dict a place reaches keep what their place declares (`keep`): none where the
    place is of a dict and the value is not one the analysis follows. -/
def keysOK (keep : Abs → Slot → Bool) (keys : Option Slot) (a : Abs) : Bool :=
  match keys, a with
  | some ks, .dict _ ka _ => keep ka ks
  | some _, _ => false
  | none, _ => true

/-- Whether every value keeps what a place declares, as the interpreter holds a value to it
    (`ranges::keeps`): the range of its numbers and dates, the count of a list, and with `reached`
    the fields of the records of a module it reaches and of the values of an enum it holds (each
    value it can be; any field of a value the analysis does not know). -/
def keepsD : Nat → Abs → Slot → Bool → Bool
  | 0, _, _, _ => false
  | k + 1, a, .mk list count range unit01 record fields values keys _, reached =>
    match stripOpt a with
    | .never => true
    | a =>
      -- the keys of a dict where a parameter reaches them, each to the whole of its type
      (!reached || keysOK (fun a s => keepsD k a s true) keys a) &&
      match elemAbs list count a with
      | none => false
      | some .never => true
      | some elem =>
        rangeOK (rangeOf range unit01 reached) elem && strOK list count elem &&
          (!(reached && record) || fields.all fun (f, fs) => keepsD k (elem.field f) fs true) &&
          (!reached || variantsKeep (fun a s => keepsD k a s true) elem values)

def keeps (a : Abs) (s : Slot) (reached : Bool) : Bool := keepsD KEEPS a s reached

/-- Whether every number of a value keeps the rule of int64 (`ranges::shown` for `overflow`). -/
def okNum : Abs → Bool
  | .num i => i.fits
  | .date i => i.fits
  | .opt _ x => okNum x
  | .any => false
  | _ => true

/-- Whether every value of an enum the abstract value holds is one of `vs` (`ranges::shown` for an
    input of an enum held to a range). -/
def enumWithin (vs : List String) : Abs → Bool
  | .never => true
  | .set (some s) => s.all (vs.contains ·)
  | _ => false

/-- Whether the values a place is given keep what it takes, for every input (`ranges::shown`). -/
def Place.shown (p : Place) : Bool :=
  match p.what with
  | .ranged r => (p.value.present.iv.map fun i => i.within r.min r.max).getD false ||
      (match p.value.present with | .never => true | _ => false)
  | .param s => keeps p.value s true
  | .slot s => keeps p.value s false
  | .isqrt => (match p.value with | .never => true | _ => false) ||
      (p.value.iv.map fun i => i.lo.any fun l => l.num ≥ 0).getD false
  | .overflow => okNum p.value
  | .zero => (match p.value with | .never => true | _ => false) ||
      (p.value.iv.map fun i => !i.hasZero).getD false
  | .pending => false
  | .across holds => holds
  | .oneOf vs => enumWithin vs p.value.present

def Place.pending (p : Place) : Bool := match p.what with | .pending => true | _ => false

/-- The strength of a relation: a stronger one says more. -/
def Rel.strength : Rel → Nat
  | .le => 1
  | .lt => 2
  | .eq => 3

def Rel.compose : Rel → Rel → Rel
  | .eq, r => r
  | r, .eq => r
  | .lt, _ => .lt
  | _, .lt => .lt
  | _, _ => .le

/-- The relations a record's fields keep, as edges between names. -/
def recordEdges (A : ACtx) (s : Sym) : List (Term × Rel × Term) :=
  match s.2.reverse with
  | [] => []
  | _ :: restRev =>
    let base : Sym := (s.1, restRev.reverse)
    match A.symAbs base with
    | some (.record _ rels) => rels.map fun (x, r, y) => (.name (base.1, base.2 ++ [x]), r, .name (base.1, base.2 ++ [y]))
    | _ => []

def bestGet (best : List (Term × Rel)) (s : Term) : Option Rel := (best.find? (·.1 == s)).map (·.2)

def bestPut (best : List (Term × Rel)) (s : Term) (r : Rel) : List (Term × Rel) :=
  if best.any (·.1 == s) then best.map fun (k, v) => if k == s then (k, r) else (k, v) else best ++ [(s, r)]

/-- One pass over the edges, each forward and (for `=`) back: the strongest relation from the
    first name to each, and whether it changed. -/
def relPass (edges : List (Term × Rel × Term)) (best : List (Term × Rel)) : List (Term × Rel) × Bool :=
  edges.foldl (fun (acc : List (Term × Rel) × Bool) (x, r, y) =>
    [(x, y, r), (y, x, r)].foldl (fun (acc : List (Term × Rel) × Bool) (src, dst, rel) =>
      if src == y && rel != .eq then acc
      else match bestGet acc.1 src with
        | none => acc
        | some cur =>
          let next := cur.compose rel
          let better := match bestGet acc.1 dst with
            | none => true
            | some old => next.strength > old.strength
          if better then (bestPut acc.1 dst next, true) else acc) acc) (best, false)

def relIterate (edges : List (Term × Rel × Term)) : Nat → List (Term × Rel) → List (Term × Rel)
  | 0, best => best
  | n + 1, best =>
    let (next, changed) := relPass edges best
    if changed then relIterate edges n next else next

/-- How the values of two expressions are known to compare: what a condition in force says of the
    names they are (and of the sums of lists, kaname's DESIGN 32), or the fields of one record,
    followed through each other. -/
def relation (A : ACtx) (a b : Expr) : Option Rel :=
  match sym a, sym b with
  | some sa, some sb =>
    let edges := A.facts ++ recordEdges A sa ++ recordEdges A sb
    let best := relIterate edges (edges.length + 1) [(.name sa, .eq)]
    if sa == sb then some .eq else bestGet best (.name sb)
  | _, _ => none

/-- The id of an expression; a `let` is no expression of its own. -/
def Expr.id? : Expr → Option Nat
  | .num id .. | .date id .. | .str id .. | .bool id .. | .none id | .enum id .. | .var id ..
  | .field id .. | .neg id .. | .not id .. | .inUnit id .. | .arith id .. | .ddiff id .. | .cmp id ..
  | .and id .. | .or id .. | .ite id .. | .mtch id .. | .block id .. | .call id .. | .rule id ..
  | .dateCall id .. | .transfer id .. | .record id .. | .list id .. | .walk id .. | .reduce id ..
  | .minmax id .. | .modOp id .. | .isqrt id .. | .allocate id .. | .round id .. | .count1 id ..
  | .startsWith id .. | .concat id .. | .text id .. | .bad id .. => some id
  | .dict id .. | .prim id .. | .walkK id .. | .toDict id .. | .mergeWith id .. => some id
  | .lam id | .callV id .. => some id
  | .letIn .. => Option.none

/-- The value of a number or a date literal, in the abstract. -/
def litAbs (v : Rat) : Abs := .num (Iv.exact v)

/-- The values of an enum and the bools a match arm's patterns name. -/
def armValues (ps : List Pat) : List String :=
  ps.filterMap fun p => match p with
    | .bool b => some (if b then "true" else "false")
    | .enum (some v) => some v
    | .value (some v) _ => some v
    | _ => none

/-- The values of an enum that has fields an arm's patterns name. -/
def valueNames (ps : List Pat) : List String :=
  ps.filterMap fun p => match p with
    | .value (some v) _ => some v
    | _ => none

/-- The values of an enum with no fields and the bools an arm's patterns name. -/
def setNames (ps : List Pat) : List String :=
  ps.filterMap fun p => match p with
    | .bool b => some (if b then "true" else "false")
    | .enum (some v) => some v
    | _ => none

def EXACT : Nat := 16

-- ── The analysis ────────────────────────────────────────────────────────────


def Slot.range? : Slot → Option Range
  | .mk _ _ r _ _ _ _ _ _ => r

def Slot.count? : Slot → Option Range
  | .mk _ c _ _ _ _ _ _ _ => c

/-- A value the analysis knows nothing of, where the evaluation can fail: a place that is never
    shown. The check of types lets no module reach one (the operands of `-` and of arithmetic are
    numbers). -/
def unknown (o : Out) (e : Nat) : Out := o.place e "unknown" .range .pending .any

/-- Out of steps: anything, and a place that is never shown, so that a module the analysis did not
    finish is not one whose places are all shown. -/
def outOfFuel (o : Out) : Out := o.place 0 "fuel" .range .pending .any

/-- What a condition narrows (`ranges::narrow`), as its analysis found it: the name of a bool, a
    comparison with the values its two sides came to when it was analysed (what `ranges.rs` reads
    back as `last`), `not`, `and`, `or`. -/
inductive Nar where
  | none
  | sym (s : Sym)
  | cmp (op : CmpOp) (a b : Expr) (ca cb : Option Conv) (sa sb : Option Rat) (va vb : Abs)
  | not (x : Nar)
  | and (a b : Nar)
  | or (a b : Nar)
  deriving Inhabited

/-- `count(xs) = 0` narrows the count of the list `xs`: the guard of an empty list. -/
def narrowCount (A : ACtx) (x : Expr) (vy : Abs) (op : CmpOp) : ACtx :=
  match counted x with
  | some list => match A.symAbs list, vy with
    | some (.list c el), .num other => match narrowIv c op other (some 1) with
      | some n => A.narrowTo list (.list n el)
      | none => A
    -- the count of the pairs of a dict and of the members of a set (DESIGN 24.5)
    | some (.dict c k el), .num other => match narrowIv c op other (some 1) with
      | some n => A.narrowTo list (.dict n k el)
      | none => A
    | some (.members c el), .num other => match narrowIv c op other (some 1) with
      | some n => A.narrowTo list (.members n el)
      | none => A
    -- the count of the code points of a string (DESIGN 25.6)
    | some (.str c), .num other => match narrowIv c op other (some 1) with
      | some n => A.narrowTo list (.str n)
      | none => A
    | _, _ => A
  | none => A

def scaleBy (c : Option Conv) (a : Abs) : Abs :=
  match c with
  | some c => a.scaled c.f c.shift
  | none => a

/-- The values of one side where the comparison with the other holds: numbers and dates by their
    intervals, a value of an enum or a bool where it is or is not one value. -/
def narrowedOf (me other : Abs) (op : CmpOp) (step : Option Rat) : Option Abs :=
  match me, other with
  | .num x, .num y => (narrowIv x op y step).map .num
  | .date x, .date y => (narrowIv x op y (some 1)).map .date
  | .set x, .set (some [v]) => match op with
    | .eq => some (.set (some (match x with | some s => s.filter (· == v) | none => [v])))
    | .ne => x.map fun s => .set (some (s.filter (· != v)))
    | _ => none
  | _, _ => none

/-- One side of a comparison narrowed by the other: a name only, and one compared in its own unit. -/
def narrowSide (A : ACtx) (s : Option Sym) (conv : Option Conv) (me other : Abs) (op : CmpOp) (step : Option Rat) : ACtx :=
  match s with
  | none => A
  | some s =>
    if conv.isSome then A
    else match narrowedOf me other op step with
      | some n => A.narrowTo s n
      | none => A

/-- What a comparison that holds says of how two names (or sums) compare, kept as a fact. -/
def factOf (op : CmpOp) (p q : Term) : Option (Term × Rel × Term) :=
  match op with
  | .lt => some (p, .lt, q)
  | .le => some (p, .le, q)
  | .gt => some (q, .lt, p)
  | .ge => some (q, .le, p)
  | .eq => some (p, .eq, q)
  | .ne => none

/-- A fact between two names is kept where both are in their own unit and both are numbers or both
    dates: what `relation` reads, and what the comparison that holds says of them. -/
def addFact (A : ACtx) (op : CmpOp) (syA syB : Option Term) (ca cb : Option Conv) (xa xb : Abs) : ACtx :=
  let comparable := ca.isNone && cb.isNone && match xa, xb with
    | .num _, .num _ => true
    | .date _, .date _ => true
    | _, _ => false
  match syA, syB, comparable with
  | some p, some q, true => match factOf op p q with
    | some f => { A with facts := A.facts ++ [f] }
    | none => A
  | _, _, _ => A

/-- `count(xs) = 0` on either side, read where both sides are in their own unit. -/
def narrowCounts (A : ACtx) (op : CmpOp) (a b : Expr) (ca cb : Option Conv) (va vb : Abs) : ACtx :=
  if ca.isNone && cb.isNone then narrowCount (narrowCount A a vb op) b va op.flip else A

/-- A comparison, where it holds (`ranges::narrow_cmp`): the fact between the names or sums the
    sides are, and each side that is a name narrowed by the other. -/
def narrowCmp (A : ACtx) (op : CmpOp) (a b : Expr) (ca cb : Option Conv) (sa sb : Option Rat) (va vb : Abs) : ACtx :=
  let A := narrowCounts A op a b ca cb va vb
  match factTerm a, factTerm b with
  | none, none => A
  | fa, fb =>
    let xa := scaleBy ca va
    let xb := scaleBy cb vb
    let A := addFact A op fa fb ca cb xa xb
    let A := narrowSide A (sym a) ca xa xb op sa
    narrowSide A (sym b) cb xb xa op.flip sb

def Nar.apply : Nar → Bool → ACtx → ACtx
  | .none, _, A => A
  | .sym s, truth, A => A.narrowTo s (boolAbs truth)
  | .cmp op a b ca cb sa sb va vb, truth, A => narrowCmp A (if truth then op else op.negate) a b ca cb sa sb va vb
  | .not x, truth, A => x.apply (!truth) A
  | .and x y, truth, A => if truth then y.apply true (x.apply true A) else A
  | .or x y, truth, A => if !truth then y.apply false (x.apply false A) else A

/-- `m - n` where a condition or a record keeps the order of `n` and `m` (DESIGN 5.3). -/
def subRel (A : ACtx) (a b : Expr) (step : Option Rat) (iv : Iv) : Iv :=
  let step := step.filter (fun s => s.num > 0)
  let iv := match relation A b a with
    | some .eq => Iv.exact 0
    | some .le => { iv with lo := maxLo iv.lo (some 0) }
    | some .lt => { iv with lo := maxLo iv.lo (some (step.getD 0)) }
    | none => iv
  match relation A a b with
  | some .le => { iv with hi := minHi iv.hi (some 0) }
  | some .lt => { iv with hi := minHi iv.hi (some ((0 : Rat) - step.getD 0)) }
  | _ => iv

/-- What `some x` names: the value where it is there. -/
def innerOf : Abs → Abs
  | .opt _ v => v
  | .any => .any
  | v => v

def Abs.isAny : Abs → Bool
  | .any => true
  | _ => false

/-- What one pattern of an arm narrows the value to, and whether it can take a value. -/
def armStep (sy : Option Sym) (sv : Abs) (acc : ACtx × Bool) (p : Pat) : ACtx × Bool :=
  match p with
  | .none =>
    if sv.mayBeNone || sv.isAny then
      ((match sy with | some s => acc.1.narrowTo s Abs.none' | none => acc.1), true)
    else acc
  | .some x =>
    match innerOf sv with
    | .never => acc
    | inner => (acc.1.bindNew x inner, true)
  | _ => acc

/-- The values an arm names that the value can be. -/
def canValues (sv : Abs) (named : List String) : List String :=
  match sv with
  | .set (some vs) => named.filter (vs.contains ·)
  | _ => named

/-- `else`. -/
def Pat.isRest : Pat → Bool
  | .rest => true
  | _ => false

/-- `none`. -/
def Pat.isNone : Pat → Bool
  | .none => true
  | _ => false

/-- `some x`. -/
def Pat.isSome : Pat → Bool
  | .some _ => true
  | _ => false

/-- Whether a value can be anything at all. -/
def notNever : Abs → Bool
  | .never => false
  | _ => true

/-- Whether a value is a number or a date. -/
def isNumDate : Abs → Bool
  | .num _ | .date _ => true
  | _ => false

/-- What `else` takes of a value, the arms above it having the patterns `prev` (`ranges::rest_of`):
    none where nothing is left (the arm is never reached); with what the value is narrowed to there,
    where that is said (the values of an enum, with fields or not, and of a bool left). -/
def restOf (prev : List Pat) : Abs → Option (Option Abs)
  | .never => none
  | .variants vs =>
    let left := vs.filter fun x => !(valueNames prev).contains x.1
    if left.isEmpty then none else some (some (.variants left))
  | .set (some vs) =>
    let left := vs.filter fun x => !(setNames prev).contains x
    if left.isEmpty then none else some (some (.set (some left)))
  | .opt n x =>
    let noneLeft := n && !prev.any Pat.isNone
    let someLeft := notNever x && !prev.any Pat.isSome
    if noneLeft || someLeft then some none else none
  | _ => some none

/-- What one literal of an arm takes of the value: the one number or day it is, where the value can
    be it (none where it cannot); the value as it is, where it is no number or date. -/
def litTake (sv : Abs) (l : Val) : Option Abs :=
  match sv, l with
  | .num i, .num r => if i.has r then some (.num (Iv.exact r)) else none
  | .date i, .date d => if i.has d then some (.date (Iv.ints d d)) else none
  | .never, _ => none
  | _, _ => some sv

/-- One literal of an arm, with what the literals before it came to: whether one can take the
    value, the numbers and days they take joined, and whether one takes a value that is no number or
    date. -/
def litStep (sv : Abs) (acc : Bool × Option Abs × Bool) (p : Pat) : Bool × Option Abs × Bool :=
  match p with
  | .lit l => match litTake sv l with
    | some a =>
      if isNumDate a then (true, some (match acc.2.1 with | some b => b.join a | none => a), acc.2.2)
      else (true, acc.2.1, true)
    | none => acc
  | _ => acc

/-- The literals of an arm: whether one can take the value, and what they narrow it to (the numbers
    and days they take, joined; nothing where one takes a value that is no number or date). -/
def litsOf (sv : Abs) (ps : List Pat) : Bool × Option Abs :=
  let (reached, lits, other) := ps.foldl (litStep sv) (false, none, false)
  (reached, if other then none else lits)

/-- The name an arm gives a value of an enum that has fields, alone in its arm (`placed p`). -/
def boundName : List Pat → Option String
  | [.value _ (some x)] => some x
  | _ => none

/-- A name narrowed to `a`, where the value split is one. -/
def ACtx.narrowOpt (A : ACtx) : Option Sym → Abs → ACtx
  | some s, a => A.narrowTo s a
  | none, _ => A

/-- `else`: the values the arms above (their patterns `prev`) leave. -/
def restScope (sy : Option Sym) (sv : Abs) (prev ps : List Pat) (acc : ACtx × Bool) : ACtx × Bool :=
  if ps.any Pat.isRest then
    match restOf prev sv with
    | none => acc
    | some none => (acc.1, true)
    | some (some a) => (acc.1.narrowOpt sy a, true)
  else acc

/-- Literals: the numbers and days they take, joined. -/
def litScope (sy : Option Sym) (sv : Abs) (ps : List Pat) (acc : ACtx × Bool) : ACtx × Bool :=
  let lr := litsOf sv ps
  ((match lr.2 with
    | some a => acc.1.narrowOpt sy a
    | none => acc.1), acc.2 || lr.1)

/-- Values of an enum and bools the arm names: what the value can be of them, each with its fields,
    and the name the arm gives a value of an enum that has fields. -/
def nameScope (sy : Option Sym) (sv : Abs) (ps : List Pat) (acc : ACtx × Bool) : ACtx × Bool :=
  let named := armValues ps
  if named.isEmpty then acc
  else
    match sv with
    -- the values of an enum that has fields the arm names, each with its fields
    | .variants vs =>
      let can := vs.filter fun x => named.contains x.1
      if can.isEmpty then acc
      else
        let A := acc.1.narrowOpt sy (.variants can)
        ((match boundName ps with | some x => A.bindNew x (.variants can) | none => A), true)
    | _ =>
      if (valueNames ps).isEmpty then
        let can := canValues sv named
        if can.isEmpty then acc
        else (acc.1.narrowOpt sy (.set (some (setOf can))), true)
      else ((match boundName ps with | some x => acc.1.bindNew x .any | none => acc.1), true)

/-- The scope of a `match` arm, the arms above it having the patterns `prev` (DESIGN 23.7): what its
    patterns narrow the value to and the names they give it, and whether anything reaches it. -/
def armScope (A : ACtx) (sy : Option Sym) (sv : Abs) (prev ps : List Pat) : ACtx × Bool :=
  nameScope sy sv ps (litScope sy sv ps (restScope sy sv prev ps (ps.foldl (armStep sy sv) (A, false))))

def exprId (x : Expr) : Nat := x.id?.getD 0

/-- One argument of a call of `fn`: read in its parameter's unit and kept as a place, the name it
    is narrowed to the parameter's range from here on. -/
def callStep (fn : Fn) (acc : Out × ACtx) (arg : (Nat × Expr × Option Conv) × Abs) : Out × ACtx :=
  let (o, A) := acc
  let ((k, x, c), v) := arg
  let (v, o) := convA o c v
  match fn.params[k]? with
  | none => (o, A)
  | some prm =>
    let o := o.place (exprId x) ("param:" ++ prm.name) .range (.param prm.slot) v
    let A := if prm.scalar then (match prm.slot.range? with
        | some r => assume A x r c
        | none => A) else A
    (o, A)

/-- A call of a function of the module, from the values of its arguments: each read in its
    parameter's unit and kept as a place, the name it is narrowed to the parameter's range from here
    on; what the function answers, or what its result declares where it is in the ring. -/
def callAbs (A : ACtx) (o : Out) (f : Nat) (args : List (Nat × Expr × Option Conv)) (vals : List Abs) : Abs × Out × ACtx :=
  match A.M.fns[f]? with
  | none => (.any, o, A)
  | some fn =>
    let (o, A) := (args.zip vals).foldl (callStep fn) (o, A)
    let o := { o with calls := o.calls ++ [f] }
    -- one that takes functions takes what it declares: its own analysis answered for functions it
    -- does not see (kaname's DESIGN 27.4)
    let result : Option Abs := if A.ring.contains f || fn.takesFn then none else (A.results[f]?).join
    (result.getD fn.declared, o, A)

/-- What an element of a list can be (anything where the value is not one list). -/
def elemOfList : Abs → Abs
  | .list _ el => el
  | _ => .any

/-- The places of the fields of the elements of a list a rule walks. -/
def elemStep (x : Expr) (inp : RuleInput) (elem : Abs) (o : Out) (col : String × Option Range) : Out :=
  match col.2 with
  | some rg => o.place (exprId x) ("elem:" ++ inp.name ++ "." ++ col.1) .rule (.ranged rg) (elem.field col.1)
  | none => o

/-- The place of an input of a rule of an ordered enum held to a range over its values, with the
    values given to it. The name given is not narrowed after the call (`ranges::call_rule`). -/
def enumPlace (x : Expr) (inp : RuleInput) (v : Abs) (o : Out) : Out :=
  match inp.values with
  | some vs => o.place (exprId x) ("input:" ++ inp.name) .rule (.oneOf vs) v
  | none => o

/-- One argument of a call of a rule. -/
def ruleStep (rd : RuleD) (acc : Out × ACtx) (arg : (Nat × Expr × Option Conv) × Abs) : Out × ACtx :=
  let (o, A) := acc
  let ((k, x, c), v) := arg
  let (v, o) := convA o c v
  match rd.inputs[k]? with
  | none => (o, A)
  | some inp =>
    match inp.elements with
    | none =>
      match inp.range with
      | some rg =>
        let o := o.place (exprId x) ("input:" ++ inp.name) .rule (.ranged rg) v
        (o, if inp.opt then A else assume A x rg c)
      | none =>
        if inp.dateRanged then (o.place (exprId x) ("rdate:" ++ inp.name) .rule .pending v, A) else (enumPlace x inp v o, A)
    | some cols => (cols.foldl (elemStep x inp (elemOfList v)) o, A)

/-- One more input a call gives, among the others in the order of their names (`ranges.rs` keeps
    them in a `BTreeMap`, which orders strings as Lean does: by their characters). -/
def insertByName (x : String × Expr) : List (String × Expr) → List (String × Expr)
  | [] => [x]
  | y :: ys => if x.1 < y.1 then x :: y :: ys else y :: insertByName x ys

/-- The relations between the inputs a call gives, each pair once in the order of their names: how
    the first's value is known to compare to the second's, else the second's to the first's. -/
def relPairs (A : ACtx) : List (String × Expr) → List (String × Rel × String)
  | [] => []
  | (na, xa) :: rest =>
    (rest.filterMap fun (nb, xb) =>
      match relation A xa xb with
      | some r => some (na, r, nb)
      | none => (relation A xb xa).map fun r => (nb, r, na)) ++ relPairs A rest

/-- The inputs a call of a rule gives, by their names in the rule (the lists it walks too), in the
    order of the names, each with the expression given. -/
def givenByName (rd : RuleD) (args : List (Nat × Expr × Option Conv)) : List (String × Expr) :=
  args.foldl (fun acc (a : Nat × Expr × Option Conv) => match rd.inputs[a.1]? with
    | some inp => insertByName (inp.name, a.2.1) acc
    | none => acc) []

/-- What a call of a rule keeps between the inputs it gives: the relations the conditions in force
    and the records of `running_totals` say of the values given (`ranges::call_rule`'s `rels`; what
    X2 reads first: kaname's DESIGN 14.3). -/
def callRels (A : ACtx) (rd : RuleD) (args : List (Nat × Expr × Option Conv)) : List (String × Rel × String) :=
  relPairs A (givenByName rd args)

/-- A call of a rule, from the values of its arguments (`ranges::call_rule`). -/
def ruleAbs (A : ACtx) (o : Out) (id : Nat) (r : Nat) (args : List (Nat × Expr × Option Conv)) (vals : List Abs) : Abs × Out × ACtx :=
  match A.M.rules[r]? with
  | none => (.any, o, A)
  | some rd =>
    let (o, A) := (args.zip vals).foldl (ruleStep rd) (o, A)
    let o := rd.pres.foldl (fun o said => o.place id ("pre:" ++ said) .rule .pending .any) o
    (.record rd.outputs [], (if A.lam then o.keepL else o.keep) id (callRels A rd args), A)

/-- One argument of a call of a date: the date it reads, where it is the one the date's days are
    counted from. -/
def dateStep (fd : DatesD) (reads : List String) (acc : Out × ACtx × Option Abs) (arg : (Nat × Expr × Option Conv) × Abs) :
    Out × ACtx × Option Abs :=
  let (o, A, given) := acc
  let ((k, x, c), v) := arg
  let (v, o) := convA o c v
  match fd.inputs[k]? with
  | none => (o, A, given)
  | some inp =>
    let rg : Range := ⟨some (inp.min : Rat), some (inp.max : Rat)⟩
    if inp.isDate then
      let o := o.place (exprId x) ("day:" ++ inp.name) .date .pending v
      let given := if reads.contains inp.name then some v else given
      (o, assume A x rg c, given)
    else
      let o := o.place (exprId x) ("int:" ++ inp.name) .date (.ranged rg) v
      (o, assume A x rg c, given)

/-- The first and last of the days of a date of a file. -/
def hullOf (dd : Option DateD) : Iv :=
  match dd.bind (·.hull) with
  | some (lo, hi) => Iv.ints lo hi
  | none => { Iv.top with den := some 1 }

/-- The days a date of a file can be: within the first and last of its days, and the days from the
    date it reads. -/
def dateIv (dd : Option DateD) (given : Option Abs) : Iv :=
  match given, dd.bind (·.days) with
  | some g, some (fewest, most) => match g.iv with
    | some gi => ((hullOf dd).meet (gi.plus (Iv.ints fewest most))).getD (hullOf dd)
    | none => hullOf dd
  | _, _ => hullOf dd

/-- A date of a dates file, from the values of its arguments (`ranges::call_date`). -/
def dateAbs (A : ACtx) (o : Out) (file : Nat) (date : String) (args : List (Nat × Expr × Option Conv)) (vals : List Abs) : Abs × Out × ACtx :=
  match A.M.dates[file]? with
  | none => (.any, o, A)
  | some fd =>
    let dd := fd.dates.find? (·.name == date)
    let reads := (dd.map (·.reads)).getD []
    let (o, A, given) := (args.zip vals).foldl (dateStep fd reads) (o, A, none)
    (.date (dateIv dd given), o, A)

/-- One argument of a transfer. -/
def transferStep (acc : List (String × Abs) × Out) (arg : (String × Expr × Option Conv × Nat × Bool) × Abs) :
    List (String × Abs) × Out :=
  let (out, o) := acc
  let ((p, x, c, _, amount), v) := arg
  let (v, o) := convA o c v
  let o := if amount then o.place (exprId x) ("amount:" ++ p) .range .pending v else o
  (out ++ [(p, v)], o)

/-- A transfer built, from the values of its arguments: an amount is a place ritsu-cross decides. -/
def transferAbs (o : Out) (args : List (String × Expr × Option Conv × Nat × Bool)) (vals : List Abs) : Abs × Out :=
  let (out, o) := (args.zip vals).foldl transferStep ([], o)
  (.transfer out, o)

/-- One field of a record as written, read in its place's unit. -/
def initStep (acc : List (Nat × Abs × Expr × Option Conv) × Out) (arg : (Nat × Expr × Option Conv × Nat) × Abs) :
    List (Nat × Abs × Expr × Option Conv) × Out :=
  let (given, o) := acc
  let ((k, x, c, _), v) := arg
  let (v, o) := convA o c v
  (given ++ [(k, v, x, c)], o)

/-- A number or a date cut to the range a field of a module's record declares. -/
def cutTo (a : Abs) (r : Option Range) (module : Bool) : Abs :=
  match a, r, module with
  | .num i, some r, true => .num (i.cut r.min r.max)
  | .date i, some r, true => .date (i.cut r.min r.max)
  | a, _, _ => a

/-- What a field of a record is: the value written last for it (where none is, the field of the
    value it is made from, or none), cut to what it declares where the record is a module's. -/
def fieldAbs (module : Bool) (slot : Slot) (given : List (Nat × Abs × Expr × Option Conv)) (src : Option (Abs × Nat))
    (f : String) (k : Nat) : Abs :=
  match given.reverse.find? (·.1 == k) with
  | some (_, a, _, _) => cutTo a slot.range? module
  | none => match src with
    | some (fa, _) => cutTo (fa.field f) slot.range? module
    | none => Abs.none'

/-- One field of a record in the order declared: its value as written (the last written), or the
    field of the value it is made from (its place at that value: `..x`), held to what it declares
    where the record is a module's. -/
def fieldStep (module : Bool) (given : List (Nat × Abs × Expr × Option Conv)) (src : Option (Abs × Nat))
    (acc : List (String × Abs) × Out × ACtx) (fk : (String × Slot × Bool) × Nat) : List (String × Abs) × Out × ACtx :=
  let (out, o, A) := acc
  let ((f, slot, scalar), k) := fk
  let held := module && (slot.range?.isSome || slot.count?.isSome)
  let (o, A) := match given.reverse.find? (·.1 == k) with
    | some (_, a, x, c) =>
      if held then
        (o.place (exprId x) ("field:" ++ f) .range (.slot slot) a,
          if scalar then (match slot.range? with
            | some r => assume A x r c
            | none => A) else A)
      else (o, A)
    | none => match src with
      | some (fa, b) => (if held then o.place b ("field:" ++ f) .range (.slot slot) (fa.field f) else o, A)
      | none => (o, A)
  (out ++ [(f, fieldAbs module slot given src f k)], o, A)

/-- A record built, or a value of an enum (DESIGN 23.7), from the values of its fields as written
    and the value it is made from (`ranges::record`). -/
def recordAbs (A : ACtx) (o : Out) (module : Bool) (fields : List (String × Slot × Bool))
    (inits : List (Nat × Expr × Option Conv × Nat)) (vals : List Abs) (tag : Option String)
    (src : Option (Abs × Nat)) : Abs × Out × ACtx :=
  let (given, o) := (inits.zip vals).foldl initStep ([], o)
  let (out, o, A) := fields.zipIdx.foldl (fieldStep module given src) ([], o, A)
  ((match tag with
    | some t => .variants [(t, out)]
    | none => .record out []), o, A)

/-- The list a builtin walks: its count, and an element (a count below 0 for one that never comes). -/
def walked : Abs → Iv × Abs
  | .never => (Iv.ints (-1) (-1), .never)
  | .list c el => (c, el)
  | .opt _ (.list c el) => (c, el)
  | _ => (Iv.mk' (some 0) none (some 1), .any)

def minMaxIv (isMin : Bool) (x y : Iv) : Iv :=
  let (lo, hi) := if isMin then (minLo x.lo y.lo, minHi x.hi y.hi) else (maxLo x.lo y.lo, maxHi x.hi y.hi)
  Iv.mk' lo hi (x.hull y).den

/-- What the analysis of an expression comes to: its values, what is kept, the scope from here on
    (narrowed by the calls that answered), and what it narrows as a condition. -/
structure AR where
  abs : Abs
  out : Out
  ctx : ACtx
  nar : Nar

def AR.of (a : Abs) (o : Out) (A : ACtx) : AR := ⟨a, o, A, .none⟩

/-- Whether a builtin walks a list with a condition. -/
def Walk.isCondition : Walk → Bool
  | .count | .any | .all | .filter | .first => true
  | _ => false

/-- No element is ever walked: a count of at most 0, or elements that never come. -/
def noneAtAll (count : Iv) (elem : Abs) : Bool :=
  count.hi.any (fun h => h.num ≤ 0) || (match elem with | .never => true | _ => false)

/-- What a walk with a condition answers, from whether the condition can hold (`t`) and fail (`f`)
    and what an element is where it holds (`held`). -/
def condResult (w : Walk) (count : Iv) (elem : Abs) (t f : Bool) (held : Abs) : Abs :=
  let none' := noneAtAll count elem
  let mayBeEmpty := count.lo.all fun l => l.num ≤ 0
  match w with
  | .count =>
    if none' then .num (Iv.ints 0 0)
    else .num (if t && !f then count else if !t then Iv.ints 0 0 else Iv.mk' (some 0) count.hi (some 1))
  | .any => bools (!none' && t) (none' || mayBeEmpty || (!none' && f))
  | .all => bools (none' || mayBeEmpty || (!none' && t)) (!none' && f)
  | .filter => .list (Iv.mk' (some 0) count.hi (some 1)) held
  | _ => .opt true (stripOpt held)

/-- What a walk with a function answers, from what the function answers (`v`), and the places of
    the sums. -/
def lamResult (o : Out) (id : Nat) (w : Walk) (count : Iv) (elem : Abs) (v : Abs) (step : Option Rat) : Abs × Out :=
  let none' := noneAtAll count elem
  let mayBeEmpty := count.lo.all fun l => l.num ≤ 0
  let wv := v.iv.getD Iv.top
  let dn := stepDen step
  let every := sums (Iv.mk' (some 0) count.hi (some 1)) wv dn
  match w with
  | .map => (.list count (if none' then .never else v), o)
  | .maxOf | .minOf => (.opt mayBeEmpty (if none' then .never else v), o)
  | .sum =>
    let o := o.place id "overflow" .overflow .overflow (.num every)
    let (total, o) := heldA o id (.num (sums count wv dn))
    (if none' then .num (Iv.ints 0 0) else total, o)
  | _ =>
    let total := sums count wv dn
    let beforeHi := count.hi.map fun h => if h - 1 < 0 then (0 : Rat) else h - 1
    let before := sums (Iv.mk' (some 0) beforeHi (some 1)) wv dn
    let upto := sums (Iv.mk' (some 1) count.hi (some 1)) wv dn
    let o := o.place id "overflow" .overflow .overflow (.num every)
    let rels : List (String × Rel × String) :=
      if wv.lo.any (fun l => l.num ≥ 0) then [("before", .le, "upto"), ("upto", .le, "total")] else []
    let r : Abs := .record [("item", elem), ("before", .num before), ("upto", .num upto), ("total", .num total)] rels
    (.list count (if none' then .never else r), o)

/-- How many times `reduce`'s function is laid over the running value where the list's count has a
    known end of at most `EXACT`: as many times as the list can have elements. -/
def exactRounds (count : Iv) : Option Nat :=
  match count.hi with
  | some h => if h.num ≤ (EXACT : Int) then some (max h.num 0).toNat else Option.none
  | Option.none => Option.none

-- ── Dicts, sets and the builtins of lists (DESIGN 24.5) ────────────────────────

/-- What is not below 0. -/
def nonNeg (v : Rat) : Rat := if v.num < 0 then 0 else v

/-- A count held to 0 and above, and to 2⁶³ − 1 at most (`ranges::count_iv`). -/
def capCount (v : Rat) : Rat := if v.num < 0 then 0 else if (i64max : Rat) < v then (i64max : Rat) else v

/-- A count from `lo` to `hi`, each held to 0 and above and to 2⁶³ − 1 at most; no upper end where
    none is said. -/
def countIv (lo hi : End) : Iv :=
  let l := (lo.map capCount).getD 0
  let h := (hi.map capCount).map fun h => if h < l then l else h
  Iv.mk' (some l) h (some 1)

/-- From one to the count, where it can be one or more (`ranges::one_up`). -/
def oneUp (c : Iv) : Iv := countIv (c.lo.map fun l => if l.num > 1 then 1 else l) c.hi

/-- From none to the count (`ranges::up_to`). -/
def upTo (c : Iv) : Iv := Iv.mk' (some 0) c.hi (some 1)

/-- The count of two dicts or two sets put together: at least the more of their least, at most the
    sum of their most (`ranges::merged`). -/
def mergedC (a b : Iv) : Iv :=
  let lo := match a.lo, b.lo with
    | some x, some y => some (if y ≤ x then x else y)
    | x, y => x.or y
  let hi := match a.hi, b.hi with
    | some x, some y => some (x + y)
    | _, _ => none
  countIv lo hi

/-- Two counts added: lists one after another (`ranges::add_counts`). -/
def addCounts (a b : Iv) : Iv :=
  let lo := match a.lo, b.lo with | some x, some y => some (x + y) | _, _ => none
  let hi := match a.hi, b.hi with | some x, some y => some (x + y) | _, _ => none
  countIv lo hi

/-- A count times a count: the lists of a list one after another (`ranges::times_count`), the lower
    ends taken from 0. -/
def timesCount (outer inner : Iv) : Iv :=
  let lo := match outer.lo, inner.lo with | some x, some y => some (nonNeg x * nonNeg y) | _, _ => none
  let hi := match outer.hi, inner.hi with | some x, some y => some (x * y) | _, _ => none
  countIv lo hi

/-- A value that is none where `may` says or where it may be none itself, anything among them
    (`ranges::opt_of`). -/
def optOf (may : Bool) : Abs → Abs
  | .opt n x => .opt (may || n) x
  | .any => .any
  | x => .opt may x

/-- Any count: from 0, no upper end. -/
def anyCount : Iv := Iv.mk' (some 0) none (some 1)

/-- A list's count and elements; any count and anything for what is no list the analysis follows. -/
def listParts : Abs → Iv × Abs
  | .list c x => (c, x)
  | .opt _ x => listParts x
  | .never => (Iv.ints 0 0, .never)
  | _ => (anyCount, .any)

/-- A dict's count, keys and values. -/
def dictParts : Abs → Iv × Abs × Abs
  | .dict c k x => (c, k, x)
  | .opt _ x => dictParts x
  | .never => (Iv.ints 0 0, .never, .never)
  | _ => (anyCount, .any, .any)

/-- A set's count and members. -/
def membersParts : Abs → Iv × Abs
  | .members c x => (c, x)
  | .opt _ x => membersParts x
  | .never => (Iv.ints 0 0, .never)
  | _ => (anyCount, .any)

/-- An entry of a dict: a record of its key and its value. -/
def entryAbs (k v : Abs) : Abs := .record [("key", k), ("value", v)] []

/-- One more where it is said (a count put one more in). -/
def plusOne (e : End) : End := e.map (· + 1)

/-- At least one (a count put one in). -/
def atLeastOne (e : End) : End := e.map fun l => if l.num < 1 then 1 else l

/-- The interval of a string's count of code points (`ranges::str_count`): from 0 for what is no
    string the analysis follows. -/
def strCount : Abs → Iv
  | .str c => c
  | .opt _ x => strCount x
  | .never => Iv.ints 0 0
  | _ => anyCount

/-- The count of the code points `text` writes (`ranges::text_count`, DESIGN 25.6): a whole number of
    int64 from 1 to 20, with the spelling of its unit after it; a date 10; a value of an enum and a
    bool its name; a string itself. -/
def textCount (fmt : TextFmt) (a : Abs) : Iv :=
  match fmt with
  | .rate => anyCount
  | .number => Iv.ints 1 20
  | .unit u => Iv.ints (1 + u.length) (20 + u.length)
  | .plain => match a with
    | .date _ => Iv.ints 10 10
    | .set (some s) => match (s.map String.length).min?, (s.map String.length).max? with
      | some lo, some hi => Iv.ints lo hi
      | _, _ => Iv.ints 0 0
    | .str c => c
    | .never => Iv.ints 0 0
    | _ => anyCount

/-- The count of the code points of strings put together, `sep` between two (`join`): the count
    times each and the gaps times `sep`, end by end (the lower ends held to 0 and above). -/
def joinedEnd (count each between : End) : End :=
  match count, each, between with
  | some c, some e, some b => some (c * e + (if c.num > 0 then c - 1 else 0) * b)
  | _, _, _ => none

/-- The builtins of a dict, a set and a list that take no function (`ranges.rs`'s `collection` and
    `scalar`), from what their arguments can be, each read in its place, the dict, the set or the list
    first: the place of `numbers`' count. -/
def primAbs (o : Out) (id : Nat) (op : Prim) (vals : List Abs) : Abs × Out :=
  let first := vals.headD .any
  let arg := fun (i : Nat) => vals.getD (i + 1) .any
  let ivA := fun (i : Nat) => (arg i).iv.getD Iv.top
  match op with
  -- a dict
  | .get => (optOf true (dictParts first).2.2, o)
  | .hasD | .hasS | .contains => (Abs.bools, o)
  | .put =>
    let (c, k, v) := dictParts first
    (.dict (countIv (atLeastOne c.lo) (plusOne c.hi)) (k.join (arg 0)) (v.join (arg 1)), o)
  | .removeD =>
    let (c, k, v) := dictParts first
    (.dict (countIv (c.lo.map (· - 1)) c.hi) k v, o)
  | .keys => let (c, k, _) := dictParts first; (.list c k, o)
  | .values => let (c, _, v) := dictParts first; (.list c v, o)
  | .entries => let (c, k, v) := dictParts first; (.list c (entryAbs k v), o)
  | .merge =>
    let (c, k, v) := dictParts first
    let (cb, kb, vb) := dictParts (arg 0)
    (.dict (mergedC c cb) (k.join kb) (v.join vb), o)
  -- a set
  | .add =>
    let (c, x) := membersParts first
    (.members (countIv (atLeastOne c.lo) (plusOne c.hi)) (x.join (arg 0)), o)
  | .removeS =>
    let (c, x) := membersParts first
    (.members (countIv (c.lo.map (· - 1)) c.hi) x, o)
  | .union =>
    let (c, x) := membersParts first
    let (cb, xb) := membersParts (arg 0)
    (.members (mergedC c cb) (x.join xb), o)
  | .intersect =>
    let (c, x) := membersParts first
    let (cb, _) := membersParts (arg 0)
    (.members (countIv (some 0) (minHi c.hi cb.hi)) x, o)
  | .difference =>
    let (c, x) := membersParts first
    let (cb, _) := membersParts (arg 0)
    let lo := match c.lo, cb.hi with
      | some l, some h => some (l - h)
      | _, _ => some 0
    (.members (countIv lo c.hi) x, o)
  | .toList => let (c, x) := membersParts first; (.list c x, o)
  -- a list
  | .first | .last =>
    let (c, el) := listParts first
    (optOf (decide (((c.lo.map (·.num)).getD 0) ≤ 0)) el, o)
  | .at => (optOf true (listParts first).2, o)
  | .append =>
    let (c, el) := listParts first
    (.list (countIv (plusOne c.lo) (plusOne c.hi)) (el.join (arg 0)), o)
  | .take =>
    let (c, el) := listParts first
    let n := ivA 0
    let lo := minLo c.lo ((n.lo.map nonNeg).or (some 0))
    (.list (countIv lo (minHi c.hi (n.hi.map nonNeg))) el, o)
  | .drop =>
    let (c, el) := listParts first
    let n := ivA 0
    let most := n.hi.map nonNeg
    let least := (n.lo.map nonNeg).getD 0
    let lo := match c.lo, most with
      | some l, some h => some (l - h)
      | _, _ => some 0
    (.list (countIv lo (c.hi.map (· - least))) el, o)
  | .slice =>
    let (c, el) := listParts first
    let (a, b) := (ivA 0, ivA 1)
    let start := (a.lo.map nonNeg).getD 0
    let most := b.hi.map fun h => nonNeg h - start
    (.list (countIv (some 0) (minHi c.hi most)) el, o)
  | .reverse | .sort => let (c, el) := listParts first; (.list c el, o)
  | .distinct => let (c, el) := listParts first; (.list (oneUp c) el, o)
  | .toSet => let (c, el) := listParts first; (.members (oneUp c) el, o)
  | .flatten =>
    let (c, el) := listParts first
    let (ci, ei) := listParts el
    (.list (timesCount c ci) ei, o)
  | .indexed =>
    let (c, el) := listParts first
    let index := Iv.mk' (some 0) (c.hi.map (· - 1)) (some 1)
    let index := if index.hi.any (·.num < 0) then Iv.ints 0 0 else index
    (.list c (.record [("index", .num index), ("item", el)] []), o)
  -- strings put together: their counts and the separators' (DESIGN 25.6)
  | .join =>
    let (c, el) := listParts first
    let (e, sep) := (strCount el, strCount (arg 0))
    let lo := joinedEnd (c.lo.map nonNeg) (e.lo.map nonNeg) (sep.lo.map nonNeg)
    let hi := joinedEnd (c.hi.map nonNeg) e.hi sep.hi
    (.str (countIv (lo.or (some 0)) hi), o)
  -- lists one after another
  | .concat =>
    let (count, elem) := vals.foldl (fun (acc : Iv × Abs) v =>
      let (c, el) := listParts v
      (addCounts acc.1 c, acc.2.join el)) (Iv.ints 0 0, .never)
    (.list count elem, o)
  -- the numbers from the least `a` to the greatest `b`: the count, b − a + 1 where it is above 0,
  -- held to int64
  | .numbers =>
    let (a, b) := (first.iv.getD Iv.top, (arg 0).iv.getD Iv.top)
    let raw := (b.minus a).plus (Iv.exact 1)
    let count : Iv := { raw with lo := raw.lo.map nonNeg }
    let o := o.place id "overflow" .overflow .overflow (.num count)
    let count := countIv count.lo count.hi
    if count.hi.any (·.num ≤ 0) then (.list (Iv.ints 0 0) .never, o)
    else (.list count (.num (Iv.mk' a.lo b.hi (some 1))), o)
  -- a string (`ranges.rs`'s `string`, DESIGN 25.6): what it can be is its count of code points
  | .chars => (.list (strCount first) (.str (Iv.ints 1 1)), o)
  | .codePoints => (.list (strCount first) (.num (Iv.ints 0 1114111)), o)
  | .toText => (optOf true (.str (listParts first).1), o)
  | .takeT =>
    let n := strCount first
    let k := ivA 0
    let lo := minLo n.lo ((k.lo.map nonNeg).or (some 0))
    (.str (countIv lo (minHi n.hi (k.hi.map nonNeg))), o)
  | .dropT =>
    let n := strCount first
    let k := ivA 0
    let most := k.hi.map nonNeg
    let least := (k.lo.map nonNeg).getD 0
    let lo := match n.lo, most with
      | some l, some h => some (l - h)
      | _, _ => some 0
    (.str (countIv lo (n.hi.map (· - least))), o)
  | .sliceT =>
    let n := strCount first
    let (a, b) := (ivA 0, ivA 1)
    let start := (a.lo.map nonNeg).getD 0
    let most := b.hi.map fun h => nonNeg h - start
    (.str (countIv (some 0) (minHi n.hi most)), o)
  -- where it is found: from 0 to the count less the count of what is looked for; or none
  | .indexOf =>
    let n := strCount first
    let t := strCount (arg 0)
    let hi := n.hi.map (· - (t.lo.map nonNeg).getD 0)
    if hi.any (·.num < 0) then (Abs.none', o) else (.opt true (.num (Iv.mk' (some 0) hi (some 1))), o)
  | .containsT | .endsWith => (Abs.bools, o)
  | .split =>
    let n := strCount first
    let sep := strCount (arg 0)
    let lo := if sep.lo.any (·.num ≥ 1) then some 1 else minLo n.lo (some 1)
    (.list (countIv lo (plusOne n.hi)) (.str (countIv (some 0) n.hi)), o)
  | .words =>
    let n := strCount first
    (.list (countIv (some 0) (plusOne n.hi)) (.str (countIv (some 0) n.hi)), o)
  | .replace => (.str (countIv (some 0) (some (i64max : Rat))), o)
  | .trim | .trimStart | .trimEnd => (.str (countIv (some 0) (strCount first).hi), o)
  | .lowerAscii | .upperAscii => (.str (strCount first), o)
  -- as long as the string, or as the width where the fill is not empty
  | .padStart | .padEnd =>
    let n := strCount first
    let (w, fill) := (ivA 0, strCount (arg 1))
    let lo := if fill.lo.any (·.num ≥ 1) then maxLo n.lo w.lo else n.lo
    (.str (countIv lo (maxHi n.hi w.hi)), o)
  | .charOf => (.str (Iv.ints 1 1), o)
  | .parseNumber => (.opt true (.num (Iv.mk' (some (i64min : Rat)) (some (i64max : Rat)) (some 1))), o)
  | .parseDate => (.opt true (.date (Iv.mk' (some (Text.DAY_MIN : Rat)) (some (Text.DAY_MAX : Rat)) (some 1))), o)

-- ── A call read again (kaname's DESIGN 26.5) ────────────────────────────────

/-- How many readings again of a body go in one another: a call past them, and a call in the ring of
    the function analysed now, take what the function answers analysed once. -/
def REREAD : Nat := 4

/-- The function a call reads again with the values it gives: a generic one, within `REREAD`
    readings, out of the ring of the function analysed now. -/
def rereads (A : ACtx) (f : Nat) : Option Fn :=
  match A.M.fns[f]? with
  | some fn => if fn.reread && decide (A.depth < REREAD) && !A.ring.contains f then some fn else none
  | none => none

/-- The ring of a function: the functions of the module that call each other with it, or itself. -/
def ringOf (M : Module) (f : Nat) : List Nat := ((M.rings.find? (·.1.contains f)).map (·.1)).getD [f]

/-- The value a call gives a parameter, read in its unit (the first argument that goes to it), or
    every value it declares where none goes. -/
def givenOne (args : List (Nat × Expr × Option Conv)) (vals : List Abs) (k : Nat) (p : Param) : Abs :=
  match (args.zip vals).find? (fun a => a.1.1 == k) with
  | some ((_, _, c), v) => scaleBy c v
  | none => p.abs

/-- The scope a call reads the body of `fn` again in: its parameters given the values of the call,
    in the ring of `fn`, one reading deeper. -/
def rereadCtx (A : ACtx) (fn : Fn) (f : Nat) (args : List (Nat × Expr × Option Conv)) (vals : List Abs) : ACtx :=
  { M := A.M, results := A.results, ring := ringOf A.M f,
    env := ((fn.params.zipIdx).map fun pk => (pk.1.name, givenOne args vals pk.2 pk.1)).reverse, facts := [],
    depth := A.depth + 1, own := false, lam := false }

/-- What a reading again comes to, as a function's analysis takes it (`analyzeFn`): the answer read
    in the result's unit and kept as the place of the answer where the result declares a range or a
    count, cut to what the result declares. The places the reading found, and the calls it made, are
    the call's own (they say its effect); the values of the body's expressions, and the relations its
    calls of rules keep, are not (nothing of the reading goes to the body's own analysis). -/
def rereadEnd (fn : Fn) (o : Out) (r : AR) : Abs × Out :=
  let (v, o') := convA r.out fn.conv r.abs
  let o' := if fn.ret.range?.isSome || fn.ret.count?.isSome then o'.place fn.answer "answer" .range (.slot fn.ret) v else o'
  (cutD D v fn.declared, { o' with vals := o.vals, kept := o.kept })

/-- The arguments of a builtin, each read in its place (the places `overflow` of the readings). -/
def convAll (o : Out) : List (Expr × Option Conv) → List Abs → List Abs × Out
  | (_, c) :: rest, a :: as =>
    let (b, o) := convA o c a
    let (bs, o) := convAll o rest as
    (b :: bs, o)
  | _, _ => ([], o)

mutual
/-- An expression's values, kept by its id, and the scope from here on. -/
def ev (A : ACtx) : Nat → Expr → Out → AR
  | 0, _, o => AR.of .any (outOfFuel o) A
  | n + 1, e, o =>
    match e with
    | .letIn x v c body =>
      let r := ev A n v o
      match r.abs with
      | .never => AR.of .never r.out r.ctx
      | _ =>
        let (a, o) := convA r.out c r.abs
        let rb := ev (r.ctx.bindNew x a) n body o
        -- the rest of the block is its own scope: what it narrows and names stays in it
        AR.of rb.abs rb.out r.ctx
    | _ =>
      let r := evI A n e o
      match e.id? with
      | some id => { r with out := if A.lam then r.out.recordL id r.abs else r.out.record id r.abs }
      | Option.none => r
termination_by structural fuel _ _ => fuel

/-- A block that answers for an expression: its answer read in the expression's unit. What the
    block narrows stays in it. -/
def branch (A : ACtx) : Nat → Expr → Option Conv → Out → Abs × Out
  | 0, _, _, o => (.any, outOfFuel o)
  | n + 1, b, c, o =>
    let r := ev A n b o
    convA r.out c r.abs
termination_by structural fuel _ _ _ => fuel

def evI (A : ACtx) : Nat → Expr → Out → AR
  | 0, _, o => AR.of .any (outOfFuel o) A
  | n + 1, e, o =>
    match e with
    | .num id _ v =>
      let a := litAbs v
      AR.of a (if fits v then o else o.place id "overflow" .overflow .overflow a) A
    | .date _ d => AR.of (.date (Iv.ints d d)) o A
    | .str _ s => AR.of (.str (Iv.ints s.length s.length)) o A
    | .bool _ b => AR.of (boolAbs b) o A
    | .none _ => AR.of Abs.none' o A
    | .enum _ v => AR.of (.set (some [v])) o A
    | .var _ _ x => ⟨(A.lookup x).getD .any, o, A, .sym (x, [])⟩
    | .field _ _ base f =>
      let r := ev A n base o
      ⟨r.abs.field f, r.out, r.ctx, match sym e with | some s => .sym s | Option.none => .none⟩
    | .neg id _ x =>
      let r := ev A n x o
      match r.abs with
      | .num i => let (a, o) := heldA r.out id (.num i.negated); AR.of a o r.ctx
      | .never => AR.of .never r.out r.ctx
      | _ => AR.of .any (unknown r.out id) r.ctx
    | .not _ _ x =>
      let r := ev A n x o
      let a := match r.abs with
        | .set (some s) => Abs.set (some (setOf (s.map fun b => if b == "true" then "false" else "true")))
        | _ => Abs.bools
      ⟨a, r.out, r.ctx, .not r.nar⟩
    | .inUnit id _ x c =>
      let r := ev A n x o
      let (v, o) := convA r.out c r.abs
      -- a number held to int64 (`interp`'s `x in u`); a value that may be a number the analysis does
      -- not know (anything, or an optional) keeps the place too
      match v with
      | .num _ | .any | .opt _ _ => let (a, o) := heldA o id v; AR.of a o r.ctx
      | v => AR.of v o r.ctx
    | .and _ _ a b => andOr A n true a b o
    | .or _ _ a b => andOr A n false a b o
    | .arith id _ op a b ca cb step =>
      let ra := ev A n a o
      let rb := ev ra.ctx n b ra.out
      let A' := rb.ctx
      let o := rb.out
      match ra.abs, rb.abs with
      | .never, _ => AR.of .never o A'
      | _, .never => AR.of .never o A'
      | .date x, .date y => AR.of (.num ((x.minus y).withDen (some 1))) o A'
      | .num _, .num _ =>
        let dn := stepDen step
        match op with
        | .add | .sub =>
          let (pa, o) := convA o ca ra.abs
          let (pb, o) := convA o cb rb.abs
          match pa.iv, pb.iv with
          | some x, some y =>
            let iv := if op == .add then x.plus y
              else if ca.isNone && cb.isNone then subRel A' a b step (x.minus y)
              else x.minus y
            let (r, o) := heldA o id (.num (iv.withDen dn))
            AR.of r o A'
          | _, _ => AR.of .any (unknown o id) A'
        | .mul =>
          match ra.abs.iv, rb.abs.iv with
          | some x, some y => let (r, o) := heldA o id (.num ((x.times y).withDen dn)); AR.of r o A'
          | _, _ => AR.of .any (unknown o id) A'
        | .div =>
          let (pa, o) := convA o ca ra.abs
          let (pb, o) := convA o cb rb.abs
          match pa.iv, pb.iv with
          | some x, some y =>
            let o := o.place id "zero" .zero .zero pb
            if y.isExact == some 0 then AR.of .never o A'
            else let (r, o) := heldA o id (.num ((x.over y).withDen dn)); AR.of r o A'
          | _, _ => AR.of .any (unknown o id) A'
      | _, _ => AR.of .any (unknown o id) A'
    | .ddiff _ a b =>
      let ra := ev A n a o
      let rb := ev ra.ctx n b ra.out
      match ra.abs, rb.abs with
      | .never, _ => AR.of .never rb.out rb.ctx
      | _, .never => AR.of .never rb.out rb.ctx
      | .date x, .date y => AR.of (.num ((x.minus y).withDen (some 1))) rb.out rb.ctx
      | _, _ => AR.of .any rb.out rb.ctx
    | .cmp _ _ op a b ca cb _ sa sb =>
      let ra := ev A n a o
      let rb := ev ra.ctx n b ra.out
      let (pa, o) := convA rb.out ca ra.abs
      let (pb, o) := convA o cb rb.abs
      let decided : Option Bool := match pa, pb with
        | .num x, .num y => decide' x op y
        | .date x, .date y => decide' x op y
        | .set (some x), .set (some y) =>
          if op == .eq || op == .ne then
            if x.length == 1 && y.length == 1 then some ((x == y) == (op == .eq))
            else if x.all (fun v => !y.contains v) then some (op == .ne) else Option.none
          else Option.none
        -- two values of an enum that has fields: no value the same, or the one value with no fields
        | .variants x, .variants y =>
          if op == .eq || op == .ne then
            if x.all (fun k => !(y.any (·.1 == k.1))) then some (op == .ne)
            else if x.length == 1 && x.map (·.1) == y.map (·.1) && (x ++ y).all (fun r => r.2.isEmpty) then
              some (op == .eq)
            else Option.none
          else Option.none
        | _, _ => Option.none
      ⟨match decided with | some v => bools v (!v) | Option.none => Abs.bools, o, rb.ctx,
        .cmp op a b ca cb sa sb ra.abs rb.abs⟩
    | .ite _ c t f ct cf =>
      let rc := ev A n c o
      match rc.abs with
      | .never => AR.of .never rc.out rc.ctx
      | _ =>
        let (canT, canF) := truths rc.abs
        let (out, o) := if canT then
            let (v, o) := branch (rc.nar.apply true rc.ctx) n t ct rc.out
            (Abs.never.join v, o)
          else (Abs.never, rc.out)
        let (out, o) := if canF then
            let (v, o) := branch (rc.nar.apply false rc.ctx) n f cf o
            (out.join v, o)
          else (out, o)
        AR.of out o rc.ctx
    | .mtch _ _ s arms =>
      let rs := ev A n s o
      match rs.abs with
      | .never => AR.of .never rs.out rs.ctx
      | _ =>
        let (out, o) := matchArms rs.ctx n (sym s) rs.abs [] arms .never rs.out
        AR.of out o rs.ctx
    | .letIn _ _ _ _ => ev A n e o
    | .block _ b c => let (v, o) := branch A n b c o; AR.of v o A
    | .call _ _ f args =>
      let (vals, o, A') := evArgs A n args o
      match vals with
      | Option.none => AR.of .never o A'
      | some vals =>
        match rereads A' f with
        -- a generic function: its body read again with the values the call gives (DESIGN 26.5)
        | some fn =>
          let (o1, A1) := (args.zip vals).foldl (callStep fn) (o, A')
          let (a, o2) := rereadEnd fn o1 (ev (rereadCtx A1 fn f args vals) n fn.body o1)
          AR.of a o2 A1
        | Option.none =>
          let (a, o, A'') := callAbs A' o f args vals
          -- each lambda given, read once with every value of its parameters' types, so that its
          -- places are the caller's (kaname's DESIGN 27.4)
          AR.of a (onceAll A'' n vals o) A''
    | .rule id _ r args =>
      let (vals, o, A') := evArgs A n args o
      match vals with
      | Option.none => AR.of .never o A'
      | some vals => let (a, o, A'') := ruleAbs A' o id r args vals; AR.of a o A''
    | .dateCall _ _ file date args =>
      let (vals, o, A') := evArgs A n args o
      match vals with
      | Option.none => AR.of .never o A'
      | some vals => let (a, o, A'') := dateAbs A' o file date args vals; AR.of a o A''
    | .transfer _ _ _ args =>
      let (vals, o, A') := evArgs5 A n args o
      match vals with
      | Option.none => AR.of .never o A'
      | some vals => let (a, o) := transferAbs o args vals; AR.of a o A'
    | .record _ _ module fields inits tag base =>
      match base with
      -- the value the fields not written are taken from, first
      | some b =>
        let rb := ev A n b o
        match rb.abs with
        | .never => AR.of .never rb.out rb.ctx
        | fa =>
          let (vals, o, A') := evInits rb.ctx n inits rb.out
          match vals with
          | Option.none => AR.of .never o A'
          | some vals => let (a, o, A'') := recordAbs A' o module fields inits vals tag (some (fa, exprId b)); AR.of a o A''
      | Option.none =>
        let (vals, o, A') := evInits A n inits o
        match vals with
        | Option.none => AR.of .never o A'
        | some vals => let (a, o, A'') := recordAbs A' o module fields inits vals tag none; AR.of a o A''
    | .list _ elems => listAbs A n elems .never 0 o
    | .walk id _ w list x body c ord step => walkAbs A n id w list x body c ord step o
    | .reduce _ _ list first acc x body c out => reduceAbs A n list first acc x body c out o
    | .minmax _ _ isMin a b ca cb _ =>
      let ra := ev A n a o
      match ra.abs with
      | .never => AR.of .never ra.out ra.ctx
      | _ =>
        let rb := ev ra.ctx n b ra.out
        match rb.abs with
        | .never => AR.of .never rb.out rb.ctx
        | _ =>
          let (pa, o) := convA rb.out ca ra.abs
          let (pb, o) := convA o cb rb.abs
          let a := match pa, pb with
            | .num x, .num y => Abs.num (minMaxIv isMin x y)
            | .date x, .date y => Abs.date (minMaxIv isMin x y)
            | _, _ => pa.join pb
          AR.of a o rb.ctx
    | .modOp id _ x k =>
      let rx := ev A n x o
      match rx.abs with
      | .never => AR.of .never rx.out rx.ctx
      | _ =>
        let rk := ev rx.ctx n k rx.out
        match rk.abs with
        | .never => AR.of .never rk.out rk.ctx
        | _ =>
          let ivx := rx.abs.iv.getD Iv.top
          let ivk := rk.abs.iv.getD Iv.top
          let m : Option Int := match ivk.isExact with
            | some q => if q.den == 1 && q.num > 0 then some q.num else Option.none
            | Option.none => Option.none
          match m with
          | some m =>
            let hi : Int := match ivx.lo, ivx.hi with
              | some l, some h => if l.num ≥ 0 && h < (m : Rat) then h.floor else m - 1
              | _, _ => m - 1
            AR.of (.num (Iv.ints 0 hi)) rk.out rk.ctx
          | Option.none =>
            -- a divisor that is a value: `zero` where it can be 0; the remainder from 0 to one less
            -- than its greatest size (Euclid's, DESIGN 24.5)
            let o := rk.out.place id "zero" .zero .zero rk.abs
            if ivk.isExact == some 0 then AR.of .never o rk.ctx
            else
              let mag := fun (r : Rat) => if r.num < 0 then -r else r
              let size : End := match ivk.lo, ivk.hi with
                | some l, some h => some (if mag h < mag l then mag l else mag h)
                | _, _ => Option.none
              AR.of (.num (Iv.mk' (some 0) (size.map (· - 1)) (some 1))) o rk.ctx
    | .isqrt _ _ x =>
      let rx := ev A n x o
      match rx.abs with
      | .never => AR.of .never rx.out rx.ctx
      | v =>
        let iv := v.iv.getD Iv.top
        let o := rx.out.place (exprId x) "isqrt" .range .isqrt v
        let A' := assume rx.ctx x ⟨some 0, Option.none⟩ Option.none
        let lo : Rat := ((iv.lo.map fun l => if l.num < 0 then (0 : Rat) else l).bind isqrtN).getD 0
        let hi := iv.hi.bind isqrtN
        AR.of (.num (Iv.mk' (some lo) hi (some 1))) o A'
    | .allocate id _ t c s cc cs out =>
      let rt := ev A n t o
      match rt.abs with
      | .never => AR.of .never rt.out rt.ctx
      | _ =>
        let rc := ev rt.ctx n c rt.out
        match rc.abs with
        | .never => AR.of .never rc.out rc.ctx
        | _ =>
          let rs := ev rc.ctx n s rc.out
          match rs.abs with
          | .never => AR.of .never rs.out rs.ctx
          | _ =>
            let (pc, o) := convA rs.out cc rc.abs
            let (ps, o) := convA o cs rs.abs
            let o := o.place id "zero" .zero .zero ps
            let ti := rt.abs.iv.getD Iv.top
            let ci := pc.iv.getD Iv.top
            let si := ps.iv.getD Iv.top
            let product := ti.times ci
            let quotient := product.over si
            let rounded := quotient.round .down 1
            let worst := if !product.fits then product else if !quotient.fits then quotient else rounded
            let o := o.place id "overflow" .overflow .overflow (.num worst)
            if si.isExact == some 0 then AR.of .never o rs.ctx
            else
              let (r, o) := heldA o id (.num rounded)
              let (r, o) := convA o out r
              AR.of r o rs.ctx
    | .round id _ way x g cg out top =>
      let rx := ev A n x o
      match rx.abs with
      | .never => AR.of .never rx.out rx.ctx
      | _ =>
        let rg := ev rx.ctx n g rx.out
        match rg.abs with
        | .never => AR.of .never rg.out rg.ctx
        | _ =>
          let (pg, o) := convA rg.out cg rg.abs
          match (pg.iv.getD Iv.top).isExact with
          | some grid =>
            if grid > 0 then
              let r := (rx.abs.iv.getD Iv.top).round way grid
              let (a, o) := heldA o id (.num r)
              let (a, o) := convA o out a
              AR.of a o rg.ctx
            else AR.of top o rg.ctx
          | Option.none =>
            -- a step that is no one value (the check of types takes a constant, E104): the
            -- rounding can leave int64, and nothing says it does not
            AR.of top (o.place id "overflow" .overflow .overflow .any) rg.ctx
    | .count1 _ l =>
      let r := ev A n l o
      let a := match r.abs with
        | .never => Abs.never
        | .list c _ => Abs.num c
        | .dict c _ _ => Abs.num c
        | .members c _ => Abs.num c
        | .str c => Abs.num c
        | _ => Abs.num (Iv.mk' (some 0) Option.none (some 1))
      AR.of a r.out r.ctx
    | .startsWith _ s p =>
      let rs := ev A n s o
      match rs.abs with
      | .never => AR.of .never rs.out rs.ctx
      | _ =>
        let rp := ev rs.ctx n p rs.out
        match rp.abs with
        | .never => AR.of .never rp.out rp.ctx
        | _ => AR.of Abs.bools rp.out rp.ctx
    | .concat _ args =>
      let (vals, o, A') := evList A n args o
      match vals with
      | Option.none => AR.of .never o A'
      | some vs => AR.of (.str (vs.foldl (fun acc v => addCounts acc (strCount v)) (Iv.ints 0 0))) o A'
    | .text _ x fmt =>
      let r := ev A n x o
      match r.abs with
      | .never => AR.of .never r.out r.ctx
      | a => AR.of (.str (textCount fmt a)) r.out r.ctx
    | .bad _ _ args =>
      let (vals, o, A') := evList A n args o
      match vals with
      | Option.none => AR.of .never o A'
      | some _ => AR.of .any o A'
    -- a lambda made a value, with what the names in scope are now (kaname's DESIGN 27.4)
    | .lam id => AR.of (.fns [(id, A.env, A.own)] Option.none) o A
    -- a function value called: what it is, then its arguments, each read in its parameter's unit
    | .callV _ _ f args top =>
      let rf := ev A n f o
      match rf.abs with
      | .never => AR.of .never rf.out rf.ctx
      | fa =>
        let (vals, o, A') := evList rf.ctx n (args.map (·.1)) rf.out
        match vals with
        | Option.none => AR.of .never o A'
        | some vals =>
          let (cvals, o) := convAll o args vals
          let (a, o) := callVAbs A' n fa cvals top o
          AR.of a o A'
    | .dict _ _ pairs => pairsAbs A n pairs .never .never 0 o
    | .prim id _ op _ args top =>
      let (vals, o, A') := evList A n (args.map (·.1)) o
      match vals with
      | Option.none => AR.of .never o A'
      | some vals =>
        -- each argument read in its place, and the builtin's places, whatever it is called on
        let (cvals, o) := convAll o args vals
        let (a, o) := primAbs o id op cvals
        AR.of (top.getD a) o A'
    | .walkK _ _ w coll x body c _ =>
      let r := ev A n coll o
      match w with
      -- the entries of a dict, each given to the condition: the pairs it can keep
      | .filterD =>
        match r.abs with
        | .never => AR.of .never r.out r.ctx
        | d =>
          let (cnt, k, v) := match d with
            | .dict cnt k v => (cnt, k, v)
            | _ => (anyCount, Abs.any, Abs.any)
          let ((_, _, held), o) := condition r.ctx none n x (entryAbs k v) body c r.out
          AR.of (.dict (upTo cnt) (held.field "key") (held.field "value")) o r.ctx
      -- the values of a dict, each given to the function: the same count and keys
      | .mapValues =>
        match r.abs with
        | .never => AR.of .never r.out r.ctx
        | d =>
          let (cnt, k, v) := match d with
            | .dict cnt k v => (cnt, k, v)
            | _ => (anyCount, Abs.any, Abs.any)
          let (u, o) := lam1 r.ctx none n x v body c r.out
          let none' := cnt.hi.any (·.num ≤ 0) || (match v with | .never => true | _ => false)
          AR.of (.dict cnt k (if none' then .never else u)) o r.ctx
      | _ =>
        let (count, elem) := walked r.abs
        if count.hi.any (fun h => h.num < 0) then AR.of .never r.out r.ctx
        else
          let noneAll := noneAtAll count elem
          match w with
          | .last =>
            let ((_, _, held), o) := condition r.ctx (totalsOf coll) n x elem body c r.out
            AR.of (.opt true (stripOpt held)) o r.ctx
          -- the same elements in another order: the keys are worked out for their places
          | .sortBy | .sortByDesc =>
            let (_, o) := lam1 r.ctx (totalsOf coll) n x elem body c r.out
            AR.of (.list count (if noneAll then .never else elem)) o r.ctx
          | .maxBy | .minBy =>
            let (_, o) := lam1 r.ctx (totalsOf coll) n x elem body c r.out
            AR.of (optOf (count.lo.all fun l => l.num ≤ 0) (if noneAll then .never else elem)) o r.ctx
          | .flatMap =>
            let (v, o) := lam1 r.ctx (totalsOf coll) n x elem body c r.out
            let (ci, ei) := match v with
              | .list ci ei => (ci, ei)
              | _ => (anyCount, Abs.any)
            if noneAll then AR.of (.list (Iv.ints 0 0) .never) o r.ctx
            else AR.of (.list (timesCount count ci) ei) o r.ctx
          | .filterMap =>
            let (v, o) := lam1 r.ctx (totalsOf coll) n x elem body c r.out
            let kept := match v with
              | .opt _ y => y
              | y => y
            AR.of (.list (upTo count) (if noneAll then .never else kept)) o r.ctx
          -- a pair for each key, the elements of a key from one to as many as the list has
          | .groupBy =>
            let (kv, o) := lam1 r.ctx (totalsOf coll) n x elem body c r.out
            if noneAll then AR.of (.dict (Iv.ints 0 0) .never .never) o r.ctx
            else AR.of (.dict (oneUp count) kv (.list (countIv (some 1) count.hi) elem)) o r.ctx
          | _ => AR.of .any r.out r.ctx
    | .toDict _ _ _ list x kb kc y vb vc =>
      let r := ev A n list o
      let (count, elem) := walked r.abs
      if count.hi.any (fun h => h.num < 0) then AR.of .never r.out r.ctx
      else
        let (ka, o) := lam1 r.ctx none n x elem kb kc r.out
        let (va, o) := lam1 r.ctx none n y elem vb vc o
        if noneAtAll count elem then AR.of (.dict (Iv.ints 0 0) .never .never) o r.ctx
        else AR.of (.dict (oneUp count) ka va) o r.ctx
    | .mergeWith _ _ _ a b x y body c =>
      let ra := ev A n a o
      match ra.abs with
      | .never => AR.of .never ra.out ra.ctx
      | _ =>
        let rb := ev ra.ctx n b ra.out
        match rb.abs with
        | .never => AR.of .never rb.out rb.ctx
        | _ =>
          let (ca, ka, va) := dictParts ra.abs
          let (cb, kb, vb) := dictParts rb.abs
          let (u, o) := lam2 rb.ctx n x y va vb body c rb.out
          AR.of (.dict (mergedC ca cb) (ka.join kb) ((va.join vb).join u)) o rb.ctx
termination_by structural fuel _ _ => fuel

/-- `and` and `or`: the right is analysed only where the left does not decide, with the left
    narrowed to what lets the right be evaluated; what the right narrows stays in it. -/
def andOr (A : ACtx) : Nat → Bool → Expr → Expr → Out → AR
  | 0, _, _, _, o => AR.of .any (outOfFuel o) A
  | n + 1, isAnd, a, b, o =>
    let ra := ev A n a o
    let (lt, lf) := truths ra.abs
    let runs := if isAnd then lt else lf
    let (rt, rf, o, nb) := if runs then
        let rb := ev (ra.nar.apply isAnd ra.ctx) n b ra.out
        let (t, f) := truths rb.abs
        (t, f, rb.out, rb.nar)
      else (false, false, ra.out, Nar.none)
    let (t, f) := if isAnd then (lt && rt, lf || rf) else (lt || rt, lf && rf)
    ⟨bools t f, o, ra.ctx, if isAnd then .and ra.nar nb else .or ra.nar nb⟩
termination_by structural fuel _ _ _ _ => fuel

/-- The arms of a `match`, each reached by what its patterns take, its block in the scope its
    patterns narrow; `prev` the patterns of the arms above. -/
def matchArms (A : ACtx) : Nat → Option Sym → Abs → List Pat → List (List Pat × Expr × Option Conv) → Abs → Out → Abs × Out
  | 0, _, _, _, _, out, o => (out.join .any, outOfFuel o)
  | _ + 1, _, _, _, [], out, o => (out, o)
  | n + 1, sy, sv, prev, (ps, body, c) :: rest, out, o =>
    let (A', reached) := armScope A sy sv prev ps
    let (out, o) := if reached then
        let (v, o) := branch A' n body c o
        (out.join v, o)
      else (out, o)
    matchArms A n sy sv (prev ++ ps) rest out o
termination_by structural fuel _ _ _ _ _ _ => fuel

def evArgs (A : ACtx) : Nat → List (Nat × Expr × Option Conv) → Out → Option (List Abs) × Out × ACtx
  | 0, args, o => (some (args.map fun _ => .any), outOfFuel o, A)
  | _ + 1, [], o => (some [], o, A)
  | n + 1, (_, x, _) :: rest, o =>
    let r := ev A n x o
    match r.abs with
    | .never => (Option.none, r.out, r.ctx)
    | _ =>
      let (vs, o, A') := evArgs r.ctx n rest r.out
      (vs.map (r.abs :: ·), o, A')
termination_by structural fuel _ _ => fuel

def evArgs5 (A : ACtx) : Nat → List (String × Expr × Option Conv × Nat × Bool) → Out → Option (List Abs) × Out × ACtx
  | 0, args, o => (some (args.map fun _ => .any), outOfFuel o, A)
  | _ + 1, [], o => (some [], o, A)
  | n + 1, (_, x, _) :: rest, o =>
    let r := ev A n x o
    match r.abs with
    | .never => (Option.none, r.out, r.ctx)
    | _ =>
      let (vs, o, A') := evArgs5 r.ctx n rest r.out
      (vs.map (r.abs :: ·), o, A')
termination_by structural fuel _ _ => fuel

def evInits (A : ACtx) : Nat → List (Nat × Expr × Option Conv × Nat) → Out → Option (List Abs) × Out × ACtx
  | 0, args, o => (some (args.map fun _ => .any), outOfFuel o, A)
  | _ + 1, [], o => (some [], o, A)
  | n + 1, (_, x, _) :: rest, o =>
    let r := ev A n x o
    match r.abs with
    | .never => (Option.none, r.out, r.ctx)
    | _ =>
      let (vs, o, A') := evInits r.ctx n rest r.out
      (vs.map (r.abs :: ·), o, A')
termination_by structural fuel _ _ => fuel

def evList (A : ACtx) : Nat → List Expr → Out → Option (List Abs) × Out × ACtx
  | 0, args, o => (some (args.map fun _ => .any), outOfFuel o, A)
  | _ + 1, [], o => (some [], o, A)
  | n + 1, x :: rest, o =>
    let r := ev A n x o
    match r.abs with
    | .never => (Option.none, r.out, r.ctx)
    | _ =>
      let (vs, o, A') := evList r.ctx n rest r.out
      (vs.map (r.abs :: ·), o, A')
termination_by structural fuel _ _ => fuel

/-- A dict written out: each key and then its value read in its type as it comes, the keys joined
    and the values joined; from one pair to as many as are written (two keys of a value may be one). -/
def pairsAbs (A : ACtx) : Nat → List (Expr × Option Conv × Expr × Option Conv) → Abs → Abs → Nat → Out → AR
  | 0, _, _, _, _, o => AR.of .any (outOfFuel o) A
  | _ + 1, [], ks, vs, k, o => AR.of (.dict (Iv.ints ((min k 1 : Nat) : Int) (k : Int)) ks vs) o A
  | n + 1, (x, cx, y, cy) :: rest, ks, vs, k, o =>
    let r := ev A n x o
    match r.abs with
    | .never => AR.of .never r.out r.ctx
    | _ =>
      let (a, o) := convA r.out cx r.abs
      let r2 := ev r.ctx n y o
      match r2.abs with
      | .never => AR.of .never r2.out r2.ctx
      | _ =>
        let (b, o) := convA r2.out cy r2.abs
        pairsAbs r2.ctx n rest (ks.join a) (vs.join b) (k + 1) o
termination_by structural fuel _ _ _ _ _ => fuel

/-- A list written out: its count, and every element read in the list's element type. -/
def listAbs (A : ACtx) : Nat → List (Expr × Option Conv) → Abs → Nat → Out → AR
  | 0, _, _, _, o => AR.of .any (outOfFuel o) A
  | _ + 1, [], elem, k, o => AR.of (.list (Iv.ints k k) elem) o A
  | n + 1, (x, c) :: rest, elem, k, o =>
    let r := ev A n x o
    match r.abs with
    | .never => AR.of .never r.out r.ctx
    | _ =>
      let (v, o) := convA r.out c r.abs
      listAbs r.ctx n rest (elem.join v) (k + 1) o
termination_by structural fuel _ _ _ _ => fuel

/-- A function given to a builtin, analysed with its parameter bound to `elem` (and its `total` to
    the sum `tot`, where the builtin walks running totals): its answer read in its place. -/
def lam1 (A : ACtx) (tot : Option Term) : Nat → String → Abs → Expr → Option Conv → Out → Abs × Out
  | 0, _, _, _, _, o => (.any, outOfFuel o)
  | n + 1, x, elem, body, c, o =>
    let r := ev (A.bindLam x elem tot) n body o
    convA r.out c r.abs
termination_by structural fuel _ _ _ _ _ => fuel

/-- A condition given to a builtin, analysed on an element: whether it can hold and fail, and the
    element where it holds. Its answer is read in its place as the evaluation reads it (a bool is
    itself). -/
def condition (A : ACtx) (tot : Option Term) : Nat → String → Abs → Expr → Option Conv → Out → (Bool × Bool × Abs) × Out
  | 0, _, elem, _, _, o => ((true, true, elem), outOfFuel o)
  | n + 1, x, elem, body, c, o =>
    let r := ev (A.bindLam x elem tot) n body o
    let (v, o) := convA r.out c r.abs
    let (t, f) := truths v
    let held := ((r.nar.apply true r.ctx).lookup x).getD elem
    ((t, f, held), o)
termination_by structural fuel _ _ _ _ _ => fuel

def walkAbs (A : ACtx) : Nat → Nat → Walk → Expr → String → Expr → Option Conv → Order → Option Rat → Out → AR
  | 0, _, _, _, _, _, _, _, _, o => AR.of .any (outOfFuel o) A
  | n + 1, id, w, list, x, body, c, _, step, o =>
    let rl := ev A n list o
    let A := rl.ctx
    let o := rl.out
    let (count, elem) := walked rl.abs
    if count.hi.any (fun h => h.num < 0) then AR.of .never o A
    else if w.isCondition then
      let ((t, f, held), o) := condition A (totalsOf list) n x elem body c o
      AR.of (condResult w count elem t f held) o A
    else
      let (v, o) := lam1 A (totalsOf list) n x elem body c o
      let (r, o) := lamResult o id w count elem v step
      AR.of r o A
termination_by structural fuel _ _ _ _ _ _ _ _ _ => fuel

/-- `reduce(xs, first, (acc, x) => …)` (DESIGN 5.5): the function laid over the running value as
    many times as the list can have elements; where that is more than 16 or not known, three times,
    and an end that still moves is let go. -/
def reduceAbs (A : ACtx) : Nat → Expr → Expr → String → String → Expr → Option Conv → Option Conv → Out → AR
  | 0, _, _, _, _, _, _, _, o => AR.of .any (outOfFuel o) A
  | n + 1, list, first, acc, x, body, c, out, o =>
    let rl := ev A n list o
    let (count, elem) := walked rl.abs
    if count.hi.any (fun h => h.num < 0) then AR.of .never rl.out rl.ctx
    else
      let r0 := ev rl.ctx n first rl.out
      match r0.abs with
      | .never => AR.of .never r0.out r0.ctx
      | v0 =>
        let exact := exactRounds count
        let rounds := exact.getD 3
        let ((accA, settled), o) := reduceRounds r0.ctx n acc x body c elem rounds v0 r0.out
        let (accA, o) := if !settled && exact.isNone then
            let a1 := widenD D v0 accA
            let (out1, o) := lam2 r0.ctx n acc x a1 elem body c o
            (widenD D v0 (a1.join out1), { o with widened := true })
          else (accA, o)
        let (a, o) := convA o out accA
        AR.of a o r0.ctx
termination_by structural fuel _ _ _ _ _ _ _ _ => fuel

def reduceRounds (A : ACtx) : Nat → String → String → Expr → Option Conv → Abs → Nat → Abs → Out → (Abs × Bool) × Out
  | 0, _, _, _, _, _, _, a, o => ((a.join .any, false), outOfFuel o)
  | _ + 1, _, _, _, _, _, 0, a, o => ((a, false), o)
  | n + 1, acc, x, body, c, elem, k + 1, a, o =>
    let (out, o) := lam2 A n acc x a elem body c o
    let next := a.join out
    if next == a then ((a, true), o)
    else reduceRounds A n acc x body c elem k next o
termination_by structural fuel _ _ _ _ _ _ _ _ => fuel

/-- `reduce`'s function, analysed with its two parameters bound. -/
def lam2 (A : ACtx) : Nat → String → String → Abs → Abs → Expr → Option Conv → Out → Abs × Out
  | 0, _, _, _, _, _, _, o => (.any, outOfFuel o)
  | n + 1, acc, x, a, elem, body, c, o =>
    let r := ev ((A.bindNew acc a).bindNew x elem) n body o
    convA r.out c r.abs
termination_by structural fuel _ _ _ _ _ _ _ => fuel

/-- The bodies of lambdas read, each with what it caught and its parameters given `args` (where
    `args` says none, every value of its parameters' types), in the ring of the function it is written
    in, with no condition in force (kaname's DESIGN 27.4); the answers, each read in the type of the
    lambda's answer, joined with `acc`. -/
def lamsAbs (A : ACtx) : Nat → List (Nat × List (String × Abs) × Bool) → Option (List Abs) → Abs → Out → Abs × Out
  | 0, _, _, _, o => (.any, outOfFuel o)
  | _ + 1, [], _, acc, o => (acc, o)
  | n + 1, (i, ae, own) :: rest, args, acc, o =>
    match A.M.lams.find? (·.id == i) with
    | Option.none => lamsAbs A n rest args (acc.join .any) o
    | some lam =>
      let given := args.getD (lam.params.map (·.2))
      let B : ACtx := { A with ring := ringOf A.M lam.fn, env := ((lam.params.map (·.1)).zip given).reverse ++ ae,
                               facts := [], own := own, lam := own }
      let r := ev B n lam.body o
      let (a, o) := convA r.out lam.conv r.abs
      lamsAbs A n rest args (acc.join a) o
termination_by structural fuel _ _ _ _ => fuel

/-- A function value called with `args` (kaname's DESIGN 27.4): each lambda it can be read with them,
    the answers joined; one the analysis does not see answers every value of the type of the answer
    (`top`), makes no place, and gives the function the effect its type writes. -/
def callVAbs (A : ACtx) : Nat → Abs → List Abs → Abs → Out → Abs × Out
  | 0, _, _, _, o => (.any, outOfFuel o)
  | n + 1, fa, args, top, o =>
    let (fs, unseen) : List (Nat × List (String × Abs) × Bool) × Option (Option (List Kind × Bool)) :=
      match fa with
      | .fns fs u => (fs, u)
      | _ => ([], some Option.none)
    let (a, o) := lamsAbs A n fs (some args) .never o
    match unseen with
    | Option.none => (a, o)
    | some w =>
      -- what the theorems leave out, as a running value let go (`widened`)
      let o := { o with unseen := true, widened := true, thru := match w with
        | some k => o.thru ++ [k]
        | Option.none => o.thru }
      (a.join top, o)
termination_by structural fuel _ _ _ _ => fuel

/-- The lambdas given to a call that does not read the called function's body again, each read once
    with every value of its parameters' types (kaname's DESIGN 27.4). -/
def onceAll (A : ACtx) : Nat → List Abs → Out → Out
  | 0, _, o => outOfFuel o
  | _ + 1, [], o => o
  | n + 1, v :: vs, o =>
    let o := match v with
      | .fns fs _ => (lamsAbs A n fs Option.none .never o).2
      | _ => o
    onceAll A n vs o
termination_by structural fuel _ _ => fuel
end

-- ── A module ────────────────────────────────────────────────────────────────

/-- What the analysis finds of one function: what it answers, cut to what its result declares,
    its places, the functions it calls, and the relations each of its calls of a rule keeps between
    the inputs it gives. -/
structure FnFacts where
  result : Abs
  places : List Place
  calls : List Nat
  widened : Bool
  kept : List (Nat × List (String × Rel × String)) := []
  /-- The effects written on the types of the function values it calls that the analysis does not
      see, and whether it takes functions or calls such a value (kaname's DESIGN 27.4, 27.5): what
      the theorems leave out. -/
  thru : List (List Kind × Bool) := []
  unseen : Bool := false
  deriving Inhabited

/-- What a function is inferred to do (DESIGN 4.1): the kinds it can fail with (the places that
    wait for ritsu-cross counted), the kinds of the places decided, whether it may not stop. -/
structure Effect where
  fails : List Kind
  decided : List Kind
  diverges : Bool
  /-- Whether the effect of a function it is given goes on to its callers (kaname's DESIGN 27.5): it
      takes functions, or calls one that does without reading it again. What `total_sound` leaves
      out. -/
  through : Bool := false
  deriving Inhabited, Repr

def Effect.total (e : Effect) : Bool := e.fails.isEmpty && !e.diverges

/-- The kinds in the order kaname writes them. -/
def kindOrder : List Kind := [.rule, .date, .range, .overflow, .zero]

def addKind (ks : List Kind) (k : Kind) : List Kind :=
  kindOrder.filter fun x => ks.contains x || x == k

/-- How many steps the analysis of one function may take: more than any of the tests takes. -/
def FUEL : Nat := 1000000

/-- The analysis of one function, in the ring `ring`, the values of the module's expressions so far
    given (`ranges::Analyzer::function`). -/
def analyzeFn (M : Module) (results : List (Option Abs)) (ring : List Nat) (f : Nat) (vals : List (Nat × Abs)) :
    FnFacts × List (Nat × Abs) :=
  match M.fns[f]? with
  | none => (default, vals)
  | some fn =>
    let A : ACtx := { M, results, ring, env := fn.params.foldl (fun env p => (p.name, p.abs) :: env) [], facts := [], depth := 0 }
    let r := ev A FUEL fn.body ⟨[], vals, [], false, [], [], [], [], false⟩
    let (v, o) := convA r.out fn.conv r.abs
    -- the answer is held to what the result declares where it declares a range or a count (what
    -- `fn.retRanged` says)
    let o := if fn.ret.range?.isSome || fn.ret.count?.isSome then o.place fn.answer "answer" .range (.slot fn.ret) v else o
    -- what the bodies of the lambdas it made gave, wherever they were read (kaname's DESIGN 27.4)
    let o := o.merged
    ({ result := cutD D v fn.declared, places := o.places, calls := o.calls, widened := o.widened, kept := o.kept,
       thru := o.thru, unseen := o.unseen || fn.takesFn }, o.vals)

/-- One function of a ring analysed, once (each is in one ring): its facts kept, its answer given to
    the functions analysed after it. -/
def analyzeStep (M : Module) (ring : List Nat) (acc : List FnFacts × List (Option Abs) × List (Nat × Abs))
    (f : Nat) : List FnFacts × List (Option Abs) × List (Nat × Abs) :=
  if (acc.2.1[f]?).join.isSome then acc
  else
    let r := analyzeFn M acc.2.1 ring f acc.2.2
    (acc.1.set f r.1, acc.2.1.set f (some r.1.result), r.2)

/-- What a place says of the effect of its function: a kind it can fail with where it is not shown,
    decided where it does not wait for ritsu-cross. -/
def placeEffect (e : Effect) (p : Place) : Effect :=
  if p.shown then e
  else if p.pending then { e with fails := addKind e.fails p.kind }
  else { e with fails := addKind e.fails p.kind, decided := addKind e.decided p.kind }

/-- What a call says: one inside the ring goes round (each ring that does is `round` already); one
    to a function of a ring before brings that function's effect. -/
def callEffect (ring : List Nat) (effs : List Effect) (e : Effect) (g : Nat) : Effect :=
  if ring.contains g then { e with diverges := true }
  else match effs[g]? with
    | some o => { fails := o.fails.foldl addKind e.fails, decided := o.decided.foldl addKind e.decided,
                  diverges := e.diverges || o.diverges, through := e.through || o.through }
    | none => e

/-- What a call of a function value the analysis does not see brings: the effect its type writes
    (kaname's DESIGN 27.5). -/
def thruEffect (e : Effect) (k : List Kind × Bool) : Effect :=
  { e with fails := k.1.foldl addKind e.fails, decided := k.1.foldl addKind e.decided, diverges := e.diverges || k.2 }

/-- What one function of a ring brings to the ring's effect: its places, then its calls, then the
    effects written on the function values it calls that the analysis does not see. -/
def fnEffect (facts : List FnFacts) (ring : List Nat) (effs : List Effect) (e : Effect) (f : Nat) : Effect :=
  let ff := facts[f]?.getD default
  let e := ff.thru.foldl thruEffect (ff.calls.foldl (callEffect ring effs) (ff.places.foldl placeEffect e))
  { e with through := e.through || ff.unseen }

/-- The effect of a ring, every function of it the same. -/
def ringEffect (facts : List FnFacts) (effs : List Effect) (ring : List Nat) (round : Bool) : Effect :=
  ring.foldl (fnEffect facts ring effs) { fails := [], decided := [], diverges := round }

/-- The effects so far, with every function of a ring given the ring's. -/
def effectsStep (facts : List FnFacts) (effs : List Effect) (x : List Nat × Bool) : List Effect :=
  let e := ringEffect facts effs x.1 x.2
  x.1.foldl (fun effs f => effs.set f e) effs

/-- The analysis of a module: each ring of calls in order, each function of a ring once; then the
    effects, ring by ring (DESIGN 4.3). -/
def analyzeModule (M : Module) : List FnFacts × List Effect × List (Nat × Abs) :=
  let n := M.fns.length
  let acc := M.rings.foldl (fun acc x => x.1.foldl (analyzeStep M x.1) acc)
    (List.replicate n default, List.replicate n none, [])
  let effects := M.rings.foldl (effectsStep acc.1) (List.replicate n { fails := [], decided := [], diverges := true })
  (acc.1, effects, acc.2.2)

-- ── The verdicts of the checks across the languages ─────────────────────────

/-- A place that waits for a check across the languages, given the verdict the module's verdicts
    say of it (`ranges.rs`'s `across`): shown where it holds, decided and not shown where it does
    not. Any other place, and one no verdict names, is as it was. -/
def decideAcross (vs : List (Nat × String × Bool)) (p : Place) : Place :=
  match p.what with
  | .pending =>
    match vs.find? (fun v => v.1 == p.expr && v.2.1 == p.key) with
    | some v => { p with what := .across v.2.2 }
    | none => p
  | _ => p

/-- A function's facts with the verdicts taken. -/
def FnFacts.decided (vs : List (Nat × String × Bool)) (ff : FnFacts) : FnFacts :=
  { ff with places := ff.places.map (decideAcross vs) }

/-- The analysis of a module with the verdicts of the checks across the languages taken at the
    places they decide, and the effects inferred from those (kaname's DESIGN 4.3, 14.5): what
    `ritsu kaname` and `ritsu check` find. With no verdicts it is `analyzeModule`
    (`analyzeWith_nil`), which the theorems of `Total.lean` are of. -/
def analyzeWith (M : Module) : List FnFacts × List Effect × List (Nat × Abs) :=
  let (facts, _, vals) := analyzeModule M
  let facts := facts.map (FnFacts.decided M.verdicts)
  let effects := M.rings.foldl (effectsStep facts) (List.replicate M.fns.length { fails := [], decided := [], diverges := true })
  (facts, effects, vals)

end KanameCore
