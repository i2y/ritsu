/-
  A machine that carries more than its state (rulec's §15.217, §15.218), as far as a certificate
  speaks about it.

  `RulecCert.Machine` follows the state alone. A machine can also carry a number with both ends of
  its range, a truth value or another enum's value: the host hands each one back as an input on the
  next call. Where a case is, then, is the state together with those values — a **configuration**.
  A certificate cannot list configurations one by one when a number has many values, so it lists
  **places**: the state and, for each carried value, a coordinate — one value when the walk went
  through every value, an interval of them when it cut the number into intervals — or "outside the
  range". From a place the check counts every row whose box can take a configuration there, and sends
  each carried value to every coordinate that what the row writes can land on.

  Two theorems make that a proof. **The simulation** (`Model.simulates`): every call that really
  happens from a configuration at a place is one the check counts from that place, to a place the new
  configuration is at. A call that really happens is a point the row's box takes, with the values of
  the rule's names behind it: each numeric axis's coordinate holding its column's value, every input
  inside the range the rule declares for it (the door turns the rest away), every value the rule
  computes the value of its expression, and under `first` the row the one that answers. What a row
  carries on is what the output that carries it rounds what the row writes to. The interval
  arithmetic of `RulecCert.Values` is what bounds a computed value over a place, and the grid the
  output rounds to is what lands the next value on a coordinate. **Closure** (`Sys.reaches_mem`): a
  set of places that holds the start and is closed under the counted calls holds every configuration
  a case can reach, so the `final`, `never` and `once` checks run on the places say what they say of
  the configurations (`Sys.final_stays`, `Sys.never_after`, `Sys.once_below_two`), an `always` line
  holds wherever its cells fit the coordinates (`Model.always_holds`), and a carried number stays
  within its range wherever no place leaves it (`Model.range_holds`).

  When the walk went through every value one by one, each place is one configuration and the claims
  are read exactly; when it cut a number into intervals, the places allow more than happens, which is
  the safe direction for every one of these claims (the "多めの遷移" of the design). The start place
  holds a configuration whenever the program's start check passes (`Model.start_inhabited`), so the
  claims are about cases that exist. That a case can still finish is the other direction — calls
  that really do happen — and is not said here.
-/
import RulecCert.Machine
import RulecCert.Values
import RulecCert.Cells

namespace RulecCert
namespace Carried

/-! ## Places stand for configurations: closure under the counted calls -/

/-- A machine's calls over configurations `Nat × α` — the state, and what is carried besides it —
    and the places `Nat × β` a check reasons over. `call c r c'` is a call that really happens,
    through the row numbered `r`; `holds b a` says the carried values `a` are at the coordinates `b`;
    `succs` lists the calls the check counts from a place, each with the row that makes it. -/
structure Sys (α β : Type) where
  call : Nat × α → Nat → Nat × α → Prop
  holds : β → α → Prop
  succs : Nat × β → List (Nat × (Nat × β))

namespace Sys

variable {α β : Type}

/-- A configuration is at a place: the same state, and the carried values at its coordinates. -/
def At (S : Sys α β) (p : Nat × β) (c : Nat × α) : Prop := p.1 = c.1 ∧ S.holds p.2 c.2

/-- **The simulation**: every call that really happens from a configuration at a place is one the
    check counts from that place, to a place the configuration it leads to is at. -/
def Simulates (S : Sys α β) : Prop :=
  ∀ p c r c', S.At p c → S.call c r c' → ∃ q, (r, q) ∈ S.succs p ∧ S.At q c'

/-- The configurations a case can be in: it starts anywhere at the start place, and each call moves
    it on. -/
inductive Reaches (S : Sys α β) (start : Nat × β) : Nat × α → Prop
  | start {c : Nat × α} : S.At start c → Reaches S start c
  | call {c c' : Nat × α} {r : Nat} : Reaches S start c → S.call c r c' → Reaches S start c'

/-- The same, with whether the case has been in a state of `B` yet. -/
inductive ReachesAfter (S : Sys α β) (start : Nat × β) (B : List Nat) : Nat × α → Bool → Prop
  | start {c : Nat × α} : S.At start c → ReachesAfter S start B c (B.contains c.1)
  | call {c c' : Nat × α} {r : Nat} {b : Bool} : ReachesAfter S start B c b → S.call c r c' →
      ReachesAfter S start B c' (b || B.contains c'.1)

/-- The same, counting — up to two — the calls through rows `counts` marks. -/
inductive ReachesCounting (S : Sys α β) (start : Nat × β) (counts : Nat → Bool) :
    Nat × α → Nat → Prop
  | start {c : Nat × α} : S.At start c → ReachesCounting S start counts c 0
  | call {c c' : Nat × α} {r n : Nat} : ReachesCounting S start counts c n → S.call c r c' →
      ReachesCounting S start counts c' (min 2 (n + if counts r then 1 else 0))

/-- The flag of `ReachesAfter` is there for every configuration a case can reach. -/
theorem reachesAfter_of_reaches {S : Sys α β} {start : Nat × β} {B : List Nat} :
    ∀ {c}, S.Reaches start c → ∃ b, S.ReachesAfter start B c b := by
  intro c hc
  induction hc with
  | start hat => exact ⟨_, ReachesAfter.start hat⟩
  | call _ hcall ih =>
    obtain ⟨b, hb⟩ := ih
    exact ⟨_, ReachesAfter.call hb hcall⟩

variable [DecidableEq β]

/-- **The reach check**: the places hold the start and are closed under the counted calls. -/
def reachOk (S : Sys α β) (start : Nat × β) (P : List (Nat × β)) : Bool :=
  P.contains start && P.all (fun p => (S.succs p).all (fun e => P.contains e.2))

/-- **The final check**: from a place in a final state, every counted call stays in that state. -/
def finalOk (S : Sys α β) (F : List Nat) (P : List (Nat × β)) : Bool :=
  P.all (fun p => !F.contains p.1 || (S.succs p).all (fun e => e.2.1 == p.1))

/-- **The `never` check**: the pairs (place, has the case been in `B`) closed under the counted
    calls, holding where a case starts, none in `A` with the flag set. -/
def neverOk (S : Sys α β) (start : Nat × β) (A B : List Nat) (Q : List ((Nat × β) × Bool)) : Bool :=
  Q.contains (start, B.contains start.1) &&
    Q.all (fun q => !(q.2 && A.contains q.1.1) &&
      (S.succs q.1).all (fun e => Q.contains (e.2, q.2 || B.contains e.2.1)))

/-- **The `once` check**: the pairs (place, counted calls so far, up to two) closed under the counted
    calls, holding where a case starts, none at two. -/
def onceOk (S : Sys α β) (start : Nat × β) (counts : Nat → Bool) (Q : List ((Nat × β) × Nat)) :
    Bool :=
  Q.contains (start, 0) &&
    Q.all (fun q => decide (q.2 < 2) &&
      (S.succs q.1).all (fun e => Q.contains (e.2, min 2 (q.2 + if counts e.1 then 1 else 0))))

/-- **Closure** ("多めの遷移の上で閉じた集合は、本当の遷移でも閉じている"): where the simulation
    holds, places the reach check passes hold every configuration a case can reach. -/
theorem reaches_mem {S : Sys α β} (hsim : S.Simulates) {start : Nat × β} {P : List (Nat × β)}
    (h : S.reachOk start P = true) : ∀ {c}, S.Reaches start c → ∃ p ∈ P, S.At p c := by
  simp only [reachOk, Bool.and_eq_true, List.contains_iff_mem, List.all_eq_true] at h
  intro c hc
  induction hc with
  | start hat => exact ⟨_, h.1, hat⟩
  | call _ hcall ih =>
    obtain ⟨p, hp, hat⟩ := ih
    obtain ⟨q, hq, hat'⟩ := hsim _ _ _ _ hat hcall
    exact ⟨q, h.2 p hp _ hq, hat'⟩

/-- **Final**: no call that really happens leads a case out of a final state. -/
theorem final_stays {S : Sys α β} (hsim : S.Simulates) {start : Nat × β} {P : List (Nat × β)}
    {F : List Nat} (hr : S.reachOk start P = true) (hf : S.finalOk F P = true)
    {c c' : Nat × α} {r : Nat} (hc : S.Reaches start c) (hF : c.1 ∈ F) (hcall : S.call c r c') :
    c'.1 = c.1 := by
  obtain ⟨p, hp, hat⟩ := reaches_mem hsim hr hc
  obtain ⟨q, hq, hat'⟩ := hsim _ _ _ _ hat hcall
  simp only [finalOk, List.all_eq_true] at hf
  have h := hf p hp
  simp only [Bool.or_eq_true, Bool.not_eq_true', List.all_eq_true, beq_iff_eq] at h
  rcases h with h | h
  · have : F.contains p.1 = true := by rw [hat.1]; exact List.contains_iff_mem.2 hF
    rw [this] at h
    exact absurd h (by simp)
  · rw [← hat'.1, h _ hq, hat.1]

theorem reachesAfter_mem {S : Sys α β} (hsim : S.Simulates) {start : Nat × β} {A B : List Nat}
    {Q : List ((Nat × β) × Bool)} (h : S.neverOk start A B Q = true) :
    ∀ {c b}, S.ReachesAfter start B c b → ∃ p, (p, b) ∈ Q ∧ S.At p c := by
  simp only [neverOk, Bool.and_eq_true, List.contains_iff_mem, List.all_eq_true] at h
  intro c b hc
  induction hc with
  | start hat => exact ⟨start, by rw [← hat.1]; exact h.1, hat⟩
  | call _ hcall ih =>
    obtain ⟨p, hp, hat⟩ := ih
    obtain ⟨q, hq, hat'⟩ := hsim _ _ _ _ hat hcall
    refine ⟨q, ?_, hat'⟩
    have := (h.2 _ hp).2 _ hq
    rw [hat'.1] at this
    exact this

/-- **Never**: once a case has been in a state of `B`, it is never in a state of `A`. -/
theorem never_after {S : Sys α β} (hsim : S.Simulates) {start : Nat × β} {A B : List Nat}
    {Q : List ((Nat × β) × Bool)} (h : S.neverOk start A B Q = true) {c : Nat × α}
    (hc : S.ReachesAfter start B c true) : c.1 ∉ A := by
  obtain ⟨p, hp, hat⟩ := reachesAfter_mem hsim h hc
  simp only [neverOk, Bool.and_eq_true, List.all_eq_true] at h
  have h1 := (h.2 _ hp).1
  intro hA
  rw [← hat.1] at hA
  simp at h1
  exact h1 hA

theorem reachesCounting_mem {S : Sys α β} (hsim : S.Simulates) {start : Nat × β}
    {counts : Nat → Bool} {Q : List ((Nat × β) × Nat)} (h : S.onceOk start counts Q = true) :
    ∀ {c n}, S.ReachesCounting start counts c n → ∃ p, (p, n) ∈ Q ∧ S.At p c := by
  simp only [onceOk, Bool.and_eq_true, List.contains_iff_mem, List.all_eq_true] at h
  intro c n hc
  induction hc with
  | start hat => exact ⟨start, h.1, hat⟩
  | call _ hcall ih =>
    obtain ⟨p, hp, hat⟩ := ih
    obtain ⟨q, hq, hat'⟩ := hsim _ _ _ _ hat hcall
    exact ⟨q, (h.2 _ hp).2 _ hq, hat'⟩

/-- **Once**: no case makes two of the counted calls. -/
theorem once_below_two {S : Sys α β} (hsim : S.Simulates) {start : Nat × β} {counts : Nat → Bool}
    {Q : List ((Nat × β) × Nat)} (h : S.onceOk start counts Q = true) {c : Nat × α} {n : Nat}
    (hc : S.ReachesCounting start counts c n) : n < 2 := by
  obtain ⟨p, hp, _⟩ := reachesCounting_mem hsim h hc
  simp only [onceOk, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at h
  exact (h.2 _ hp).1

end Sys

/-! ## The carried values -/

/-- What a row writes into a carried value: the carried input itself, a value written in the row
    (for a truth value or an enum's value, its place among the item's words), or a value the rule
    computes, by its name. -/
inductive CMove where
  | stay
  | lit : Rat → CMove
  | expr : String → CMove
  deriving Repr, Inhabited

/-- What is carried. -/
inductive CKind where
  /-- A number: the range the rule declares for it — the door takes nothing outside it — the step
      its coordinates are cut on, and the grid the output that carries it on rounds to (§7.2:
      every numeric output declares one, E104). -/
  | num (lo hi step grid : Rat)
  /-- A truth value (`false`, `true`) or an enum's value, by its place among these words. -/
  | words (ws : List String)
  deriving Repr, Inhabited

/-- One value a machine carries besides its state. -/
structure CItem where
  /-- The input it is carried in, as the table and the expressions name it. -/
  input : String
  /-- The axis of the table it is read on, if the table reads it. -/
  axis : Option Nat
  kind : CKind
  /-- Its coordinates, each a closed interval of values: every value of a number walked whole, the
      intervals it was cut into otherwise, and a word's place `(i, i)`. -/
  coords : List (Rat × Rat)
  /-- What each row writes into it, by row number. -/
  move : Nat → Option CMove

/-- A carried value at a coordinate: inside its interval; `none` is outside the range, which only a
    number can be. -/
def CItem.holdsAt (it : CItem) : Option Nat → Rat → Prop
  | some ci, v => ∃ a b, it.coords[ci]? = some (a, b) ∧ a ≤ v ∧ v ≤ b
  | none, v =>
    match it.kind with
    | .num lo hi _ _ => v < lo ∨ hi < v
    | .words _ => False

/-- Every carried value at its coordinate. -/
def PlaceHolds : List CItem → List (Option Nat) → List Rat → Prop
  | [], [], [] => True
  | it :: its, c :: cs, v :: vs => it.holdsAt c v ∧ PlaceHolds its cs vs
  | _, _, _ => False

/-- A machine that carries more than its state, as the certificate lays it on the rows of the table
    that decides the state. -/
structure Model where
  /-- The state's machine: the table, the state's axis, each row's move of the state. -/
  m : Machine
  /-- Per axis of the table: what each coordinate stands for, where it stands for a number. -/
  coords : List (List (Option Coord))
  /-- Per axis: what each coordinate is written as. -/
  labels : List (List String)
  /-- Per axis: the column it reads. -/
  columns : List String
  /-- The inputs' declared ranges, by name: the door takes no value outside them. -/
  ranges : List (String × Span2)
  /-- The values the rule computes, with their expressions. -/
  values : List (String × Expr)
  items : List CItem

namespace Model

def coordAt (K : Model) (i c : Nat) : Option Coord :=
  match K.coords[i]? with
  | some cs =>
    match cs[c]? with
    | some x => x
    | none => none
  | none => none

def labelAt (K : Model) (i c : Nat) : Option String :=
  match K.labels[i]? with
  | some ls => ls[c]?
  | none => none

end Model

/-- `v'` is what an output that rounds to the grid `g` can make of `w`: a whole number of `g`
    between `w` rounded down to the grid and `w` rounded up to it. Each of the five modes gives one of
    these (§7.2), and which one does not matter here. -/
def roundsTo (g w v' : Rat) : Prop := onGrid g v' ∧ floorTo g w ≤ v' ∧ v' ≤ ceilTo g w

/-- The call is made with `v` for this carried value: a number is the carried input's value at the
    call, and where the table reads the input as a column, the point's coordinate there holds it —
    a number inside the coordinate, a word as what the coordinate is written as. -/
def CItem.passed (it : CItem) (K : Model) (p : Point) (env : String → Option UVal) (v : Rat) : Prop :=
  (match it.kind with
   | .num .. => ∃ t, env it.input = some (v, t)
   | .words _ => True) ∧
  (match it.axis with
   | none => True
   | some ax => ∃ c, p[ax]? = some c ∧
       match it.kind with
       | .num .. => ∃ x, K.coordAt ax c = some x ∧ x.holds v
       | .words ws => ∃ (i : Nat) (w : String), v = (i : Rat) ∧ ws[i]? = some w ∧ K.labelAt ax c = some w)

/-- What a call through row `r` carries on, from `v` to `v'`: the output's rounding of what the row
    writes — the carried input itself, a value written in the row, or a computed value. A word is
    carried on as it is. A row with no move, and a word the row computes, are shapes the certificate
    does not lay out: the program refuses a section that has one (`CItem.okB`), so no call is left out
    by them. -/
def CItem.next (it : CItem) (env : String → Option UVal) (r : Nat) (v v' : Rat) : Prop :=
  match it.kind, it.move r with
  | .num _ _ _ g, some .stay => roundsTo g v v'
  | .num _ _ _ g, some (.lit x) => roundsTo g x v'
  | .num _ _ _ g, some (.expr n) => ∃ w t, env n = some (w, t) ∧ roundsTo g w v'
  | .words _, some .stay => v' = v
  | .words _, some (.lit x) => v' = x
  | _, _ => False

/-- Every carried value of a call: what the call is made with, and what it carries on. -/
def CallItems (K : Model) (p : Point) (env : String → Option UVal) (r : Nat) :
    List CItem → List Rat → List Rat → Prop
  | [], [], [] => True
  | it :: its, v :: vs, v' :: vs' => it.passed K p env v ∧ it.next env r v v' ∧ CallItems K p env r its vs vs'
  | _, _, _ => False

namespace Model

/-- **A call that really happens**, from the configuration `c` through the row numbered `r` to `c'`:
    a point of the space at `c`'s state that the row's box takes, under `first` no earlier row's box
    taking it; the values of the rule's names behind it — each numeric axis's coordinate holding its
    column's value, every input inside the range the rule declares for it, every value the rule
    computes its expression's value (with a rounding that `RoundsWell` describes); the state moved as
    the row moves it, and each carried value made with `c`'s value and carried on as the row says. -/
def call (K : Model) (c : Nat × List Rat) (r : Nat) (c' : Nat × List Rat) : Prop :=
  ∃ (row : Row) (p : Point) (env : String → Option UVal) (rnd : Rat → Rat → Rat) (mv : Move),
    row ∈ K.m.table.rows ∧ row.index = r ∧
    inSpace K.m.table.arities p = true ∧ K.m.atState p c.1 ∧ inBox row.box p = true ∧
    (K.m.table.policy = Policy.first →
      ∀ q ∈ K.m.table.rows, q.index < row.index → inBox q.box p = false) ∧
    K.m.move row.index = some mv ∧ c'.1 = mv.apply c.1 ∧
    RoundsWell rnd ∧
    (∀ i ci x n, p[i]? = some ci → K.coordAt i ci = some x → K.columns[i]? = some n →
      ∃ w t, env n = some (w, t) ∧ x.holds w) ∧
    (∀ n I w t, List.lookup n K.ranges = some I → env n = some (w, t) → inSpan I w) ∧
    (∀ n e, (n, e) ∈ K.values → env n = eval rnd env e) ∧
    CallItems K p env row.index K.items c.2 c'.2

end Model

/-! ## The calls the check counts from a place -/

/-- Whether a coordinate of the table's axis can hold a value in `[a, b]`. -/
def coordMeets : Coord → Rat → Rat → Bool
  | .exactly w, a, b => decide (a ≤ w) && decide (w ≤ b)
  | .between lo hi, a, b =>
    (match lo with | some l => decide (l < b) | none => true) &&
    (match hi with | some h => decide (a < h) | none => true)

theorem coordMeets_of_holds {x : Coord} {v a b : Rat} (hx : x.holds v) (ha : a ≤ v) (hb : v ≤ b) :
    coordMeets x a b = true := by
  cases x with
  | exactly w =>
    simp only [Coord.holds] at hx
    subst hx
    simp [coordMeets, ha, hb]
  | between lo hi =>
    obtain ⟨hlo, hhi⟩ := hx
    simp only [coordMeets, Bool.and_eq_true]
    constructor
    · cases lo with
      | none => rfl
      | some l => exact decide_eq_true (ltle (hlo l rfl) hb)
    · cases hi with
      | none => rfl
      | some h => exact decide_eq_true (lelt ha (hhi h rfl))

namespace Model

/-- Whether coordinate `x` of the table's axis `ax` can hold a value of item `it` at the item's
    coordinate `ci`: for a number the two meet, for a word `x` is written as the item's word. -/
def coordFits (K : Model) (it : CItem) (ax x ci : Nat) : Bool :=
  match it.kind with
  | .num .. =>
    match it.coords[ci]?, K.coordAt ax x with
    | some (a, b), some co => coordMeets co a b
    | some _, none => true
    | none, _ => false
  | .words ws =>
    match ws[ci]?, K.labelAt ax x with
    | some w, some l => w == l
    | _, _ => false

/-- Whether a row's box can take a configuration with this carried value at coordinate `c`. -/
def meets (K : Model) (row : Row) (it : CItem) (c : Option Nat) : Bool :=
  match c, it.axis with
  | none, _ => false
  | some _, none => true
  | some ci, some ax =>
    match row.box[ax]? with
    | some xs => xs.any (fun x => K.coordFits it ax x ci)
    | none => false

def meetsAll (K : Model) (row : Row) (cc : List (Option Nat)) : Bool :=
  (K.items.zip cc).all (fun ic => K.meets row ic.1 ic.2)

/-- The carried value read on axis `ai`, with its coordinate at the place. -/
def itemOn (K : Model) (cc : List (Option Nat)) (ai : Nat) : Option (CItem × Option Nat) :=
  (K.items.zip cc).find? (fun ic => ic.1.axis == some ai)

/-- Under `policy first`, row `r0` answers every call row `row` could take at a place, on its own:
    it takes the place's state, on each carried value's axis every coordinate that can hold the value
    at the place, and on every other axis all that `row` takes. -/
def covers (K : Model) (r0 row : Row) (s : Nat) (cc : List (Option Nat)) : Bool :=
  K.m.rowAt r0 s && (r0.box.length == row.box.length) &&
    (List.range row.box.length).all (fun ai =>
      (K.m.axis == some ai) ||
        (match K.itemOn cc ai, r0.box[ai]? with
         | some (it, some ci), some a0 =>
           (List.range ((K.m.table.arities[ai]?).getD 0)).all (fun x =>
             !K.coordFits it ai x ci || a0.contains x)
         | none, some a0 =>
           match row.box[ai]? with
           | some a1 => a1.all (fun x => a0.contains x)
           | none => false
         | _, _ => false))

def firstCovered (K : Model) (row : Row) (s : Nat) (cc : List (Option Nat)) : Bool :=
  (K.m.table.policy == Policy.first) &&
    K.m.table.rows.any (fun r0 => decide (r0.index < row.index) && K.covers r0 row s cc)

end Model

/-- One pass over the values the rule computes: each one not yet bounded whose expression the names
    bounded so far give an interval, in the order the document lists them. -/
def addOne (acc : List (String × Span2)) (ne : String × Expr) : List (String × Span2) :=
  match List.lookup ne.1 acc with
  | some _ => acc
  | none =>
    match interval (fun n => List.lookup n acc) (fun _ _ => false) ne.2 with
    | some I => acc ++ [(ne.1, I)]
    | none => acc

def addKnown (values : List (String × Expr)) (acc : List (String × Span2)) : List (String × Span2) :=
  values.foldl addOne acc

def knownAfter (values : List (String × Expr)) : Nat → List (String × Span2) → List (String × Span2)
  | 0, acc => acc
  | k + 1, acc => knownAfter values k (addKnown values acc)

/-- Each carried number at the place's coordinate. -/
def carriedRanges : List CItem → List (Option Nat) → List (String × Span2)
  | it :: its, c :: cs =>
    (match it.kind, c with
     | .num .., some ci =>
       match it.coords[ci]? with
       | some I => [(it.input, I)]
       | none => []
     | _, _ => []) ++ carriedRanges its cs
  | _, _ => []

/-- The lower of two low ends and the higher of two high ends, where both are there. -/
def joinLo : Option Rat → Option Rat → Option Rat
  | some a, some b => some (rmin a b)
  | _, _ => none

def joinHi : Option Rat → Option Rat → Option Rat
  | some a, some b => some (rmax a b)
  | _, _ => none

namespace Model

/-- What coordinate `x` of axis `ai` spans, where it stands for a number. -/
def spanAt (K : Model) (ai x : Nat) : Ival :=
  match K.coordAt ai x with
  | some co => co.span
  | none => (none, none)

/-- What the coordinates a row's box takes on axis `ai` span together. -/
def boxHull (K : Model) (ai : Nat) : List Nat → Ival
  | [] => (none, none)
  | [x] => K.spanAt ai x
  | x :: xs => (joinLo (K.spanAt ai x).1 (K.boxHull ai xs).1, joinHi (K.spanAt ai x).2 (K.boxHull ai xs).2)

/-- **What a computed value comes to over a place, through a row**: its expression's interval over
    the inputs' declared ranges with each carried number at the place's coordinate, as many passes as
    there are values; narrowed to the row's box where the table reads the value as a column. -/
def image (K : Model) (row : Row) (cc : List (Option Nat)) (n : String) : Option Span2 :=
  match List.lookup n (knownAfter K.values (K.values.length + 1) (carriedRanges K.items cc ++ K.ranges)) with
  | none => none
  | some I =>
    match K.columns.idxOf? n with
    | none => some I
    | some ai =>
      match row.box[ai]? with
      | some xs => some (narrowTo I (some (K.boxHull ai xs)))
      | none => some I

end Model

/-- Every coordinate of a carried number, and outside its range. -/
def CItem.everywhere (it : CItem) : List (Option Nat) :=
  (List.range it.coords.length).map some ++ [none]

/-- The coordinates an interval `[A, B]` meets, and outside the range where it reaches past it. -/
def CItem.meeting (it : CItem) (lo hi A B : Rat) : List (Option Nat) :=
  ((List.range it.coords.length).filter (fun ci =>
      match it.coords[ci]? with
      | some (a, b) => decide (a ≤ B) && decide (A ≤ b)
      | none => false)).map some ++
    (if decide (A < lo) || decide (hi < B) then [none] else [])

/-- Where a call through row `r` can carry a value on to, from coordinate `c`: every coordinate the
    output's rounding of what the row writes can land on, the written value bounded by the
    coordinate (`stay`), by itself (a value written in the row), or by `img` (a computed one). -/
def CItem.nextCoords (it : CItem) (img : String → Option Span2) (r : Nat) (c : Option Nat) :
    List (Option Nat) :=
  match it.kind, it.move r with
  | .num lo hi _ g, some .stay =>
    match c with
    | some ci =>
      match it.coords[ci]? with
      | some (a, b) => it.meeting lo hi (floorTo g a) (ceilTo g b)
      | none => it.everywhere
    | none => it.everywhere
  | .num lo hi _ g, some (.lit x) => it.meeting lo hi (floorTo g x) (ceilTo g x)
  | .num lo hi _ g, some (.expr n) =>
    match img n with
    | some I => it.meeting lo hi (floorTo g I.1) (ceilTo g I.2)
    | none => it.everywhere
  | .words _, some .stay => [c]
  | .words _, some (.lit x) =>
    ((List.range it.coords.length).filter (fun ci =>
      match it.coords[ci]? with
      | some (a, b) => decide (a ≤ x) && decide (x ≤ b)
      | none => false)).map some
  | _, _ => []

def nextListsOf (img : String → Option Span2) (r : Nat) :
    List CItem → List (Option Nat) → List (List (Option Nat))
  | it :: its, c :: cs => it.nextCoords img r c :: nextListsOf img r its cs
  | _, _ => []

/-- Every way of taking one element of each list. -/
def combos {γ : Type} : List (List γ) → List (List γ)
  | [] => [[]]
  | l :: ls => l.flatMap (fun x => (combos ls).map (fun rest => x :: rest))

namespace Model

def nextLists (K : Model) (row : Row) (cc : List (Option Nat)) : List (List (Option Nat)) :=
  nextListsOf (K.image row cc) row.index K.items cc

/-- **The calls the check counts from a place**: none from outside a range (the door takes no such
    value); otherwise, through every row whose box takes the place's state and can take each carried
    value there — under `first`, not one an earlier row covers there on its own — to the state the row
    moves to and every combination of the coordinates each carried value can land on. -/
def succs (K : Model) (pl : Nat × List (Option Nat)) : List (Nat × (Nat × List (Option Nat))) :=
  if pl.2.contains none then [] else
  K.m.table.rows.flatMap (fun row =>
    if K.m.rowAt row pl.1 && K.meetsAll row pl.2 && !K.firstCovered row pl.1 pl.2 then
      match K.m.move row.index with
      | some mv => (combos (K.nextLists row pl.2)).map (fun cc' => (row.index, (mv.apply pl.1, cc')))
      | none => []
    else [])

/-- The model as a system of calls and places. -/
def sys (K : Model) : Sys (List Rat) (List (Option Nat)) where
  call := K.call
  holds := PlaceHolds K.items
  succs := K.succs

end Model

/-! ## What the program checks of the section -/

/-- A number's coordinates tile its range on its step: from `lo` to `hi`, each ending on the step's
    grid where the next one starts a step further on, none running backwards. -/
def tilesB (st lo hi : Rat) : List (Rat × Rat) → Bool
  | [] => false
  | [ab] => ab.1 == lo && ab.2 == hi && decide (ab.1 ≤ ab.2) && gridB st ab.1 && gridB st ab.2
  | ab :: cd :: rest =>
    ab.1 == lo && decide (ab.1 ≤ ab.2) && gridB st ab.1 && gridB st ab.2 &&
      tilesB st (ab.2 + st) hi (cd :: rest)

/-- What the proofs need of one carried value, checked: a number's step and grid positive, the grid a
    whole number of steps, the coordinates tiling the range the rule declares for its input, a move on
    every row; a word's coordinates its places, and every word a row writes one of them. -/
def CItem.okB (it : CItem) (ranges : List (String × Span2)) (rows : List Row) : Bool :=
  match it.kind with
  | .num lo hi st g =>
    decide (0 < st) && decide (0 < g) && gridB st g && tilesB st lo hi it.coords &&
      (List.lookup it.input ranges == some (lo, hi)) &&
      rows.all (fun r => (it.move r.index).isSome)
  | .words ws =>
    (it.coords == (List.range ws.length).map (fun (i : Nat) => (((i : Nat) : Rat), ((i : Nat) : Rat)))) &&
      rows.all (fun r =>
        match it.move r.index with
        | some .stay => true
        | some (.lit x) => it.coords.any (fun ab => ab.1 == x && ab.2 == x)
        | _ => false)

def Model.okB (K : Model) : Bool := K.items.all (fun it => it.okB K.ranges K.m.table.rows)

/-! ## The simulation -/

theorem tilesB_lo_le_hi {st : Rat} (hst : 0 < st) :
    ∀ {lo hi : Rat} {cs : List (Rat × Rat)}, tilesB st lo hi cs = true → lo ≤ hi
  | _, _, [], h => by simp [tilesB] at h
  | lo, hi, [ab], h => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨h1, h2⟩, h3⟩, _⟩, _⟩ := h
    rw [← h1, ← h2]
    exact h3
  | lo, hi, ab :: cd :: rest, h => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨h1, h2⟩, _⟩, _⟩, h5⟩ := h
    have := tilesB_lo_le_hi hst h5
    rw [← h1]
    exact Rat.le_trans h2 (Rat.le_trans (Rat.le_of_lt (lt_add_pos hst)) this)
where
  lt_add_pos {a s : Rat} (hs : 0 < s) : a < a + s := by
    have := (Rat.add_lt_add_right (c := a)).2 hs
    rwa [Rat.zero_add, Rat.add_comm] at this

/-- **The tiling covers the grid**: every value on the step's grid inside the range is in a
    coordinate. -/
theorem tilesB_cover {st : Rat} (hst : 0 < st) {v : Rat} (hv : onGrid st v) :
    ∀ {lo hi : Rat} {cs : List (Rat × Rat)}, tilesB st lo hi cs = true → lo ≤ v → v ≤ hi →
      ∃ (ci : Nat) (a b : Rat), cs[ci]? = some (a, b) ∧ a ≤ v ∧ v ≤ b
  | _, _, [], h, _, _ => by simp [tilesB] at h
  | lo, hi, [ab], h, hlo, hhi => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨h1, h2⟩, _⟩, _⟩, _⟩ := h
    exact ⟨0, ab.1, ab.2, by simp, h1 ▸ hlo, h2 ▸ hhi⟩
  | lo, hi, ab :: cd :: rest, h, hlo, hhi => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨h1, _⟩, _⟩, h4⟩, h5⟩ := h
    by_cases hb : v ≤ ab.2
    · exact ⟨0, ab.1, ab.2, by simp, h1 ▸ hlo, hb⟩
    · have hlt : ab.2 < v := Rat.not_le.1 hb
      have hnext := grid_succ_le hst (onGrid_of_gridB h4) hv hlt
      obtain ⟨ci, a, b, hci, ha, hb'⟩ := tilesB_cover hst hv h5 hnext hhi
      exact ⟨ci + 1, a, b, by simpa using hci, ha, hb'⟩

/-- Every coordinate of a tiling lies inside the range. -/
theorem tilesB_within {st : Rat} (hst : 0 < st) :
    ∀ {lo hi : Rat} {cs : List (Rat × Rat)} {ci : Nat} {a b : Rat}, tilesB st lo hi cs = true →
      cs[ci]? = some (a, b) → lo ≤ a ∧ b ≤ hi
  | _, _, [], _, _, _, h, _ => by simp [tilesB] at h
  | lo, hi, [(a0, b0)], ci, a, b, h, hci => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨h1, h2⟩, _⟩, _⟩, _⟩ := h
    cases ci with
    | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq, Prod.mk.injEq] at hci
      obtain ⟨ha, hb⟩ := hci
      refine ⟨?_, ?_⟩
      · rw [← ha, h1]; exact Rat.le_refl
      · rw [← hb, h2]; exact Rat.le_refl
    | succ k => simp at hci
  | lo, hi, (a0, b0) :: cd :: rest, ci, a, b, h, hci => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨h1, h2⟩, _⟩, _⟩, h5⟩ := h
    have hstep : b0 < b0 + st := tilesB_lo_le_hi.lt_add_pos hst
    cases ci with
    | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq, Prod.mk.injEq] at hci
      obtain ⟨ha, hb⟩ := hci
      refine ⟨?_, ?_⟩
      · rw [← ha, h1]; exact Rat.le_refl
      · rw [← hb]; exact Rat.le_trans (Rat.le_of_lt hstep) (tilesB_lo_le_hi hst h5)
    | succ k =>
      obtain ⟨hlo, hhi⟩ := tilesB_within hst h5 (by simpa using hci)
      refine ⟨?_, hhi⟩
      rw [← h1]
      exact Rat.le_trans h2 (Rat.le_trans (Rat.le_of_lt hstep) hlo)

/-! ### Every interval the check works out over a place holds the call's value -/

/-- Every name an interval list bounds has its value inside that interval. -/
def Good (env : String → Option UVal) (acc : List (String × Span2)) : Prop :=
  ∀ n I w t, List.lookup n acc = some I → env n = some (w, t) → inSpan I w

theorem good_append {env : String → Option UVal} {acc acc' : List (String × Span2)}
    (h1 : Good env acc) (h2 : Good env acc') : Good env (acc ++ acc') := by
  intro n I w t hl hv
  rw [List.lookup_append] at hl
  cases h : List.lookup n acc with
  | some J =>
    rw [h, Option.some_or] at hl
    cases hl
    exact h1 n _ w t h hv
  | none =>
    rw [h, Option.none_or] at hl
    exact h2 n I w t hl hv

theorem good_single {env : String → Option UVal} {n : String} {I : Span2}
    (h : ∀ w t, env n = some (w, t) → inSpan I w) : Good env [(n, I)] := by
  intro m J w t hl hv
  cases hmn : (m == n) with
  | true =>
    have hm : m = n := beq_iff_eq.1 hmn
    simp [List.lookup_cons, hmn] at hl
    subst hm
    subst hl
    exact h w t hv
  | false => simp [List.lookup_cons, hmn] at hl

theorem good_nil {env : String → Option UVal} : Good env [] := by
  intro n I w t hl
  simp at hl

theorem good_addOne {env : String → Option UVal} {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    {values : List (String × Expr)} (hdef : ∀ n e, (n, e) ∈ values → env n = eval rnd env e)
    {acc : List (String × Span2)} {ne : String × Expr} (hne : ne ∈ values) (h : Good env acc) :
    Good env (addOne acc ne) := by
  obtain ⟨n, e⟩ := ne
  unfold addOne
  split
  · exact h
  · split
    · rename_i I hI
      refine good_append h (good_single ?_)
      intro w t hv
      have hev : eval rnd env e = some (w, t) := by rw [← hdef n e hne, hv]
      exact eval_mem_interval hrnd (fun m J w' t' hJ hw' => h m J w' t' hJ hw')
        (fun _ _ _ _ _ _ hf => by simp at hf) e I w t hI hev
    · exact h

theorem good_foldl_addOne {env : String → Option UVal} {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    {values : List (String × Expr)} (hdef : ∀ n e, (n, e) ∈ values → env n = eval rnd env e) :
    ∀ (vs : List (String × Expr)) (acc : List (String × Span2)), (∀ ne ∈ vs, ne ∈ values) →
      Good env acc → Good env (vs.foldl addOne acc)
  | [], _, _, h => h
  | ne :: vs, acc, hvs, h =>
    good_foldl_addOne hrnd hdef vs (addOne acc ne) (fun x hx => hvs x (List.mem_cons_of_mem _ hx))
      (good_addOne hrnd hdef (hvs ne (by simp)) h)

theorem good_knownAfter {env : String → Option UVal} {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    {values : List (String × Expr)} (hdef : ∀ n e, (n, e) ∈ values → env n = eval rnd env e) :
    ∀ (k : Nat) {acc : List (String × Span2)}, Good env acc → Good env (knownAfter values k acc)
  | 0, _, h => h
  | k + 1, _, h => good_knownAfter hrnd hdef k (good_foldl_addOne hrnd hdef values _ (fun _ h => h) h)

theorem placeHolds_length : ∀ {its : List CItem} {cs : List (Option Nat)} {vs : List Rat},
    PlaceHolds its cs vs → cs.length = its.length
  | [], [], [], _ => rfl
  | [], _ :: _, _, h => by simp [PlaceHolds] at h
  | [], [], _ :: _, h => by simp [PlaceHolds] at h
  | _ :: _, [], _, h => by simp [PlaceHolds] at h
  | _ :: _, _ :: _, [], h => by simp [PlaceHolds] at h
  | _ :: _, _ :: _, _ :: _, h => by
    simp only [PlaceHolds] at h
    simp [placeHolds_length h.2]

/-- What a configuration at a place and a call made from it say of each carried value, together. -/
theorem holds_passed {K : Model} {p : Point} {env : String → Option UVal} {r : Nat} :
    ∀ {its : List CItem} {cs : List (Option Nat)} {vs vs' : List Rat} {it : CItem} {c : Option Nat},
      PlaceHolds its cs vs → CallItems K p env r its vs vs' → (it, c) ∈ its.zip cs →
        ∃ v v', it.holdsAt c v ∧ it.passed K p env v ∧ it.next env r v v'
  | [], _, _, _, _, _, _, _, h => by simp at h
  | _ :: _, [], _, _, _, _, hp, _, _ => by simp [PlaceHolds] at hp
  | _ :: _, _ :: _, [], _, _, _, hp, _, _ => by simp [PlaceHolds] at hp
  | _ :: _, _ :: _, _ :: _, [], _, _, _, hc, _ => by simp [CallItems] at hc
  | it0 :: its, c0 :: cs, v0 :: vs, v0' :: vs', it, c, hp, hc, hmem => by
    simp only [PlaceHolds] at hp
    simp only [CallItems] at hc
    simp only [List.zip_cons_cons, List.mem_cons, Prod.mk.injEq] at hmem
    rcases hmem with ⟨rfl, rfl⟩ | hmem
    · exact ⟨v0, v0', hp.1, hc.1, hc.2.1⟩
    · exact holds_passed hp.2 hc.2.2 hmem

theorem good_carriedRanges {K : Model} {p : Point} {env : String → Option UVal} {r : Nat} :
    ∀ {its : List CItem} {cs : List (Option Nat)} {vs vs' : List Rat},
      PlaceHolds its cs vs → CallItems K p env r its vs vs' → Good env (carriedRanges its cs)
  | [], _, _, _, _, _ => by simp only [carriedRanges]; exact good_nil
  | _ :: _, [], _, _, hp, _ => by simp [PlaceHolds] at hp
  | _ :: _, _ :: _, [], _, hp, _ => by simp [PlaceHolds] at hp
  | _ :: _, _ :: _, _ :: _, [], _, hc => by simp [CallItems] at hc
  | it :: its, c :: cs, v :: vs, v' :: vs', hp, hc => by
    simp only [PlaceHolds] at hp
    simp only [CallItems] at hc
    simp only [carriedRanges]
    refine good_append ?_ (good_carriedRanges hp.2 hc.2.2)
    obtain ⟨hpass, _⟩ := hc.1
    cases hk : it.kind with
    | words ws => exact good_nil
    | num lo hi st g =>
      cases c with
      | none => exact good_nil
      | some ci =>
        cases hci : it.coords[ci]? with
        | none => simp only [hci]; exact good_nil
        | some I =>
          simp only [hci]
          refine good_single ?_
          intro w t hw
          simp only [hk] at hpass
          obtain ⟨t0, ht0⟩ := hpass
          rw [ht0] at hw
          simp only [Option.some.injEq, Prod.mk.injEq] at hw
          obtain ⟨rfl, _⟩ := hw
          obtain ⟨a, b, hab, ha, hb⟩ := hp.1
          rw [hab] at hci
          cases hci
          exact ⟨ha, hb⟩

theorem idxOf?_getElem? {l : List String} {n : String} {i : Nat} (h : l.idxOf? n = some i) :
    l[i]? = some n := by
  obtain ⟨hi, heq, _⟩ := List.idxOf?_eq_some_iff.1 h
  rw [List.getElem?_eq_getElem hi, heq]

theorem inIval_joinLeft {I J : Ival} {w : Rat} (h : inIval I w) : inIval (joinLo I.1 J.1, joinHi I.2 J.2) w := by
  obtain ⟨h1, h2⟩ := h
  refine ⟨fun a ha => ?_, fun b hb => ?_⟩
  · cases h3 : I.1 <;> cases h4 : J.1 <;> simp [joinLo, h3, h4] at ha
    rename_i x y
    rw [← ha]
    exact Rat.le_trans (rmin_le_left _ _) (h1 x h3)
  · cases h3 : I.2 <;> cases h4 : J.2 <;> simp [joinHi, h3, h4] at hb
    rename_i x y
    rw [← hb]
    exact Rat.le_trans (h2 x h3) (left_le_rmax _ _)

theorem inIval_joinRight {I J : Ival} {w : Rat} (h : inIval J w) : inIval (joinLo I.1 J.1, joinHi I.2 J.2) w := by
  obtain ⟨h1, h2⟩ := h
  refine ⟨fun a ha => ?_, fun b hb => ?_⟩
  · cases h3 : I.1 <;> cases h4 : J.1 <;> simp [joinLo, h3, h4] at ha
    rename_i x y
    rw [← ha]
    exact Rat.le_trans (rmin_le_right _ _) (h1 y h4)
  · cases h3 : I.2 <;> cases h4 : J.2 <;> simp [joinHi, h3, h4] at hb
    rename_i x y
    rw [← hb]
    exact Rat.le_trans (h2 y h4) (right_le_rmax _ _)

theorem Model.inIval_boxHull {K : Model} {ai : Nat} {w : Rat} :
    ∀ {xs : List Nat} {x : Nat}, x ∈ xs → inIval (K.spanAt ai x) w → inIval (K.boxHull ai xs) w
  | [], _, hx, _ => by simp at hx
  | [y], x, hx, h => by
    simp only [List.mem_singleton] at hx
    subst hx
    exact h
  | y :: z :: zs, x, hx, h => by
    simp only [List.mem_cons] at hx
    show inIval (joinLo (K.spanAt ai y).1 (K.boxHull ai (z :: zs)).1,
      joinHi (K.spanAt ai y).2 (K.boxHull ai (z :: zs)).2) w
    rcases hx with rfl | hx
    · exact inIval_joinLeft h
    · exact inIval_joinRight (Model.inIval_boxHull (List.mem_cons.2 hx) h)

/-- **What a computed value comes to over a place, through a row, holds its value at every call the
    row answers from a configuration there.** -/
theorem Model.image_sound {K : Model} {row : Row} {cc : List (Option Nat)} {p : Point}
    {env : String → Option UVal} {rnd : Rat → Rat → Rat} (hrnd : RoundsWell rnd)
    (hdef : ∀ n e, (n, e) ∈ K.values → env n = eval rnd env e)
    (hgood : Good env (carriedRanges K.items cc ++ K.ranges))
    (hbehind : ∀ i ci x n, p[i]? = some ci → K.coordAt i ci = some x → K.columns[i]? = some n →
      ∃ w t, env n = some (w, t) ∧ x.holds w)
    (hbox : inBox row.box p = true) :
    ∀ n I w t, K.image row cc n = some I → env n = some (w, t) → inSpan I w := by
  intro n I w t himg hw
  unfold Model.image at himg
  split at himg
  · exact absurd himg (by simp)
  · rename_i I0 hI0
    have h0 : inSpan I0 w := good_knownAfter hrnd hdef _ hgood n I0 w t hI0 hw
    split at himg
    · cases himg; exact h0
    · rename_i ai hai
      split at himg
      · rename_i xs hxs
        cases himg
        refine inSpan_narrowTo h0 (fun J hJ => ?_)
        cases hJ
        obtain ⟨x0, hp0, hx0⟩ := inBox_getElem hbox hxs
        refine Model.inIval_boxHull (List.contains_iff_mem.1 hx0) ?_
        unfold Model.spanAt
        split
        · rename_i co hco
          obtain ⟨w', t', hw', hco'⟩ := hbehind ai x0 co n hp0 hco (idxOf?_getElem? hai)
          rw [hw] at hw'
          simp only [Option.some.injEq, Prod.mk.injEq] at hw'
          rw [hw'.1]
          exact Coord.holds_span hco'
        · exact ⟨fun a ha => by simp at ha, fun b hb => by simp at hb⟩
      · cases himg; exact h0

/-! ### Each carried value lands on a coordinate -/

theorem CItem.okB_num {it : CItem} {ranges : List (String × Span2)} {rows : List Row}
    {lo hi st g : Rat} (hk : it.kind = .num lo hi st g) (hok : it.okB ranges rows = true) :
    0 < st ∧ 0 < g ∧ gridB st g = true ∧ tilesB st lo hi it.coords = true ∧
      List.lookup it.input ranges = some (lo, hi) ∧ ∀ r ∈ rows, (it.move r.index).isSome = true := by
  simp only [CItem.okB, hk, Bool.and_eq_true, decide_eq_true_eq, beq_iff_eq, List.all_eq_true] at hok
  obtain ⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩ := hok
  exact ⟨h1, h2, h3, h4, h5, h6⟩

theorem CItem.okB_words_coords {it : CItem} {ranges : List (String × Span2)} {rows : List Row}
    {ws : List String} (hk : it.kind = .words ws) (hok : it.okB ranges rows = true)
    {ci : Nat} {a b : Rat} (h : it.coords[ci]? = some (a, b)) : a = (ci : Rat) ∧ b = (ci : Rat) := by
  simp only [CItem.okB, hk, Bool.and_eq_true, beq_iff_eq] at hok
  rw [hok.1, List.getElem?_map] at h
  by_cases hci : ci < ws.length
  · rw [List.getElem?_range hci] at h
    simp only [Option.map_some, Option.some.injEq, Prod.mk.injEq] at h
    exact ⟨h.1.symm, h.2.symm⟩
  · rw [List.getElem?_eq_none (by simpa using hci)] at h
    simp at h

theorem CItem.okB_words_lit {it : CItem} {ranges : List (String × Span2)} {rows : List Row}
    {ws : List String} (hk : it.kind = .words ws) (hok : it.okB ranges rows = true)
    {row : Row} (hrow : row ∈ rows) {x : Rat} (hm : it.move row.index = some (.lit x)) :
    ∃ ci : Nat, it.coords[ci]? = some (x, x) := by
  simp only [CItem.okB, hk, Bool.and_eq_true, List.all_eq_true] at hok
  have h := hok.2 row hrow
  rw [hm] at h
  obtain ⟨ab, hab, hx⟩ := List.any_eq_true.1 h
  simp only [Bool.and_eq_true, beq_iff_eq] at hx
  obtain ⟨ci, hci⟩ := List.mem_iff_getElem?.1 hab
  refine ⟨ci, ?_⟩
  rw [hci]
  obtain ⟨a, b⟩ := ab
  simp only at hx
  rw [hx.1, hx.2]

/-- **The coordinates an interval meets hold every value of the output's grid inside it**: one of
    the coordinates the tiling lays over the range, or outside the range. -/
theorem CItem.meeting_sound {it : CItem} {ranges : List (String × Span2)} {rows : List Row}
    {lo hi st g : Rat} (hk : it.kind = .num lo hi st g) (hok : it.okB ranges rows = true)
    {A B v : Rat} (hA : A ≤ v) (hB : v ≤ B) (hgrid : onGrid g v) :
    ∃ c' ∈ it.meeting lo hi A B, it.holdsAt c' v := by
  obtain ⟨hst, _, hstg, htile, _, _⟩ := CItem.okB_num hk hok
  by_cases hlo : v < lo
  · refine ⟨none, ?_, by simp only [CItem.holdsAt, hk]; exact Or.inl hlo⟩
    have : A < lo := lelt hA hlo
    simp [CItem.meeting, this]
  · by_cases hhi : hi < v
    · refine ⟨none, ?_, by simp only [CItem.holdsAt, hk]; exact Or.inr hhi⟩
      have : hi < B := ltle hhi hB
      simp [CItem.meeting, this]
    · obtain ⟨ci, a, b, hci, ha, hb⟩ := tilesB_cover hst (onGrid_trans (onGrid_of_gridB hstg) hgrid)
        htile (Rat.not_lt.1 hlo) (Rat.not_lt.1 hhi)
      refine ⟨some ci, ?_, ⟨a, b, hci, ha, hb⟩⟩
      simp only [CItem.meeting, List.mem_append, List.mem_map, List.mem_filter, List.mem_range]
      left
      refine ⟨ci, ⟨lt_of_getElem? hci, ?_⟩, rfl⟩
      rw [hci]
      simp [Rat.le_trans ha hB, Rat.le_trans hA hb]

/-- Every coordinate, and outside the range, hold every value of the output's grid. -/
theorem CItem.everywhere_sound {it : CItem} {ranges : List (String × Span2)} {rows : List Row}
    {lo hi st g : Rat} (hk : it.kind = .num lo hi st g) (hok : it.okB ranges rows = true)
    {v : Rat} (hgrid : onGrid g v) : ∃ c' ∈ it.everywhere, it.holdsAt c' v := by
  obtain ⟨hst, _, hstg, htile, _, _⟩ := CItem.okB_num hk hok
  by_cases hlo : v < lo
  · exact ⟨none, by simp [CItem.everywhere], by simp only [CItem.holdsAt, hk]; exact Or.inl hlo⟩
  · by_cases hhi : hi < v
    · exact ⟨none, by simp [CItem.everywhere], by simp only [CItem.holdsAt, hk]; exact Or.inr hhi⟩
    · obtain ⟨ci, a, b, hci, ha, hb⟩ := tilesB_cover hst (onGrid_trans (onGrid_of_gridB hstg) hgrid)
        htile (Rat.not_lt.1 hlo) (Rat.not_lt.1 hhi)
      refine ⟨some ci, ?_, ⟨a, b, hci, ha, hb⟩⟩
      simp only [CItem.everywhere, List.mem_append, List.mem_map, List.mem_range]
      exact Or.inl ⟨ci, lt_of_getElem? hci, rfl⟩

/-- **Where a call carries a value on to is one of the coordinates the check counts.** -/
theorem CItem.nextCoords_sound {it : CItem} {ranges : List (String × Span2)} {rows : List Row}
    (hok : it.okB ranges rows = true) {img : String → Option Span2} {env : String → Option UVal}
    (himg : ∀ n I w t, img n = some I → env n = some (w, t) → inSpan I w)
    {row : Row} (hrow : row ∈ rows) {c : Option Nat} {v v' : Rat} (hc : it.holdsAt c v)
    (hnext : it.next env row.index v v') : ∃ c' ∈ it.nextCoords img row.index c, it.holdsAt c' v' := by
  unfold CItem.next at hnext
  unfold CItem.nextCoords
  cases hk : it.kind with
  | num lo hi st g =>
    obtain ⟨_, hg0, _, _, _, _⟩ := CItem.okB_num hk hok
    cases hm : it.move row.index with
    | none => simp [hk, hm] at hnext
    | some mv =>
      cases mv with
      | stay =>
        simp only [hk, hm] at hnext ⊢
        obtain ⟨hgrid, hfl, hce⟩ := hnext
        cases c with
        | none => exact CItem.everywhere_sound hk hok hgrid
        | some ci =>
          obtain ⟨a, b, hab, ha, hb⟩ := hc
          simp only [hab]
          exact CItem.meeting_sound hk hok (Rat.le_trans (floorTo_monotone hg0 ha) hfl)
            (Rat.le_trans hce (ceilTo_monotone hg0 hb)) hgrid
      | lit x =>
        simp only [hk, hm] at hnext ⊢
        obtain ⟨hgrid, hfl, hce⟩ := hnext
        exact CItem.meeting_sound hk hok hfl hce hgrid
      | expr n =>
        simp only [hk, hm] at hnext ⊢
        obtain ⟨w, t, hw, hgrid, hfl, hce⟩ := hnext
        cases hi : img n with
        | none => exact CItem.everywhere_sound hk hok hgrid
        | some I =>
          obtain ⟨h1, h2⟩ := himg n I w t hi hw
          exact CItem.meeting_sound hk hok (Rat.le_trans (floorTo_monotone hg0 h1) hfl)
            (Rat.le_trans hce (ceilTo_monotone hg0 h2)) hgrid
  | words ws =>
    cases hm : it.move row.index with
    | none => simp [hk, hm] at hnext
    | some mv =>
      cases mv with
      | stay =>
        simp only [hk, hm] at hnext ⊢
        subst hnext
        exact ⟨c, by simp, hc⟩
      | lit x =>
        simp only [hk, hm] at hnext ⊢
        obtain ⟨ci, hci⟩ := CItem.okB_words_lit hk hok hrow hm
        refine ⟨some ci, ?_, ⟨x, x, hci, by rw [hnext]; exact Rat.le_refl,
          by rw [hnext]; exact Rat.le_refl⟩⟩
        simp only [List.mem_map, List.mem_filter, List.mem_range]
        refine ⟨ci, ⟨lt_of_getElem? hci, ?_⟩, rfl⟩
        rw [hci]
        simp
      | expr n => simp [hk, hm] at hnext

/-- Taking one element of each list. -/
def AllIn {γ : Type} : List γ → List (List γ) → Prop
  | [], [] => True
  | x :: xs, l :: ls => x ∈ l ∧ AllIn xs ls
  | _, _ => False

theorem mem_combos {γ : Type} : ∀ {xs : List γ} {ls : List (List γ)}, AllIn xs ls → xs ∈ combos ls
  | [], [], _ => by simp [combos]
  | [], _ :: _, h => by simp [AllIn] at h
  | _ :: _, [], h => by simp [AllIn] at h
  | x :: xs, l :: ls, h => by
    simp only [AllIn] at h
    simp only [combos, List.mem_flatMap, List.mem_map]
    exact ⟨x, h.1, xs, mem_combos h.2, rfl⟩

theorem nextListsOf_sound {K : Model} {p : Point} {env : String → Option UVal} {rows : List Row}
    {ranges : List (String × Span2)} {img : String → Option Span2}
    (himg : ∀ n I w t, img n = some I → env n = some (w, t) → inSpan I w)
    {row : Row} (hrow : row ∈ rows) :
    ∀ {its : List CItem} {cs : List (Option Nat)} {vs vs' : List Rat},
      (∀ it ∈ its, it.okB ranges rows = true) → PlaceHolds its cs vs →
      CallItems K p env row.index its vs vs' →
      ∃ cs', AllIn cs' (nextListsOf img row.index its cs) ∧ PlaceHolds its cs' vs'
  | [], [], [], [], _, _, _ => ⟨[], by simp [nextListsOf, AllIn], by simp [PlaceHolds]⟩
  | [], _ :: _, _, _, _, hp, _ => by simp [PlaceHolds] at hp
  | [], [], _ :: _, _, _, hp, _ => by simp [PlaceHolds] at hp
  | [], [], [], _ :: _, _, _, hc => by simp [CallItems] at hc
  | _ :: _, [], _, _, _, hp, _ => by simp [PlaceHolds] at hp
  | _ :: _, _ :: _, [], _, _, hp, _ => by simp [PlaceHolds] at hp
  | _ :: _, _ :: _, _ :: _, [], _, _, hc => by simp [CallItems] at hc
  | it :: its, c :: cs, v :: vs, v' :: vs', hoks, hp, hc => by
    simp only [PlaceHolds] at hp
    simp only [CallItems] at hc
    obtain ⟨c', hc', hhold⟩ := CItem.nextCoords_sound (hoks it (by simp)) himg hrow hp.1 hc.2.1
    obtain ⟨cs', hall, hph⟩ := nextListsOf_sound himg hrow
      (fun it' h' => hoks it' (List.mem_cons_of_mem _ h')) hp.2 hc.2.2
    exact ⟨c' :: cs', by simp only [nextListsOf, AllIn]; exact ⟨hc', hall⟩, by
      simp only [PlaceHolds]; exact ⟨hhold, hph⟩⟩

/-! ### The row is one the check counts -/

/-- Where the table reads a carried value as a column, the point's coordinate there can hold the
    value at its coordinate. -/
theorem Model.coordFits_of_passed {K : Model} {it : CItem} {ranges : List (String × Span2)}
    {rows : List Row} (hok : it.okB ranges rows = true) {p : Point} {env : String → Option UVal}
    {ax ci c : Nat} {v : Rat} (hax : it.axis = some ax) (hc : it.holdsAt (some ci) v)
    (hpass : it.passed K p env v) (hp : p[ax]? = some c) : K.coordFits it ax c ci = true := by
  obtain ⟨_, hpass⟩ := hpass
  rw [hax] at hpass
  obtain ⟨c0, hc0, hfit⟩ := hpass
  rw [hp] at hc0
  cases hc0
  obtain ⟨a, b, hab, ha, hb⟩ := hc
  unfold Model.coordFits
  cases hk : it.kind with
  | num lo hi st g =>
    simp only [hk] at hfit
    obtain ⟨x, hx, hxv⟩ := hfit
    simp only [hab, hx]
    exact coordMeets_of_holds hxv ha hb
  | words ws =>
    simp only [hk] at hfit
    obtain ⟨i, w, hvi, hwi, hl⟩ := hfit
    obtain ⟨hac, hbc⟩ := CItem.okB_words_coords hk hok hab
    have hv : v = (ci : Rat) := Rat.le_antisymm (hbc ▸ hb) (hac ▸ ha)
    have hic : i = ci := Rat.natCast_inj.1 (hvi.symm.trans hv)
    subst hic
    simp [hwi, hl]

theorem Model.meets_sound {K : Model} {it : CItem} {ranges : List (String × Span2)}
    {rows : List Row} (hok : it.okB ranges rows = true) {row : Row} {p : Point}
    {env : String → Option UVal} {ci : Nat} {v : Rat} (hbox : inBox row.box p = true)
    (hc : it.holdsAt (some ci) v) (hpass : it.passed K p env v) :
    K.meets row it (some ci) = true := by
  unfold Model.meets
  cases hax : it.axis with
  | none => rfl
  | some ax =>
    have hpass' := hpass.2
    simp only [hax] at hpass'
    obtain ⟨c, hpc, _⟩ := hpass'
    obtain ⟨xs, hxs, hcx⟩ := Machine.box_takes hbox hpc
    simp only [hxs]
    exact List.any_eq_true.2 ⟨c, List.contains_iff_mem.1 hcx,
      Model.coordFits_of_passed hok hax hc hpass hpc⟩

theorem Model.meetsAll_sound {K : Model} {row : Row} {p : Point} {env : String → Option UVal}
    {r : Nat} (hoks : ∀ it ∈ K.items, it.okB K.ranges K.m.table.rows = true)
    (hbox : inBox row.box p = true) {cc : List (Option Nat)} {xs xs' : List Rat}
    (hph : PlaceHolds K.items cc xs) (hci : CallItems K p env r K.items xs xs')
    (hnone : none ∉ cc) : K.meetsAll row cc = true := by
  unfold Model.meetsAll
  refine List.all_eq_true.2 (fun ic hic => ?_)
  obtain ⟨it, c⟩ := ic
  have hmem : it ∈ K.items := (List.of_mem_zip hic).1
  have hcmem : c ∈ cc := (List.of_mem_zip hic).2
  obtain ⟨v, _, hc, hpass, _⟩ := holds_passed hph hci hic
  cases c with
  | none => exact absurd hcmem hnone
  | some ci => exact Model.meets_sound (hoks it hmem) hbox hc hpass

/-- **The place is not outside a range**: the door takes every carried number inside its range. -/
theorem none_not_mem {K : Model} {p : Point} {env : String → Option UVal} {r : Nat}
    (hdoor : ∀ n I w t, List.lookup n K.ranges = some I → env n = some (w, t) → inSpan I w)
    (hoks : ∀ it ∈ K.items, it.okB K.ranges K.m.table.rows = true) {cc : List (Option Nat)}
    {xs xs' : List Rat} (hph : PlaceHolds K.items cc xs) (hci : CallItems K p env r K.items xs xs') :
    none ∉ cc := by
  intro hnone
  have hlen : cc.length = K.items.length := placeHolds_length hph
  obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 hnone
  have hklt : k < K.items.length := hlen ▸ lt_of_getElem? hk
  obtain ⟨it, hit⟩ : ∃ it, K.items[k]? = some it := ⟨_, List.getElem?_eq_getElem hklt⟩
  have hzip : (it, (none : Option Nat)) ∈ K.items.zip cc := by
    apply List.mem_iff_getElem?.2
    exact ⟨k, by rw [List.getElem?_zip_eq_some]; exact ⟨hit, hk⟩⟩
  obtain ⟨v, _, hc, hpass, _⟩ := holds_passed hph hci hzip
  have hmem : it ∈ K.items := List.mem_of_getElem? hit
  unfold CItem.holdsAt at hc
  cases hkind : it.kind with
  | words ws => simp [hkind] at hc
  | num lo hi st g =>
    simp only [hkind] at hc
    obtain ⟨_, _, _, _, hrange, _⟩ := CItem.okB_num hkind (hoks it hmem)
    have hp1 := hpass.1
    simp only [hkind] at hp1
    obtain ⟨t, ht⟩ := hp1
    obtain ⟨h1, h2⟩ := hdoor it.input (lo, hi) v t hrange ht
    rcases hc with hc | hc
    · exact Rat.lt_irrefl (lelt h1 hc)
    · exact Rat.lt_irrefl (ltle hc h2)

/-- **Under `first`, a row an earlier one covers at a place answers nothing there**: the earlier row's
    box takes every point the later row could be called at from a configuration at the place. -/
theorem Model.covers_sound {K : Model} (hoks : ∀ it ∈ K.items, it.okB K.ranges K.m.table.rows = true)
    {r0 row : Row} {s : Nat} {cc : List (Option Nat)} {xs xs' : List Rat} {p : Point}
    {env : String → Option UVal} {r : Nat}
    (hcov : K.covers r0 row s cc = true) (hsp : inSpace K.m.table.arities p = true)
    (hat : K.m.atState p s) (hbox : inBox row.box p = true)
    (hph : PlaceHolds K.items cc xs) (hci : CallItems K p env r K.items xs xs') :
    inBox r0.box p = true := by
  unfold Model.covers at hcov
  simp only [Bool.and_eq_true, beq_iff_eq, List.all_eq_true, List.mem_range] at hcov
  obtain ⟨⟨hrow0, hlen⟩, hall⟩ := hcov
  have hplen : row.box.length = p.length := Machine.inBox_length hbox
  refine inBox_of_getElem (hlen.trans hplen) ?_
  intro i a0 c0 ha0 hc0
  have hi : i < row.box.length := hplen ▸ lt_of_getElem? hc0
  have h := hall i hi
  simp only [Bool.or_eq_true, beq_iff_eq] at h
  rcases h with hax | h
  · unfold Machine.atState at hat
    rw [hax] at hat
    rw [hat] at hc0
    cases hc0
    unfold Machine.rowAt at hrow0
    simp only [hax, ha0] at hrow0
    exact hrow0
  · rw [ha0] at h
    cases hio : K.itemOn cc i with
    | none =>
      simp only [hio] at h
      obtain ⟨xs1, hxs1, hc1⟩ := Machine.box_takes hbox hc0
      rw [hxs1] at h
      exact List.all_eq_true.1 h c0 (List.contains_iff_mem.1 hc1)
    | some ic =>
      obtain ⟨it, c⟩ := ic
      cases c with
      | none => simp [hio] at h
      | some ci =>
        simp only [hio] at h
        have hfind := List.find?_some hio
        have hmem := List.mem_of_find?_eq_some hio
        have hax : it.axis = some i := by simpa using hfind
        obtain ⟨v, _, hc, hpass, _⟩ := holds_passed hph hci hmem
        have hitm : it ∈ K.items := (List.of_mem_zip hmem).1
        have hfit := Model.coordFits_of_passed (hoks it hitm) hax hc hpass hc0
        have hplen' : p.length = K.m.table.arities.length := inSpace_length hsp
        have hia : i < K.m.table.arities.length := hplen' ▸ lt_of_getElem? hc0
        have hn : K.m.table.arities[i]? = some (K.m.table.arities[i]'hia) := List.getElem?_eq_getElem hia
        obtain ⟨c1, hc1, hlt⟩ := inSpace_getElem hsp hn
        rw [hc0] at hc1
        cases hc1
        rw [hn] at h
        have := List.all_eq_true.1 h c0 (List.mem_range.2 hlt)
        simp only [hfit, Bool.not_true, Bool.false_or] at this
        exact this

/-- **The simulation** ("模倣の定理"): every call that really happens from a configuration at a place
    is one the check counts from that place, to a place the configuration it leads to is at. -/
theorem Model.simulates {K : Model} (hok : K.okB = true) : K.sys.Simulates := by
  have hoks : ∀ it ∈ K.items, it.okB K.ranges K.m.table.rows = true := List.all_eq_true.1 hok
  intro pl c r c' hat hcall
  obtain ⟨row, p, env, rnd, mv, hrow, hidx, hsp, hstate, hbox, hfirst, hmv, hc1, hrnd, hbehind,
    hdoor, hdef, hitems⟩ := hcall
  obtain ⟨hs, hph⟩ := hat
  have hph : PlaceHolds K.items pl.2 c.2 := hph
  have hnone : none ∉ pl.2 := none_not_mem hdoor hoks hph hitems
  have hstate' : K.m.atState p pl.1 := by rw [hs]; exact hstate
  have hrowAt : K.m.rowAt row pl.1 = true := Machine.rowAt_of_box hstate' hbox
  have hmeets : K.meetsAll row pl.2 = true := Model.meetsAll_sound hoks hbox hph hitems hnone
  have hnotcov : K.firstCovered row pl.1 pl.2 = false := by
    unfold Model.firstCovered
    cases hpol : (K.m.table.policy == Policy.first) with
    | false => rfl
    | true =>
      simp only [Bool.true_and]
      cases hany : K.m.table.rows.any (fun r0 => decide (r0.index < row.index) && K.covers r0 row pl.1 pl.2) with
      | false => rfl
      | true =>
        exfalso
        obtain ⟨r0, hr0, hr0'⟩ := List.any_eq_true.1 hany
        simp only [Bool.and_eq_true, decide_eq_true_eq] at hr0'
        have hin := Model.covers_sound hoks hr0'.2 hsp hstate' hbox hph hitems
        have hno := hfirst (beq_iff_eq.1 hpol) r0 hr0 hr0'.1
        rw [hin] at hno
        exact absurd hno (by simp)
  have himg := Model.image_sound (row := row) (cc := pl.2) hrnd hdef
    (good_append (good_carriedRanges hph hitems) hdoor) hbehind hbox
  obtain ⟨cs', hall, hph'⟩ := nextListsOf_sound himg hrow hoks hph hitems
  refine ⟨(mv.apply pl.1, cs'), ?_, ?_, hph'⟩
  · rw [← hidx]
    show (row.index, (mv.apply pl.1, cs')) ∈ K.succs pl
    unfold Model.succs
    have hcn : pl.2.contains none = false := by
      cases h : pl.2.contains none with
      | false => rfl
      | true => exact absurd (List.contains_iff_mem.1 h) hnone
    split
    · rename_i h
      rw [hcn] at h
      exact absurd h (by simp)
    · refine List.mem_flatMap.2 ⟨row, hrow, ?_⟩
      split
      · rw [hmv]
        exact List.mem_map.2 ⟨cs', mem_combos hall, rfl⟩
      · rename_i h
        exact absurd (by simp [hrowAt, hmeets, hnotcov]) h
  · show mv.apply pl.1 = c'.1
    rw [hc1, hs]

/-! ## The claims over the carried values -/

/-- A configuration's value at index `k`, at the place's coordinate there. -/
theorem placeHolds_get : ∀ {its : List CItem} {cs : List (Option Nat)} {vs : List Rat} {k : Nat}
    {it : CItem}, PlaceHolds its cs vs → its[k]? = some it →
      ∃ c v, cs[k]? = some c ∧ vs[k]? = some v ∧ it.holdsAt c v
  | [], _, _, _, _, _, h => by simp at h
  | _ :: _, [], _, _, _, hp, _ => by simp [PlaceHolds] at hp
  | _ :: _, _ :: _, [], _, _, hp, _ => by simp [PlaceHolds] at hp
  | it0 :: its, c0 :: cs, v0 :: vs, k, it, hp, hk => by
    simp only [PlaceHolds] at hp
    cases k with
    | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq] at hk
      subst hk
      exact ⟨c0, v0, rfl, rfl, hp.1⟩
    | succ k =>
      simp only [List.getElem?_cons_succ] at hk ⊢
      exact placeHolds_get hp.2 hk

/-- One `always` line: the states it speaks of (every state when the list is empty) and, for each
    carried value it names by its place among the items, the cell the value has to fit. -/
structure Always where
  when : List Nat
  tests : List (Nat × CellTest)

/-- What a cell says of a carried value: a number passes the comparisons, is one of the values or
    none of them; a word is one of the words, or none of them. -/
def CItem.cellHolds (it : CItem) (cell : CellTest) (v : Rat) : Prop :=
  match it.kind, cell with
  | _, .any => True
  | .num .., .cmp ts => ∀ t ∈ ts, t.1.holds v t.2
  | .num .., .inVals ws => ∃ w ∈ ws, v = w
  | .num .., .notInVals ws => ∀ w ∈ ws, v ≠ w
  | .words ws, .isIn ws' => ∃ (i : Nat) (w : String), v = (i : Rat) ∧ ws[i]? = some w ∧ w ∈ ws'
  | .words ws, .notIn ws' => ∃ (i : Nat) (w : String), v = (i : Rat) ∧ ws[i]? = some w ∧ w ∉ ws'
  | _, _ => False

/-- Whether every value of a coordinate fits a cell: both ends of a number's interval pass each
    comparison, the interval is one value of the set or meets none of them; a word is or is not one
    of the cell's. -/
def CItem.fits (it : CItem) (cell : CellTest) : Option Nat → Bool
  | none => false
  | some ci =>
    match it.coords[ci]? with
    | none => false
    | some (a, b) =>
      match it.kind, cell with
      | _, .any => true
      | .num .., .cmp ts => ts.all (fun t => cmpHolds t.1 a t.2 && cmpHolds t.1 b t.2)
      | .num .., .inVals ws => decide (a = b) && ws.contains a
      | .num .., .notInVals ws => ws.all (fun w => decide (w < a) || decide (b < w))
      | .words ws, .isIn ws' =>
        match ws[ci]? with
        | some w => ws'.contains w
        | none => false
      | .words ws, .notIn ws' =>
        match ws[ci]? with
        | some w => !ws'.contains w
        | none => false
      | _, _ => false

theorem cmp_between {op : Cmp} {a b v w : Rat} (ha : a ≤ v) (hb : v ≤ b)
    (h1 : cmpHolds op a w = true) (h2 : cmpHolds op b w = true) : op.holds v w := by
  cases op with
  | le => exact Rat.le_trans hb (holds_of_cmpHolds h2)
  | lt => exact lelt hb (holds_of_cmpHolds h2)
  | ge => exact Rat.le_trans (holds_of_cmpHolds h1) ha
  | gt => exact ltle (holds_of_cmpHolds h1) ha
  | eq =>
    have e1 : a = w := holds_of_cmpHolds h1
    have e2 : b = w := holds_of_cmpHolds h2
    subst e1
    exact Rat.le_antisymm (e2 ▸ hb) ha

/-- **A coordinate that fits a cell holds only values the cell says yes to.** -/
theorem CItem.fits_sound {it : CItem} {ranges : List (String × Span2)} {rows : List Row}
    (hok : it.okB ranges rows = true) {cell : CellTest} {c : Option Nat} {v : Rat}
    (hfit : it.fits cell c = true) (hc : it.holdsAt c v) : it.cellHolds cell v := by
  cases c with
  | none => simp [CItem.fits] at hfit
  | some ci =>
    obtain ⟨a, b, hab, ha, hb⟩ := hc
    simp only [CItem.fits, hab] at hfit
    unfold CItem.cellHolds
    cases hk : it.kind with
    | num lo hi st g =>
      rw [hk] at hfit
      cases cell with
      | any => trivial
      | cmp ts =>
        simp only [List.all_eq_true, Bool.and_eq_true] at hfit
        exact fun t ht => cmp_between ha hb (hfit t ht).1 (hfit t ht).2
      | inVals ws =>
        simp only [Bool.and_eq_true, decide_eq_true_eq, List.contains_iff_mem] at hfit
        exact ⟨a, hfit.2, Rat.le_antisymm (hfit.1 ▸ hb) ha⟩
      | notInVals ws =>
        simp only [List.all_eq_true, Bool.or_eq_true, decide_eq_true_eq] at hfit
        intro w hw hvw
        subst hvw
        rcases hfit v hw with h | h
        · exact Rat.lt_irrefl (ltle h ha)
        · exact Rat.lt_irrefl (lelt hb h)
      | nothing => simp at hfit
      | isIn _ => simp at hfit
      | notIn _ => simp at hfit
      | prefixOf _ => simp at hfit
      | rank _ _ => simp at hfit
    | words ws =>
      rw [hk] at hfit
      obtain ⟨hac, hbc⟩ := CItem.okB_words_coords hk hok hab
      have hv : v = (ci : Rat) := Rat.le_antisymm (hbc ▸ hb) (hac ▸ ha)
      cases cell with
      | any => trivial
      | isIn ws' =>
        cases hw : ws[ci]? with
        | none => simp [hw] at hfit
        | some w =>
          simp only [hw, List.contains_iff_mem] at hfit
          exact ⟨ci, w, hv, hw, hfit⟩
      | notIn ws' =>
        cases hw : ws[ci]? with
        | none => simp [hw] at hfit
        | some w =>
          simp only [hw, Bool.not_eq_true'] at hfit
          refine ⟨ci, w, hv, hw, fun hmem => ?_⟩
          rw [List.contains_iff_mem.2 hmem] at hfit
          exact absurd hfit (by simp)
      | nothing => simp at hfit
      | cmp _ => simp at hfit
      | inVals _ => simp at hfit
      | notInVals _ => simp at hfit
      | prefixOf _ => simp at hfit
      | rank _ _ => simp at hfit

/-- **The `always` check**: at every place in the states the line names, with every carried number
    inside its range, every value the line names fits its cell. -/
def Model.alwaysOk (K : Model) (P : List (Nat × List (Option Nat))) (al : Always) : Bool :=
  P.all (fun pl => !(al.when.isEmpty || al.when.contains pl.1) || pl.2.contains none ||
    al.tests.all (fun kt =>
      match K.items[kt.1]?, pl.2[kt.1]? with
      | some it, some c => it.fits kt.2 c
      | _, _ => false))

/-- A carried value inside its range: a number between the ends the rule declares; a word always. -/
def CItem.inRange (it : CItem) (v : Rat) : Prop :=
  match it.kind with
  | .num lo hi _ _ => lo ≤ v ∧ v ≤ hi
  | .words _ => True

/-- **Always**: in every configuration a case can reach in a state the line names, whose carried
    numbers are inside their ranges, every value the line names fits its cell. A configuration with
    a number outside its range is the range's claim (`range_holds`), not this one's (E134). -/
theorem Model.always_holds {K : Model} (hok : K.okB = true) {start : Nat × List (Option Nat)}
    {P : List (Nat × List (Option Nat))} (hr : K.sys.reachOk start P = true) {al : Always}
    (ha : K.alwaysOk P al = true) {c : Nat × List Rat} (hc : K.sys.Reaches start c)
    (hwhen : al.when = [] ∨ c.1 ∈ al.when)
    (hin : ∀ (k : Nat) (it : CItem) (v : Rat), K.items[k]? = some it → c.2[k]? = some v → it.inRange v) :
    ∀ kt ∈ al.tests, ∃ it v, K.items[kt.1]? = some it ∧ c.2[kt.1]? = some v ∧ it.cellHolds kt.2 v := by
  have hoks : ∀ it ∈ K.items, it.okB K.ranges K.m.table.rows = true := List.all_eq_true.1 hok
  obtain ⟨pl, hpl, hs, hph⟩ := Sys.reaches_mem (Model.simulates hok) hr hc
  have hph : PlaceHolds K.items pl.2 c.2 := hph
  have h := List.all_eq_true.1 ha pl hpl
  simp only [Bool.or_eq_true, Bool.not_eq_true'] at h
  have hw : (al.when.isEmpty || al.when.contains pl.1) = true := by
    rcases hwhen with hw | hw
    · simp [hw]
    · rw [hs]; simp [hw]
  have hn : pl.2.contains none = false := by
    cases hcn : pl.2.contains none with
    | false => rfl
    | true =>
      exfalso
      obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 (List.contains_iff_mem.1 hcn)
      have hlen := placeHolds_length hph
      have hklt : k < K.items.length := hlen ▸ lt_of_getElem? hk
      obtain ⟨c0, v, hc0, hv, hhold⟩ := placeHolds_get hph (List.getElem?_eq_getElem hklt)
      rw [hk] at hc0
      cases hc0
      have hr' := hin k _ v (List.getElem?_eq_getElem hklt) hv
      unfold CItem.holdsAt at hhold
      unfold CItem.inRange at hr'
      cases hkind : (K.items[k]'hklt).kind with
      | words ws => simp [hkind] at hhold
      | num lo hi st g =>
        simp only [hkind] at hhold hr'
        rcases hhold with h1 | h1
        · exact Rat.lt_irrefl (lelt hr'.1 h1)
        · exact Rat.lt_irrefl (ltle h1 hr'.2)
  rcases h with (h | h) | h
  · rw [hw] at h; exact absurd h (by simp)
  · rw [hn] at h; exact absurd h (by simp)
  · intro kt hkt
    have hkt' := List.all_eq_true.1 h kt hkt
    cases hit : K.items[kt.1]? with
    | none => simp [hit] at hkt'
    | some it =>
      obtain ⟨c0, v, hc0, hv, hhold⟩ := placeHolds_get hph hit
      rw [hit, hc0] at hkt'
      exact ⟨it, v, rfl, hv, CItem.fits_sound (hoks it (List.mem_of_getElem? hit)) hkt' hhold⟩

/-- **The range check**: no place has the value outside its range. -/
def Model.rangeOk (P : List (Nat × List (Option Nat))) (k : Nat) : Bool :=
  P.all (fun pl => pl.2[k]? != some none)

/-- **A carried number stays within its range**: in every configuration a case can reach. -/
theorem Model.range_holds {K : Model} (hok : K.okB = true) {start : Nat × List (Option Nat)}
    {P : List (Nat × List (Option Nat))} (hr : K.sys.reachOk start P = true) {k : Nat}
    (hrange : Model.rangeOk P k = true) {c : Nat × List Rat} (hc : K.sys.Reaches start c)
    {it : CItem} (hit : K.items[k]? = some it) {v : Rat} (hv : c.2[k]? = some v) : it.inRange v := by
  have hoks : ∀ it ∈ K.items, it.okB K.ranges K.m.table.rows = true := List.all_eq_true.1 hok
  obtain ⟨pl, hpl, _, hph⟩ := Sys.reaches_mem (Model.simulates hok) hr hc
  have hph : PlaceHolds K.items pl.2 c.2 := hph
  obtain ⟨c0, v0, hc0, hv0, hhold⟩ := placeHolds_get hph hit
  rw [hv] at hv0
  cases hv0
  have h := List.all_eq_true.1 hrange pl hpl
  rw [hc0] at h
  unfold CItem.inRange
  cases c0 with
  | none => simp at h
  | some ci =>
    obtain ⟨a, b, hab, ha, hb⟩ := hhold
    cases hkind : it.kind with
    | words ws => trivial
    | num lo hi st g =>
      obtain ⟨hst, _, _, htile, _, _⟩ := CItem.okB_num hkind (hoks it (List.mem_of_getElem? hit))
      obtain ⟨hlo, hhi⟩ := tilesB_within hst htile hab
      exact ⟨Rat.le_trans hlo ha, Rat.le_trans hb hhi⟩

/-! ## The claims speak about cases that exist -/

/-- Every coordinate of a tiling runs forwards. -/
theorem tilesB_le {st : Rat} :
    ∀ {lo hi : Rat} {cs : List (Rat × Rat)} {ci : Nat} {a b : Rat}, tilesB st lo hi cs = true →
      cs[ci]? = some (a, b) → a ≤ b
  | _, _, [], _, _, _, h, _ => by simp [tilesB] at h
  | lo, hi, [(a0, b0)], ci, a, b, h, hci => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨_, _⟩, h3⟩, _⟩, _⟩ := h
    cases ci with
    | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq, Prod.mk.injEq] at hci
      obtain ⟨rfl, rfl⟩ := hci
      exact h3
    | succ k => simp at hci
  | lo, hi, (a0, b0) :: cd :: rest, ci, a, b, h, hci => by
    simp only [tilesB, Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
    obtain ⟨⟨⟨⟨_, h2⟩, _⟩, _⟩, h5⟩ := h
    cases ci with
    | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq, Prod.mk.injEq] at hci
      obtain ⟨rfl, rfl⟩ := hci
      exact h2
    | succ k => exact tilesB_le h5 (by simpa using hci)

/-- **The start check**: one coordinate of each carried value, each one of its coordinates. -/
def Model.startOk (K : Model) (cc : List (Option Nat)) : Bool :=
  cc.length == K.items.length &&
    (K.items.zip cc).all (fun ic =>
      match ic.2 with
      | some ci => decide (ci < ic.1.coords.length)
      | none => false)

theorem startOk_inhabited {ranges : List (String × Span2)} {rows : List Row} :
    ∀ {its : List CItem} {cc : List (Option Nat)},
      (∀ it ∈ its, it.okB ranges rows = true) → cc.length = its.length →
      (∀ ic ∈ its.zip cc, match ic.2 with
        | some ci => ci < ic.1.coords.length
        | none => False) →
      ∃ xs, PlaceHolds its cc xs
  | [], [], _, _, _ => ⟨[], by simp [PlaceHolds]⟩
  | [], _ :: _, _, hlen, _ => by simp at hlen
  | _ :: _, [], _, hlen, _ => by simp at hlen
  | it :: its, c :: cs, hoks, hlen, hall => by
    obtain ⟨xs, hxs⟩ := startOk_inhabited (fun it' h' => hoks it' (List.mem_cons_of_mem _ h'))
      (by simpa using hlen) (fun ic hic => hall ic (by simp [List.zip_cons_cons, hic]))
    have hc := hall (it, c) (by simp [List.zip_cons_cons])
    cases c with
    | none => exact absurd hc (by simp)
    | some ci =>
      simp only at hc
      obtain ⟨ab, hab⟩ : ∃ ab, it.coords[ci]? = some ab := ⟨_, List.getElem?_eq_getElem hc⟩
      obtain ⟨a, b⟩ := ab
      have hle : a ≤ b := by
        cases hk : it.kind with
        | num lo hi st g =>
          obtain ⟨_, _, _, htile, _, _⟩ := CItem.okB_num hk (hoks it (by simp))
          exact tilesB_le htile hab
        | words ws =>
          obtain ⟨ha, hb⟩ := CItem.okB_words_coords hk (hoks it (by simp)) hab
          rw [ha, hb]
          exact Rat.le_refl
      exact ⟨a :: xs, by simp only [PlaceHolds]; exact ⟨⟨a, b, hab, Rat.le_refl, hle⟩, hxs⟩⟩

/-- **The start place holds a configuration**, where the start check passes: the claims over the
    places are about cases that exist, not about none. -/
theorem Model.start_inhabited {K : Model} (hok : K.okB = true) {s : Nat} {cc : List (Option Nat)}
    (h : K.startOk cc = true) : ∃ c, K.sys.At (s, cc) c := by
  have hoks : ∀ it ∈ K.items, it.okB K.ranges K.m.table.rows = true := List.all_eq_true.1 hok
  simp only [Model.startOk, Bool.and_eq_true, beq_iff_eq, List.all_eq_true] at h
  obtain ⟨hlen, hall⟩ := h
  obtain ⟨xs, hxs⟩ := startOk_inhabited hoks hlen (fun ic hic => by
    have := hall ic hic
    cases h2 : ic.2 with
    | none => rw [h2] at this; exact absurd this (by simp)
    | some ci => rw [h2] at this; simpa using this)
  exact ⟨(s, xs), rfl, hxs⟩

end Carried
end RulecCert
