/-
  A rule as one step of a state machine, as far as a certificate speaks about it (DESIGN §15.148).

  A `machine` names one input of a rule that the host hands back as one of the rule's outputs
  on the next call. The generated code keeps nothing; what the section adds is claims about
  every **sequence** of calls. This file says what those claims are, in the terms of
  `RulecCert.Semantics`, and proves that the checks `Main` runs on the certificate's
  `machine` section settle them.

  The table that decides the carried output is a `Table` as before. One of its axes is the
  state — or none, for a table that moves every state alike — and each row either writes a
  state or leaves the one it was called in. One **call** from state `s` is a point the rule
  is asked about, at `s`, in some row's box, and it goes where that row's move takes `s`. The
  row that answers a point takes it, so every call the rule answers is one of these: `step`
  may say more than happens, never less. That is the right way round for the claims that
  something is never reached — a set that holds the start and is closed under `step` holds
  everything a case can reach. For the claim that a final state can always still be reached
  it is the wrong way round, and there the certificate hands over calls that really happen:
  a point the rule is asked about, in the row that answers it (`fires`).
-/
import RulecCert.Certified

namespace RulecCert

/-- What a row does to the carried state. -/
inductive Move where
  /-- Write this state. -/
  | goTo : Nat → Move
  /-- Leave the state the call was made in. -/
  | stay : Move
  deriving DecidableEq, Repr

def Move.apply : Move → Nat → Nat
  | .goTo s, _ => s
  | .stay, s => s

/-- A machine laid on one table: the table, the axis that is the state (none when the table
    does not read it), and each row's move by row number. States are coordinates of that axis:
    the enum's values in the order it declares them. -/
structure Machine where
  table : Table
  axis : Option Nat
  move : Nat → Option Move

namespace Machine

/-- A point is at state `s`. A table that does not read the state answers every state alike. -/
def atState (m : Machine) (p : Point) (s : Nat) : Prop :=
  match m.axis with
  | some k => p[k]? = some s
  | none => True

/-- **One call**, through row `r`, from `s` to `s'`. -/
def stepVia (m : Machine) (s : Nat) (r : Row) (s' : Nat) : Prop :=
  ∃ p mv, inSpace m.table.arities p = true ∧ m.table.asked p ∧ m.atState p s ∧
    r ∈ m.table.rows ∧ inBox r.box p = true ∧ m.move r.index = some mv ∧ mv.apply s = s'

def step (m : Machine) (s s' : Nat) : Prop := ∃ r, m.stepVia s r s'

/-- The states a case can be in: it starts at `init`, and each call moves it on. -/
inductive Reaches (m : Machine) (init : Nat) : Nat → Prop
  | start : Reaches m init init
  | call {s s' : Nat} : Reaches m init s → m.step s s' → Reaches m init s'

/-- The same, with whether the case has been in a state of `B` yet. -/
inductive ReachesAfter (m : Machine) (init : Nat) (B : List Nat) : Nat → Bool → Prop
  | start : ReachesAfter m init B init (B.contains init)
  | call {s s' : Nat} {b : Bool} : ReachesAfter m init B s b → m.step s s' →
      ReachesAfter m init B s' (b || B.contains s')

/-- The same, counting — up to two — the calls through rows `counts` marks. -/
inductive ReachesCounting (m : Machine) (init : Nat) (counts : Nat → Bool) : Nat → Nat → Prop
  | start : ReachesCounting m init counts init 0
  | call {s s' n : Nat} {r : Row} : ReachesCounting m init counts s n → m.stepVia s r s' →
      ReachesCounting m init counts s' (min 2 (n + if counts r.index then 1 else 0))

/-- **A call that really happens**: a point the rule is asked about, at `s`, answered by `r` —
    under `first`, no earlier row takes the point. (Under `unique` the table's own certificate
    says `r` is the only row that takes it: `firing_eq_of_unique`.) -/
def fires (m : Machine) (s : Nat) (r : Row) (s' : Nat) : Prop :=
  ∃ p mv, inSpace m.table.arities p = true ∧ m.table.asked p ∧ m.atState p s ∧
    r ∈ m.table.rows ∧ inBox r.box p = true ∧
    (m.table.policy = Policy.first → ∀ q ∈ m.table.rows, q.index < r.index → inBox q.box p = false) ∧
    m.move r.index = some mv ∧ mv.apply s = s'

/-- From `s` a final state can be reached, by calls that really happen. -/
inductive Finishes (m : Machine) (F : List Nat) : Nat → Prop
  | done {f : Nat} : f ∈ F → Finishes m F f
  | call {s s' : Nat} {r : Row} : m.fires s r s' → Finishes m F s' → Finishes m F s

/-! ## The checks -/

/-- Whether a row's box takes state `s` on the state's axis. -/
def rowAt (m : Machine) (r : Row) (s : Nat) : Bool :=
  match m.axis with
  | some k =>
    match r.box[k]? with
    | some xs => xs.contains s
    | none => false
  | none => true

/-- Where one call from `s` can go, and through which row: every row whose box takes `s`. -/
def succs (m : Machine) (s : Nat) : List (Nat × Nat) :=
  m.table.rows.filterMap (fun r =>
    if m.rowAt r s then (m.move r.index).map (fun mv => (r.index, mv.apply s)) else none)

/-- **The reach check**: the set holds where a case starts and is closed under a call. -/
def reachOk (m : Machine) (init : Nat) (R : List Nat) : Bool :=
  R.contains init && R.all (fun s => (m.succs s).all (fun e => R.contains e.2))

/-- **The final check**: from a final state every call stays there. -/
def finalOk (m : Machine) (F : List Nat) : Bool :=
  F.all (fun f => (m.succs f).all (fun e => e.2 == f))

/-- **The `never` check**: the pairs (state, has been in `B`) a case can be in, closed under a
    call, holding where a case starts, and none of them in `A` after `B`. -/
def neverOk (m : Machine) (init : Nat) (A B : List Nat) (P : List (Nat × Bool)) : Bool :=
  P.contains (init, B.contains init) &&
    P.all (fun q => !(q.2 && A.contains q.1) &&
      (m.succs q.1).all (fun e => P.contains (e.2, q.2 || B.contains e.2)))

/-- **The `once` check**: the pairs (state, calls counted, at most two), closed under a call,
    holding where a case starts, and none of them at two. -/
def onceOk (m : Machine) (init : Nat) (counts : Nat → Bool) (P : List (Nat × Nat)) : Bool :=
  P.contains (init, 0) &&
    P.all (fun q => decide (q.2 < 2) &&
      (m.succs q.1).all (fun e => P.contains (e.2, min 2 (q.2 + if counts e.1 then 1 else 0))))

/-- One call a path hands over: the state it is made from, the row, and the point with the
    values behind it. -/
structure Call where
  state : Nat
  row : Nat
  point : Point
  values : List Rat

/-- Whether a point is at state `s`, as a test. -/
def atStateB (m : Machine) (p : Point) (s : Nat) : Bool :=
  match m.axis with
  | some k => p[k]? == some s
  | none => true

theorem atState_of_atStateB {m : Machine} {p : Point} {s : Nat} (h : m.atStateB p s = true) :
    m.atState p s := by
  unfold atStateB at h
  unfold atState
  cases hax : m.axis with
  | none => trivial
  | some k =>
    rw [hax] at h
    simpa using h

/-- Whether no earlier row takes the point, where that matters. -/
def firstOk (m : Machine) (r : Row) (p : Point) : Bool :=
  m.table.policy != Policy.first ||
    m.table.rows.all (fun q => decide (r.index ≤ q.index) || !inBox q.box p)

theorem first_of_firstOk {m : Machine} {r : Row} {p : Point} (h : m.firstOk r p = true) :
    m.table.policy = Policy.first → ∀ q ∈ m.table.rows, q.index < r.index → inBox q.box p = false := by
  intro hpol q hq hlt
  unfold firstOk at h
  simp only [Bool.or_eq_true, bne_iff_ne, ne_eq, List.all_eq_true, decide_eq_true_eq,
    Bool.not_eq_true'] at h
  rcases h with hne | hall
  · exact absurd hpol hne
  · rcases hall q hq with hle | hno
    · omega
    · exact hno

/-- A call checks when its point is in the space, the rule is asked about it, it is in the
    row's box at the stated state, and — under `first` — no earlier row takes it. The answer
    is the state the call goes to. -/
def callOk (m : Machine) (askedOk : Point → List Rat → Bool) (c : Call) : Option Nat :=
  match m.table.rows.find? (fun r => r.index == c.row) with
  | none => none
  | some r =>
    match m.move r.index with
    | none => none
    | some mv =>
      if inSpace m.table.arities c.point && askedOk c.point c.values && inBox r.box c.point &&
          m.atStateB c.point c.state && m.firstOk r c.point
      then some (mv.apply c.state) else none

/-- A path from `s` checks when each call is made from where the one before went, and it ends
    at a final state. -/
def pathOk (m : Machine) (askedOk : Point → List Rat → Bool) (F : List Nat) : Nat → List Call → Bool
  | s, [] => F.contains s
  | s, c :: cs =>
    c.state == s &&
      (match m.callOk askedOk c with
       | some s' => m.pathOk askedOk F s' cs
       | none => false)

/-- **The finish check**: from every state of the set, a path to a final state. -/
def finishOk (m : Machine) (askedOk : Point → List Rat → Bool) (R F : List Nat)
    (paths : Nat → List Call) : Bool :=
  R.all (fun s => F.contains s || m.pathOk askedOk F s (paths s))

/-! ## What they settle -/

theorem inBox_length {b : Box} {p : Point} (h : inBox b p = true) : b.length = p.length := by
  induction b generalizing p with
  | nil => cases p with
    | nil => rfl
    | cons _ _ => simp [inBox] at h
  | cons x xs ih => cases p with
    | nil => simp [inBox] at h
    | cons c cs =>
      simp only [inBox, Bool.and_eq_true] at h
      simp [ih h.2]

/-- A row that answers a call from `s` takes `s` on the state's axis. -/
theorem rowAt_of_box {m : Machine} {p : Point} {s : Nat} {r : Row}
    (hat : m.atState p s) (hbox : inBox r.box p = true) : m.rowAt r s = true := by
  unfold rowAt
  unfold atState at hat
  cases hax : m.axis with
  | none => rfl
  | some k =>
    rw [hax] at hat
    simp only
    have hlen := inBox_length hbox
    have hk : k < p.length := lt_of_getElem? hat
    cases hb : r.box[k]? with
    | none =>
      rw [List.getElem?_eq_none_iff] at hb
      omega
    | some xs =>
      obtain ⟨c, hc, hcx⟩ := inBox_getElem hbox hb
      rw [hat] at hc
      cases hc
      exact hcx

/-- Every call is one of the successors the check walks. -/
theorem mem_succs {m : Machine} {s s' : Nat} {r : Row} (h : m.stepVia s r s') :
    (r.index, s') ∈ m.succs s := by
  obtain ⟨p, mv, _, _, hat, hr, hbox, hmv, hap⟩ := h
  unfold succs
  rw [List.mem_filterMap]
  refine ⟨r, hr, ?_⟩
  rw [rowAt_of_box hat hbox]
  simp [hmv, hap]

/-- **Reach**: a set the reach check passes holds every state a case can reach. -/
theorem reaches_mem {m : Machine} {init : Nat} {R : List Nat} (h : m.reachOk init R = true) :
    ∀ {s : Nat}, m.Reaches init s → s ∈ R := by
  simp only [reachOk, Bool.and_eq_true, List.contains_iff_mem, List.all_eq_true] at h
  intro s hr
  induction hr with
  | start => exact h.1
  | call _ hs ih =>
    obtain ⟨r, hv⟩ := hs
    have := h.2 _ ih _ (mem_succs hv)
    simpa using this

/-- **Final**: no call leads out of a state the final check passes. -/
theorem final_stays {m : Machine} {F : List Nat} (h : m.finalOk F = true) {f s' : Nat}
    (hf : f ∈ F) (hs : m.step f s') : s' = f := by
  obtain ⟨r, hv⟩ := hs
  simp only [finalOk, List.all_eq_true] at h
  have := h f hf _ (mem_succs hv)
  simpa using this

/-- **Never**: the pairs a case can be in are in the set the check passes. -/
theorem reachesAfter_mem {m : Machine} {init : Nat} {A B : List Nat} {P : List (Nat × Bool)}
    (h : m.neverOk init A B P = true) : ∀ {s : Nat} {b : Bool}, m.ReachesAfter init B s b → (s, b) ∈ P := by
  simp only [neverOk, Bool.and_eq_true, List.contains_iff_mem, List.all_eq_true] at h
  intro s b hr
  induction hr with
  | start => exact h.1
  | call _ hs ih =>
    obtain ⟨r, hv⟩ := hs
    have := (h.2 _ ih).2 _ (mem_succs hv)
    simpa using this

/-- **Never, as the claim reads**: once a case has been in a state of `B`, it is never in a
    state of `A`. -/
theorem never_after {m : Machine} {init : Nat} {A B : List Nat} {P : List (Nat × Bool)}
    (h : m.neverOk init A B P = true) {s : Nat} (hr : m.ReachesAfter init B s true) : s ∉ A := by
  have hm := reachesAfter_mem h hr
  simp only [neverOk, Bool.and_eq_true, List.all_eq_true] at h
  have h1 := (h.2 _ hm).1
  intro hs
  simp at h1
  exact h1 hs

/-- `ReachesAfter` carries exactly "has been in `B`": the flag is set once a state of `B` has
    been visited, and stays set. -/
theorem reachesAfter_of_reaches {m : Machine} {init : Nat} {B : List Nat} :
    ∀ {s : Nat}, m.Reaches init s → ∃ b, m.ReachesAfter init B s b := by
  intro s hr
  induction hr with
  | start => exact ⟨_, ReachesAfter.start⟩
  | call _ hs ih =>
    obtain ⟨b, hb⟩ := ih
    exact ⟨_, ReachesAfter.call hb hs⟩

/-- **Once**: the pairs a case can be in are in the set the check passes, so no case counts
    two calls. -/
theorem reachesCounting_mem {m : Machine} {init : Nat} {counts : Nat → Bool} {P : List (Nat × Nat)}
    (h : m.onceOk init counts P = true) :
    ∀ {s n : Nat}, m.ReachesCounting init counts s n → (s, n) ∈ P := by
  simp only [onceOk, Bool.and_eq_true, List.contains_iff_mem, List.all_eq_true] at h
  intro s n hr
  induction hr with
  | start => exact h.1
  | call _ hv ih =>
    have := (h.2 _ ih).2 _ (mem_succs hv)
    simpa using this

theorem once_below_two {m : Machine} {init : Nat} {counts : Nat → Bool} {P : List (Nat × Nat)}
    (h : m.onceOk init counts P = true) {s n : Nat} (hr : m.ReachesCounting init counts s n) : n < 2 := by
  have hm := reachesCounting_mem h hr
  simp only [onceOk, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at h
  exact (h.2 _ hm).1

/-- A call that checks is a call that really happens. -/
theorem fires_of_callOk {m : Machine} {askedOk : Point → List Rat → Bool}
    (hask : ∀ p v, askedOk p v = true → m.table.asked p) {c : Call} {s' : Nat}
    (h : m.callOk askedOk c = some s') : ∃ r, m.fires c.state r s' := by
  unfold callOk at h
  cases hr : m.table.rows.find? (fun r => r.index == c.row) with
  | none => rw [hr] at h; exact absurd h (by simp)
  | some r =>
    rw [hr] at h
    dsimp only at h
    cases hmv : m.move r.index with
    | none => rw [hmv] at h; exact absurd h (by simp)
    | some mv =>
      rw [hmv] at h
      dsimp only at h
      split at h
      next hc =>
        simp only [Bool.and_eq_true] at hc
        obtain ⟨⟨⟨⟨hsp, hak⟩, hbox⟩, hst⟩, hfirst⟩ := hc
        have hrm : r ∈ m.table.rows := List.mem_of_find?_eq_some hr
        refine ⟨r, c.point, mv, hsp, hask _ _ hak, atState_of_atStateB hst, hrm, hbox,
          first_of_firstOk hfirst, hmv, ?_⟩
        simpa using h
      next => exact absurd h (by simp)

/-- A path that checks is one a case can follow to a final state. -/
theorem finishes_of_pathOk {m : Machine} {askedOk : Point → List Rat → Bool}
    (hask : ∀ p v, askedOk p v = true → m.table.asked p) {F : List Nat} :
    ∀ {cs : List Call} {s : Nat}, m.pathOk askedOk F s cs = true → m.Finishes F s := by
  intro cs
  induction cs with
  | nil =>
    intro s h
    simp only [pathOk, List.contains_iff_mem] at h
    exact Finishes.done h
  | cons c cs ih =>
    intro s h
    simp only [pathOk, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨hst, hrest⟩ := h
    cases hc : m.callOk askedOk c with
    | none => rw [hc] at hrest; exact absurd hrest (by simp)
    | some s' =>
      rw [hc] at hrest
      obtain ⟨r, hf⟩ := fires_of_callOk hask hc
      rw [hst] at hf
      exact Finishes.call hf (ih hrest)

/-- **Finish**: from every state of the set the check passes, a final state can be reached by
    calls that really happen. With `reaches_mem`, that is every state a case can reach. -/
theorem finishes_of_finishOk {m : Machine} {askedOk : Point → List Rat → Bool}
    (hask : ∀ p v, askedOk p v = true → m.table.asked p) {R F : List Nat} {paths : Nat → List Call}
    (h : m.finishOk askedOk R F paths = true) : ∀ s ∈ R, m.Finishes F s := by
  intro s hs
  simp only [finishOk, List.all_eq_true, Bool.or_eq_true, List.contains_iff_mem] at h
  rcases h s hs with hf | hp
  · exact Finishes.done hf
  · exact finishes_of_pathOk hask hp

/-- **Everything a case can reach can still finish**: the two checks together. -/
theorem reachable_finishes {m : Machine} {askedOk : Point → List Rat → Bool}
    (hask : ∀ p v, askedOk p v = true → m.table.asked p) {init : Nat} {R F : List Nat}
    {paths : Nat → List Call} (hr : m.reachOk init R = true) (hf : m.finishOk askedOk R F paths = true) :
    ∀ {s : Nat}, m.Reaches init s → m.Finishes F s :=
  fun hs => finishes_of_finishOk hask hf _ (reaches_mem hr hs)

/-- Under `unique`, the row a call names is the only row that takes the point: the table's
    own certificate (`Certified.unique`) is what says so. -/
theorem firing_eq_of_unique {t : Table} {p : Point} {r : Row} (hlen : (t.firing p).length = 1)
    (hr : r ∈ t.rows) (hbox : inBox r.box p = true) : t.firing p = [r] := by
  have hmem : r ∈ t.firing p := by
    unfold Table.firing
    exact List.mem_filter.2 ⟨hr, hbox⟩
  match hf : t.firing p, hlen with
  | [x], _ =>
    rw [hf] at hmem
    simp at hmem
    rw [hmem]

/-! ## Worlds (§15.149)

A case holds its `held` inputs from its first call to its last. Where the table reads one as a
column, every call a case makes is at a point with the same coordinate there, and only the rows
that take that coordinate can answer it. A **world** is the machine with the other rows left
out, and the checks above are run on it once per world: what they settle about the world's
machine, they settle about the cases of the world. -/

/-- Whether a row takes the coordinates `w` on the axes `fixed`. -/
def takesWorld (fixed w : List Nat) (r : Row) : Bool :=
  (fixed.zip w).all (fun kc =>
    match r.box[kc.1]? with
    | some xs => xs.contains kc.2
    | none => false)

/-- **One world**: the machine with the rows that do not take its coordinates left out. -/
def world (m : Machine) (fixed w : List Nat) : Machine :=
  { m with table := { m.table with rows := m.table.rows.filter (takesWorld fixed w) } }

/-- A point has the coordinates `w` on the axes `fixed`. -/
def atWorld (fixed w : List Nat) (p : Point) : Prop :=
  ∀ kc ∈ fixed.zip w, p[kc.1]? = some kc.2

/-- **One call a case of the world makes**: from `s` to `s'`, at a point with the world's
    coordinates. -/
def stepIn (m : Machine) (fixed w : List Nat) (s s' : Nat) : Prop :=
  ∃ r p mv, inSpace m.table.arities p = true ∧ m.table.asked p ∧ m.atState p s ∧ atWorld fixed w p ∧
    r ∈ m.table.rows ∧ inBox r.box p = true ∧ m.move r.index = some mv ∧ mv.apply s = s'

/-- The states a case of the world can be in. -/
inductive ReachesIn (m : Machine) (fixed w : List Nat) (init : Nat) : Nat → Prop
  | start : ReachesIn m fixed w init init
  | call {s s' : Nat} : ReachesIn m fixed w init s → m.stepIn fixed w s s' → ReachesIn m fixed w init s'

/-- A box that takes a point takes each of its coordinates. -/
theorem box_takes {b : Box} {p : Point} (h : inBox b p = true) {k c : Nat} (hk : p[k]? = some c) :
    ∃ xs, b[k]? = some xs ∧ xs.contains c = true := by
  induction b generalizing p k with
  | nil =>
    cases p with
    | nil => simp at hk
    | cons _ _ => simp [inBox] at h
  | cons x xs ih =>
    cases p with
    | nil => simp [inBox] at h
    | cons y ys =>
      simp only [inBox, Bool.and_eq_true] at h
      cases k with
      | zero =>
        simp at hk
        subst hk
        exact ⟨x, rfl, h.1⟩
      | succ k =>
        simp at hk
        exact ih h.2 hk

/-- **A call of the world is a call of the world's machine**: the row that answers it takes the
    world's coordinates, because the point is in its box. -/
theorem stepIn_world {m : Machine} {fixed w : List Nat} {s s' : Nat} (h : m.stepIn fixed w s s') :
    (m.world fixed w).step s s' := by
  obtain ⟨r, p, mv, hsp, hask, hat, hw, hr, hbox, hmv, hap⟩ := h
  refine ⟨r, p, mv, hsp, hask, hat, ?_, hbox, hmv, hap⟩
  show r ∈ m.table.rows.filter (takesWorld fixed w)
  rw [List.mem_filter]
  refine ⟨hr, ?_⟩
  unfold takesWorld
  rw [List.all_eq_true]
  intro kc hkc
  obtain ⟨xs, hxs, hc⟩ := box_takes hbox (hw kc hkc)
  rw [hxs]
  exact hc

/-- **What the checks on a world settle holds of the cases of that world**: every state a case
    of the world reaches is one its machine reaches, so `reaches_mem`, `final_stays`,
    `never_after` and `once_below_two` on the world's machine speak for them. -/
theorem reachesIn_world {m : Machine} {fixed w : List Nat} {init s : Nat}
    (h : m.ReachesIn fixed w init s) : (m.world fixed w).Reaches init s := by
  induction h with
  | start => exact Reaches.start
  | call _ hs ih => exact Reaches.call ih (stepIn_world hs)

end Machine

end RulecCert
