/-
  What a contract lets through, held to what a rule takes (DESIGN §15.140, §15.142).

  A contract — a `.proto` with its Protovalidate rules, a JSON Schema — says which values can
  reach the rule's inputs, and it can relate them to each other. `rulec` reads that as a
  condition: atoms over the values, opened into cases, a value getting through when it
  satisfies every atom of some case. The rule's door asks things of the same values: each
  input inside its declared range, each `constraint` kept, each enum input one of its values.
  The claim is **inclusion**: every value the contract lets through is one the door takes, so
  nothing a caller sends past its own validation is refused at the door — or, since the tables
  are complete over what the door takes, left without an answer.

  The certificate hands over, for each thing the door asks and each case, why the case keeps
  it: multipliers that add the case's inequalities and the door's negation up to a
  contradiction, strings or truth values of the case that cannot all hold, or the strings the
  case lets an enum input be, all among the enum's. This file says what those mean and proves
  that checking them settles the claim. What the condition is — that these atoms are what the
  contract says — is the certificate's word, as the rule's own expressions are.
-/
import RulecCert.Linear

namespace RulecCert

/-! ## The values, the condition, and the door -/

/-- The values a contract's condition speaks about, each kind numbered on its own. -/
structure CVals where
  nums : List Rat
  strs : List String
  bools : List Bool

/-- One condition of a contract. -/
inductive CAtom where
  /-- `coeffs · nums + k ≤ 0`, or `< 0` when strict. -/
  | le : LinIneq → CAtom
  /-- `coeffs · nums + k = 0`. -/
  | eq : LinIneq → CAtom
  /-- String `i` is one of these (`true`), or none of them (`false`). -/
  | str : Nat → List String → Bool → CAtom
  /-- Truth value `i` is this one. -/
  | bool : Nat → Bool → CAtom
  deriving Inhabited

def CAtom.holds : CAtom → CVals → Prop
  | .le q, σ => q.holds σ.nums
  | .eq q, σ => q.lhs σ.nums = 0
  | .str i vs yes, σ => vs.contains (σ.strs.getD i "") = yes
  | .bool i b, σ => σ.bools.getD i false = b

/-- Every atom the case names holds. -/
def caseHolds (atoms : List CAtom) (case : List Nat) (σ : CVals) : Prop :=
  ∀ i ∈ case, ∃ a, atoms[i]? = some a ∧ a.holds σ

/-- **What the contract lets through**: the values satisfy some case. -/
def admits (atoms : List CAtom) (cases : List (List Nat)) (σ : CVals) : Prop :=
  ∃ c ∈ cases, caseHolds atoms c σ

/-- One thing the rule's door asks of the values. -/
inductive Door where
  /-- An inequality over the numbers: an end of an input's range, a `constraint`. -/
  | num : LinIneq → Door
  /-- String `i` is one of an enum's values. -/
  | member : Nat → List String → Door
  deriving Inhabited

def Door.holds : Door → CVals → Prop
  | .num q, σ => q.holds σ.nums
  | .member i vs, σ => vs.contains (σ.strs.getD i "") = true

/-! ## The inequalities a case gives a refutation -/

/-- The inequality that holds wherever `q` does not. -/
def negIneq (q : LinIneq) : LinIneq :=
  { coeffs := vscale (-1) q.coeffs, k := -q.k, strict := !q.strict }

theorem negIneq_lhs (q : LinIneq) (v : List Rat) : (negIneq q).lhs v = -(q.lhs v) := by
  unfold negIneq LinIneq.lhs
  simp only [dot_vscale]
  grind

theorem negIneq_holds {q : LinIneq} {v : List Rat} (h : ¬ q.holds v) : (negIneq q).holds v := by
  unfold LinIneq.holds at h ⊢
  rw [negIneq_lhs]
  cases hs : q.strict <;> simp only [hs, negIneq, Bool.not_false, Bool.not_true, ite_true,
    Bool.false_eq_true, ite_false] at h ⊢ <;> grind

/-- One inequality an atom gives: the atom itself, or one half of an equality (`0` is
    `lhs ≤ 0`, `1` is `−lhs ≤ 0`). -/
def CAtom.part : CAtom → Nat → Option LinIneq
  | .le q, 0 => some q
  | .eq q, 0 => some { q with strict := false }
  | .eq q, 1 => some { coeffs := vscale (-1) q.coeffs, k := -q.k, strict := false }
  | _, _ => none

theorem CAtom.part_holds {a : CAtom} {σ : CVals} {n : Nat} {q : LinIneq}
    (ha : a.holds σ) (hp : a.part n = some q) : q.holds σ.nums := by
  cases a with
  | le q' =>
    cases n with
    | zero => simp only [CAtom.part, Option.some.injEq] at hp; subst hp; exact ha
    | succ _ => simp [CAtom.part] at hp
  | eq q' =>
    simp only [CAtom.holds] at ha
    match n, hp with
    | 0, hp =>
      simp only [CAtom.part, Option.some.injEq] at hp
      subst hp
      unfold LinIneq.holds LinIneq.lhs at *
      simp only [Bool.false_eq_true, ite_false]
      grind
    | 1, hp =>
      simp only [CAtom.part, Option.some.injEq] at hp
      subst hp
      unfold LinIneq.holds LinIneq.lhs at *
      simp only [Bool.false_eq_true, ite_false, dot_vscale]
      grind
    | n + 2, hp => simp [CAtom.part] at hp
  | str _ _ _ => simp [CAtom.part] at hp
  | bool _ _ => simp [CAtom.part] at hp

/-- Where one inequality of a case's refutation comes from. -/
inductive CRef where
  /-- An atom the case names, and which of its inequalities. -/
  | atom : Nat → Nat → CRef
  /-- The negation of what the door asks. -/
  | door : CRef
  deriving Inhabited

/-- The inequalities a refutation names, built again from the case and the door; `none` where
    an atom is not the case's or the door is not an inequality. -/
def caseIneqs (atoms : List CAtom) (case : List Nat) (door : Option LinIneq) :
    List (CRef × Rat) → Option (List (Rat × LinIneq))
  | [] => some []
  | (r, y) :: rest =>
    match caseIneqs atoms case door rest with
    | none => none
    | some qs =>
      match r with
      | .atom i n =>
        if case.contains i then
          match atoms[i]? with
          | some a => (a.part n).map (fun q => (y, q) :: qs)
          | none => none
        else none
      | .door => door.map (fun q => (y, negIneq q) :: qs)

theorem caseIneqs_hold {atoms : List CAtom} {case : List Nat} {door : Option LinIneq} {σ : CVals}
    (hc : caseHolds atoms case σ) (hd : ∀ q, door = some q → (negIneq q).holds σ.nums) :
    ∀ (refs : List (CRef × Rat)) (ps : List (Rat × LinIneq)),
      caseIneqs atoms case door refs = some ps → ∀ p ∈ ps, p.2.holds σ.nums
  | [], ps, h => by simp only [caseIneqs, Option.some.injEq] at h; subst h; simp
  | (r, y) :: rest, ps, h => by
    simp only [caseIneqs] at h
    split at h
    · exact absurd h (by simp)
    · rename_i qs hqs
      have ih := caseIneqs_hold hc hd rest qs hqs
      cases r with
      | atom i n =>
        by_cases hi : case.contains i = true
        · simp only [hi, ite_true] at h
          split at h
          · rename_i a ha
            simp only [Option.map_eq_some_iff] at h
            obtain ⟨q, hq, rfl⟩ := h
            intro p hp
            simp only [List.mem_cons] at hp
            rcases hp with rfl | hp
            · obtain ⟨a', ha', hh⟩ := hc i (List.contains_iff_mem.1 hi)
              rw [ha] at ha'
              obtain rfl : a = a' := Option.some.inj ha'
              exact CAtom.part_holds hh hq
            · exact ih p hp
          · exact absurd h (by simp)
        · simp only [hi, Bool.false_eq_true, ite_false] at h
          exact absurd h (by simp)
      | door =>
        simp only [Option.map_eq_some_iff] at h
        obtain ⟨q, hq, rfl⟩ := h
        intro p hp
        simp only [List.mem_cons] at hp
        rcases hp with rfl | hp
        · exact hd q hq
        · exact ih p hp

/-! ## Strings and truth values -/

/-- The lists a case says string `i` is in (`yes`), or not in (`!yes`). -/
def strLists (atoms : List CAtom) (case : List Nat) (i : Nat) (yes : Bool) : List (List String) :=
  case.filterMap (fun j =>
    match atoms[j]? with
    | some (CAtom.str i' vs y) => if i' == i && y == yes then some vs else none
    | _ => none)

/-- Every string the case lets through is in `ok`: the case names at least one list the value is
    in, and each string of the first is left out by another list, or is in `ok`. With `ok`
    empty, the case lets no string through at all. -/
def strsWithin (yes no : List (List String)) (ok : List String) : Bool :=
  match yes with
  | [] => false
  | y :: _ => y.all (fun s => yes.any (fun l => !l.contains s) || no.any (fun l => l.contains s) || ok.contains s)

theorem mem_strLists {atoms : List CAtom} {case : List Nat} {i : Nat} {yes : Bool} {l : List String}
    (h : l ∈ strLists atoms case i yes) : ∃ j ∈ case, atoms[j]? = some (.str i l yes) := by
  simp only [strLists, List.mem_filterMap] at h
  obtain ⟨j, hj, hm⟩ := h
  refine ⟨j, hj, ?_⟩
  split at hm
  · rename_i i' vs y hat
    by_cases hc : (i' == i && y == yes) = true
    · simp only [hc, ite_true, Option.some.injEq] at hm
      simp only [Bool.and_eq_true, beq_iff_eq] at hc
      obtain ⟨rfl, rfl⟩ := hc
      subst hm
      exact hat
    · simp only [hc, Bool.false_eq_true, ite_false] at hm; exact absurd hm (by simp)
  · exact absurd hm (by simp)

theorem within_of_strsWithin {atoms : List CAtom} {case : List Nat} {σ : CVals} {i : Nat}
    {ok : List String} (hc : caseHolds atoms case σ)
    (h : strsWithin (strLists atoms case i true) (strLists atoms case i false) ok = true) :
    ok.contains (σ.strs.getD i "") = true := by
  unfold strsWithin at h
  split at h
  · exact absurd h (by simp)
  · rename_i y rest hy
    have hyes : ∀ l ∈ strLists atoms case i true, l.contains (σ.strs.getD i "") = true := by
      intro l hl
      obtain ⟨j, hj, hat⟩ := mem_strLists hl
      obtain ⟨a, ha, hh⟩ := hc j hj
      rw [hat] at ha
      obtain rfl := Option.some.inj ha
      simpa [CAtom.holds] using hh
    have hno : ∀ l ∈ strLists atoms case i false, l.contains (σ.strs.getD i "") = false := by
      intro l hl
      obtain ⟨j, hj, hat⟩ := mem_strLists hl
      obtain ⟨a, ha, hh⟩ := hc j hj
      rw [hat] at ha
      obtain rfl := Option.some.inj ha
      simpa [CAtom.holds] using hh
    have hin : y.contains (σ.strs.getD i "") = true := hyes y (by rw [hy]; simp)
    have hall := List.all_eq_true.1 h (σ.strs.getD i "") (List.contains_iff_mem.1 hin)
    simp only [Bool.or_eq_true, List.any_eq_true, Bool.not_eq_true'] at hall
    rcases hall with (⟨l, hl, hnot⟩ | ⟨l, hl, hc'⟩) | hok
    · rw [hyes l hl] at hnot; exact absurd hnot (by simp)
    · rw [hno l hl] at hc'; exact absurd hc' (by simp)
    · exact hok

/-- Whether the case says truth value `i` is `b`. -/
def saysBool (atoms : List CAtom) (case : List Nat) (i : Nat) (b : Bool) : Bool :=
  case.any (fun j =>
    match atoms[j]? with
    | some (CAtom.bool i' b') => i' == i && b' == b
    | _ => false)

theorem bool_of_saysBool {atoms : List CAtom} {case : List Nat} {σ : CVals} {i : Nat} {b : Bool}
    (hc : caseHolds atoms case σ) (h : saysBool atoms case i b = true) : σ.bools.getD i false = b := by
  simp only [saysBool, List.any_eq_true] at h
  obtain ⟨j, hj, hm⟩ := h
  split at hm
  · rename_i i' b' hat
    simp only [Bool.and_eq_true, beq_iff_eq] at hm
    obtain ⟨rfl, rfl⟩ := hm
    obtain ⟨a, ha, hh⟩ := hc j hj
    rw [hat] at ha
    obtain rfl := Option.some.inj ha
    simpa [CAtom.holds] using hh
  · exact absurd hm (by simp)

/-! ## Why a case keeps what the door asks -/

inductive CaseWhy where
  /-- The case's inequalities and the door's negation add up to a contradiction. -/
  | farkas : List (CRef × Rat) → CaseWhy
  /-- The case lets string `i` be no string at all. -/
  | clashStr : Nat → CaseWhy
  /-- The case says truth value `i` is both. -/
  | clashBool : Nat → CaseWhy
  /-- The strings the case lets an enum input be are all among the enum's values. -/
  | within : CaseWhy
  deriving Inhabited

def caseWhyOk (atoms : List CAtom) (case : List Nat) (door : Door) : CaseWhy → Bool
  | .farkas refs =>
    match caseIneqs atoms case (match door with | .num q => some q | .member _ _ => none) refs with
    | some ps => farkasOk ps
    | none => false
  | .clashStr i => strsWithin (strLists atoms case i true) (strLists atoms case i false) []
  | .clashBool i => saysBool atoms case i true && saysBool atoms case i false
  | .within =>
    match door with
    | .member i vs => strsWithin (strLists atoms case i true) (strLists atoms case i false) vs
    | .num _ => false

theorem caseWhy_sound {atoms : List CAtom} {case : List Nat} {door : Door} {w : CaseWhy}
    {σ : CVals} (h : caseWhyOk atoms case door w = true) (hc : caseHolds atoms case σ) :
    door.holds σ := by
  cases w with
  | farkas refs =>
    cases door with
    | num q =>
      simp only [caseWhyOk] at h
      split at h
      · rename_i ps hps
        exact Classical.byContradiction (fun hq => farkas_sound h σ.nums (caseIneqs_hold hc (fun q' hq' => by
          simp only [Option.some.injEq] at hq'; subst hq'; exact negIneq_holds hq) refs ps hps))
      · exact absurd h (by simp)
    | member i vs =>
      simp only [caseWhyOk] at h
      split at h
      · rename_i ps hps
        exact (farkas_sound h σ.nums (caseIneqs_hold hc (fun q' hq' => by simp at hq') refs ps hps)).elim
      · exact absurd h (by simp)
  | clashStr i =>
    simp only [caseWhyOk] at h
    have := within_of_strsWithin (ok := []) hc h
    simp at this
  | clashBool i =>
    simp only [caseWhyOk, Bool.and_eq_true] at h
    have h1 := bool_of_saysBool hc h.1
    have h2 := bool_of_saysBool hc h.2
    rw [h1] at h2
    exact absurd h2 (by simp)
  | within =>
    cases door with
    | num q => simp [caseWhyOk] at h
    | member i vs => exact within_of_strsWithin hc h

/-! ## Inclusion -/

/-- **The inclusion check.** For each thing the door asks, one reason per case, in the
    cases' order, and every one of them holds. -/
def includedOk (atoms : List CAtom) (cases : List (List Nat)) (doors : List (Door × List CaseWhy)) : Bool :=
  doors.all (fun d => d.2.length == cases.length && (cases.zip d.2).all (fun cw => caseWhyOk atoms cw.1 d.1 cw.2))

theorem exists_zip_of_mem {α β : Type _} :
    ∀ {l1 : List α} {l2 : List β} {a : α}, a ∈ l1 → l1.length = l2.length → ∃ b, (a, b) ∈ l1.zip l2
  | [], _, _, h, _ => by simp at h
  | x :: xs, [], _, _, hl => by simp at hl
  | x :: xs, y :: ys, a, h, hl => by
    simp only [List.mem_cons] at h
    rcases h with rfl | h
    · exact ⟨y, by simp⟩
    · obtain ⟨b, hb⟩ := exists_zip_of_mem h (by simpa using hl)
      exact ⟨b, by simp [hb]⟩

/-- **What the contract lets through, the door takes.** If the inclusion check passes, every
    value the contract's condition admits satisfies everything the door asks. -/
theorem included_sound {atoms : List CAtom} {cases : List (List Nat)} {doors : List (Door × List CaseWhy)}
    (h : includedOk atoms cases doors = true) (σ : CVals) (ha : admits atoms cases σ) :
    ∀ d ∈ doors, d.1.holds σ := by
  intro d hd
  obtain ⟨c, hc, hch⟩ := ha
  have hdd := List.all_eq_true.1 h d hd
  simp only [Bool.and_eq_true, beq_iff_eq] at hdd
  obtain ⟨hlen, hall⟩ := hdd
  obtain ⟨w, hw⟩ := exists_zip_of_mem hc hlen.symm
  exact caseWhy_sound (List.all_eq_true.1 hall (c, w) hw) hch

end RulecCert
