/-
  What a branch narrows holds in the branch (kaname's DESIGN 5.3, 15.2.2).

  `Sat A ρ` says an environment of the evaluation is among those the scope of the analysis says:
  every name the scope has a value for is in the environment, with a value among what the scope
  says of it, and every relation the conditions in force keep between two names (or fields of
  them) holds between their values. A condition that came out true (or false) narrows the scope to
  one the environment is still in: `Sat.apply`. A difference of two names whose order a relation
  says is on the side it says: `relation_sound`, `subRel_sound`.
-/
import KanameCore.Grows

namespace KanameCore

-- ── The values of names ──────────────────────────────────────────────────────

/-- The value at the end of a path of fields. -/
def pathVal (w : Val) (path : List String) : Option Val :=
  path.foldl (fun acc f => acc.bind (·.field? f)) (some w)

/-- The value a name and the fields under it have in an environment of the evaluation. -/
def symVal (ρ : Env) (s : Sym) : Option Val := (ρ.get? s.1).bind (pathVal · s.2)

theorem foldl_bind_none (path : List String) :
    path.foldl (fun (acc : Option Val) f => acc.bind (·.field? f)) none = none := by
  induction path with
  | nil => rfl
  | cons f rest ih => simp only [List.foldl_cons, Option.bind_none]; exact ih

theorem pathVal_nil (w : Val) : pathVal w [] = some w := rfl

theorem pathVal_cons (w : Val) (f : String) (rest : List String) :
    pathVal w (f :: rest) = (w.field? f).bind (pathVal · rest) := by
  unfold pathVal
  simp only [List.foldl_cons, Option.bind_some]
  rcases w.field? f with _ | u
  · simp [foldl_bind_none]
  · rfl

theorem pathVal_append (w : Val) (path : List String) (f : String) :
    pathVal w (path ++ [f]) = (pathVal w path).bind (·.field? f) := by
  unfold pathVal
  simp [List.foldl_append]

/-- A relation between two values: equal, or two numbers (two dates) in order. -/
def RelSem : Rel → Val → Val → Prop
  | .eq, x, y => x = y
  | .lt, .num a, .num b => a < b
  | .lt, .date a, .date b => a < b
  | .le, .num a, .num b => a ≤ b
  | .le, .date a, .date b => a ≤ b
  | _, _, _ => False

/-- **An environment of the evaluation among those a scope of the analysis says.** -/
structure Sat (A : ACtx) (ρ : Env) : Prop where
  env : ∀ x a, A.lookup x = some a → ∃ v, ρ.get? x = some v ∧ Mem a v
  facts : ∀ p r q, (p, r, q) ∈ A.facts → ∃ x y, symVal ρ p = some x ∧ symVal ρ q = some y ∧ RelSem r x y

theorem lookup_cons (A : ACtx) (x : String) (a : Abs) (y : String) :
    ({ A with env := (x, a) :: A.env } : ACtx).lookup y = if x = y then some a else A.lookup y := by
  unfold ACtx.lookup
  by_cases h : x = y
  · subst h; simp
  · simp [h]

theorem get_cons (ρ : Env) (x : String) (v : Val) (y : String) :
    Env.get? ((x, v) :: ρ) y = if x = y then some v else ρ.get? y := by
  unfold Env.get?
  by_cases h : x = y
  · subst h; simp
  · simp [h]

theorem symVal_cons_ne {ρ : Env} {x : String} {v : Val} {s : Sym} (h : s.1 ≠ x) :
    symVal ((x, v) :: ρ) s = symVal ρ s := by
  unfold symVal
  rw [get_cons]; simp [Ne.symm h]

/-- **A name the evaluation gives a value to.** -/
theorem Sat.bindNew {A : ACtx} {ρ : Env} (h : Sat A ρ) {x : String} {a : Abs} {v : Val} (hv : Mem a v) :
    Sat (A.bindNew x a) ((x, v) :: ρ) where
  env y b hb := by
    unfold ACtx.bindNew at hb
    have := lookup_cons A x a y
    simp only at this hb
    rw [show ACtx.lookup { A with env := (x, a) :: A.env, facts := A.facts.filter _ } y =
      ACtx.lookup { A with env := (x, a) :: A.env } y from rfl, this] at hb
    rw [get_cons]
    by_cases hxy : x = y
    · simp only [hxy, ite_true] at hb ⊢; cases hb; exact ⟨v, rfl, hv⟩
    · simp only [hxy, ite_false] at hb ⊢; exact h.env y b hb
  facts p r q hm := by
    unfold ACtx.bindNew at hm
    simp only [List.mem_filter, Bool.and_eq_true, bne_iff_ne, ne_eq] at hm
    obtain ⟨hm, hp, hq⟩ := hm
    obtain ⟨x', y', h1, h2, h3⟩ := h.facts p r q hm
    exact ⟨x', y', by rw [symVal_cons_ne hp]; exact h1, by rw [symVal_cons_ne hq]; exact h2, h3⟩

/-- The scope where a function starts: its parameters, each with what it takes. -/
theorem Sat.nil (M : Module) (results : List (Option Abs)) (ring : List Nat) : Sat ⟨M, results, ring, [], []⟩ [] where
  env x a h := by simp [ACtx.lookup] at h
  facts p r q h := by simp at h

-- ── The value of a name ─────────────────────────────────────────────────────

/-- The value of a name the analysis can narrow is the value of that name in the environment. -/
theorem sym_eval {C : Ctx} : ∀ {e : Expr} {s : Sym} {n dep : Nat} {ρ : Env} {v : Val},
    sym e = some s → eval C n dep ρ e = .ok v → symVal ρ s = some v
  | .var _ _ x, s, n, dep, ρ, v, hs, he => by
    simp only [sym] at hs; cases hs
    cases n with
    | zero => simp [eval] at he
    | succ n =>
      simp only [eval] at he
      split at he
      · rename_i w hw; cases he; simp [symVal, hw, pathVal_nil]
      · simp [failK] at he
  | .field _ _ b f, s, n, dep, ρ, v, hs, he => by
    simp only [sym] at hs
    rcases hb : sym b with _ | ⟨nm, fs⟩ <;> rw [hb] at hs <;> simp at hs
    subst hs
    cases n with
    | zero => simp [eval] at he
    | succ n =>
      simp only [eval, bind, Except.bind] at he
      rcases hbv : eval C n dep ρ b with e | bv <;> rw [hbv] at he
      · simp at he
      · simp only at he
        split at he
        · rename_i w hw
          cases he
          have := sym_eval hb hbv
          unfold symVal at this ⊢
          simp only at this ⊢
          rcases hg : ρ.get? nm with _ | g <;> rw [hg] at this <;> simp at this
          simp only [Option.bind_some, pathVal_append, this, Option.bind_some, hw]
        · simp [failK] at he
  | .num .., _, _, _, _, _, hs, _ | .date .., _, _, _, _, _, hs, _ | .str .., _, _, _, _, _, hs, _
  | .bool .., _, _, _, _, _, hs, _ | .none .., _, _, _, _, _, hs, _ | .enum .., _, _, _, _, _, hs, _
  | .neg .., _, _, _, _, _, hs, _ | .not .., _, _, _, _, _, hs, _ | .inUnit .., _, _, _, _, _, hs, _
  | .arith .., _, _, _, _, _, hs, _ | .ddiff .., _, _, _, _, _, hs, _ | .cmp .., _, _, _, _, _, hs, _
  | .and .., _, _, _, _, _, hs, _ | .or .., _, _, _, _, _, hs, _ | .ite .., _, _, _, _, _, hs, _
  | .mtch .., _, _, _, _, _, hs, _ | .letIn .., _, _, _, _, _, hs, _ | .block .., _, _, _, _, _, hs, _
  | .call .., _, _, _, _, _, hs, _ | .rule .., _, _, _, _, _, hs, _ | .dateCall .., _, _, _, _, _, hs, _
  | .transfer .., _, _, _, _, _, hs, _ | .record .., _, _, _, _, _, hs, _ | .list .., _, _, _, _, _, hs, _
  | .walk .., _, _, _, _, _, hs, _ | .reduce .., _, _, _, _, _, hs, _ | .minmax .., _, _, _, _, _, hs, _
  | .modOp .., _, _, _, _, _, hs, _ | .isqrt .., _, _, _, _, _, hs, _ | .allocate .., _, _, _, _, _, hs, _
  | .round .., _, _, _, _, _, hs, _ | .count1 .., _, _, _, _, _, hs, _ | .startsWith .., _, _, _, _, _, hs, _
  | .concat .., _, _, _, _, _, hs, _ | .text .., _, _, _, _, _, hs, _ | .bad .., _, _, _, _, _, hs, _ => by
    simp [sym] at hs

theorem Mem_path : ∀ (path : List String) {a : Abs} {w u : Val}, Mem a w → pathVal w path = some u →
    Mem (path.foldl (fun a f => a.field f) a) u
  | [], a, w, u, h, hp => by simp [pathVal_nil] at hp; subst hp; exact h
  | f :: rest, a, w, u, h, hp => by
    rw [pathVal_cons] at hp
    rcases hw : w.field? f with _ | w1 <;> rw [hw] at hp <;> simp at hp
    simp only [List.foldl_cons]
    exact Mem_path rest (Mem_field h hw) hp

/-- **What the scope says of a name holds of its value.** -/
theorem Sat.symAbs {A : ACtx} {ρ : Env} (h : Sat A ρ) {s : Sym} {a : Abs} {v : Val}
    (ha : A.symAbs s = some a) (hv : symVal ρ s = some v) : Mem a v := by
  unfold ACtx.symAbs at ha
  rcases hl : A.lookup s.1 with _ | a0 <;> rw [hl] at ha <;> simp at ha
  subst ha
  obtain ⟨w0, hw0, hm⟩ := h.env s.1 a0 hl
  unfold symVal at hv
  rw [hw0] at hv
  simp only [Option.bind_some] at hv
  exact Mem_path s.2 hm hv

-- ── Narrowing a name ─────────────────────────────────────────────────────────

theorem fieldOf_putMap {fs : List (String × Abs)} {f n : String} {g : Abs → Abs} :
    Abs.fieldOf (fs.map fun x => (x.1, if x.1 == f then g x.2 else x.2)) n =
      (Abs.fieldOf fs n).map (fun x => if n == f then g x else x) := by
  induction fs with
  | nil => rfl
  | cons p rest ih =>
    obtain ⟨k, x⟩ := p
    by_cases hk : k = n
    · subst hk; simp [Abs.fieldOf, List.find?]
    · have hk' : (k == n) = false := by simp [hk]
      simp only [Abs.fieldOf, List.map_cons, List.find?, hk'] at ih ⊢
      exact ih

/-- **Putting a field to what it is narrowed to keeps every value in.** -/
theorem putD_sound : ∀ (d : Nat) (v : Abs) (w : Val) (path : List String) (a : Abs),
    Mem v w → (∀ u, pathVal w path = some u → Mem a u) → Mem (putD d v path a) w
  | 0, v, w, path, a, h, _ => by simp only [putD]; exact h
  | d + 1, v, w, [], a, _, ha => by simp only [putD]; exact ha w (pathVal_nil w)
  | d + 1, v, w, f :: rest, a, h, ha => by
    cases h with
    | @record fs rels vs hf hr =>
      simp only [putD]
      refine Mem.record ?_ hr
      intro n x hx b hb
      rw [fieldOf_putMap (g := fun y => putD d y rest a)] at hb
      rcases hfo : Abs.fieldOf fs n with _ | b0 <;> rw [hfo] at hb <;> simp at hb
      subst hb
      have hm := hf n x hx b0 hfo
      split
      · rename_i hn
        have hn : n = f := by simpa using hn
        subst hn
        refine putD_sound d b0 x rest a hm ?_
        intro u hu
        apply ha u
        rw [pathVal_cons]
        simp only [Val.field?]
        unfold fieldV at hx
        rw [hx]; exact hu
      · exact hm
    | optNone => simp only [putD]; exact Mem.optNone
    | @optSome n x w hne hx =>
      simp only [putD]
      exact Mem.optSome hne (putD_sound d x w (f :: rest) a hx ha)
    | any => simp only [putD]; exact Mem.any _
    | num hi => simp only [putD]; exact Mem.num hi
    | date hi => simp only [putD]; exact Mem.date hi
    | enumAll => simp only [putD]; exact Mem.enumAll
    | boolAll => simp only [putD]; exact Mem.boolAll
    | enum hx => simp only [putD]; exact Mem.enum hx
    | bool hx => simp only [putD]; exact Mem.bool hx
    | str => simp only [putD]; exact Mem.str
    | list hc he => simp only [putD]; exact Mem.list hc he
    | transfer hf => simp only [putD]; exact Mem.transfer hf

/-- **A name narrowed to what its value is in.** -/
theorem Sat.narrowTo {A : ACtx} {ρ : Env} (h : Sat A ρ) {s : Sym} {a : Abs}
    (ha : ∀ w, symVal ρ s = some w → Mem a w) : Sat (A.narrowTo s a) ρ := by
  unfold ACtx.narrowTo
  split
  · rename_i v hv
    obtain ⟨w0, hw0, hm⟩ := h.env s.1 v hv
    refine ⟨?_, h.facts⟩
    intro y b hb
    unfold ACtx.bind at hb
    rw [lookup_cons] at hb
    by_cases hy : s.1 = y
    · simp only [hy, ite_true] at hb
      cases hb
      refine ⟨w0, hy ▸ hw0, ?_⟩
      refine putD_sound D v w0 s.2 a hm ?_
      intro u hu
      apply ha u
      unfold symVal; rw [hw0]; exact hu
    · simp only [hy, ite_false] at hb
      exact h.env y b hb
  · exact h

/-- Where a value is held to a range: the point of the value is in it. -/
def InRange (r : Range) (w : Val) : Prop := ∀ p, w.point = some p → EndLe r.min p ∧ EndGe r.max p

/-- **What holds once a place took a value**: the name keeps the range. -/
theorem Sat.assume {A : ACtx} {ρ : Env} (h : Sat A ρ) {x : Expr} {r : Range} {c : Option Conv}
    (hx : c = none → ∀ s, sym x = some s → ∀ w, symVal ρ s = some w → InRange r w) : Sat (KanameCore.assume A x r c) ρ := by
  unfold KanameCore.assume
  split
  · exact h
  · rename_i hc
    have hc' : c = none := by cases c <;> simp_all
    split
    · exact h
    · rename_i s hs
      split
      · rename_i i hi
        split
        · rename_i n hn
          refine h.narrowTo ?_
          intro w hw
          have hm := h.symAbs hi hw
          cases hm with
          | num hin =>
            rename_i p
            have hr := hx hc' s hs _ hw p rfl
            obtain ⟨m, hm', hmin⟩ := In_meet hin (⟨hr.1, hr.2, trivial⟩ : (⟨r.min, r.max, none⟩ : Iv).In p)
            rw [hn] at hm'; cases hm'
            exact Mem.num hmin
        · exact h
      · rename_i i hi
        split
        · rename_i n hn
          refine h.narrowTo ?_
          intro w hw
          have hm := h.symAbs hi hw
          cases hm with
          | date hin =>
            rename_i p
            have hr := hx hc' s hs _ hw p rfl
            obtain ⟨m, hm', hmin⟩ := In_meet hin (⟨hr.1, hr.2, trivial⟩ : (⟨r.min, r.max, none⟩ : Iv).In p)
            rw [hn] at hm'; cases hm'
            exact Mem.date hmin
        · exact h
      · exact h


-- ── Comparisons ─────────────────────────────────────────────────────────────

/-- What a comparison says of two numbers. -/
def OpRel : CmpOp → Rat → Rat → Prop
  | .lt, p, q => p < q
  | .le, p, q => p ≤ q
  | .gt, p, q => q < p
  | .ge, p, q => q ≤ p
  | .eq, p, q => p = q
  | .ne, p, q => p ≠ q

theorem holds_cmpRat (op : CmpOp) (p q : Rat) : op.holds (cmpRat p q) = true ↔ OpRel op p q := by
  by_cases h1 : p < q
  · have e : cmpRat p q = .lt := by simp [cmpRat, h1]
    have h3 : ¬ q < p := Rat.not_lt.mpr (Rat.le_of_lt h1)
    have h4 : ¬ q ≤ p := Rat.not_le.mpr h1
    rw [e]
    cases op <;> simp [CmpOp.holds, OpRel, h1, h3, h4, Rat.le_of_lt h1, Rat.ne_of_lt h1]
  · by_cases h2 : q < p
    · have e : cmpRat p q = .gt := by simp [cmpRat, h1, h2]
      have h4 : ¬ p ≤ q := Rat.not_le.mpr h2
      rw [e]
      cases op <;> simp [CmpOp.holds, OpRel, h1, h2, h4, Rat.le_of_lt h2, Rat.ne_of_gt h2]
    · have e : p = q := Rat.le_antisymm (Rat.not_lt.mp h2) (Rat.not_lt.mp h1)
      subst e
      have e2 : cmpRat p p = .eq := by simp [cmpRat]
      rw [e2]
      cases op <;> simp [CmpOp.holds, OpRel]

theorem holds_negate (op : CmpOp) (o : Ordering) : op.negate.holds o = !(op.holds o) := by
  cases op <;> cases o <;> rfl

theorem holds_truth {op : CmpOp} {o : Ordering} {t : Bool} (h : op.holds o = t) :
    (if t then op else op.negate).holds o = true := by
  cases t
  · simp only [Bool.false_eq_true, ite_false, holds_negate, h]; rfl
  · simp only [ite_true]; exact h

theorem holds_flip (op : CmpOp) (p q : Rat) : op.flip.holds (cmpRat q p) = op.holds (cmpRat p q) := by
  unfold cmpRat
  by_cases h1 : p < q
  · have : ¬ q < p := Rat.not_lt.mpr (Rat.le_of_lt h1)
    cases op <;> simp [CmpOp.flip, CmpOp.holds, h1, this] <;> rfl
  · by_cases h2 : q < p
    · cases op <;> simp [CmpOp.flip, CmpOp.holds, h1, h2] <;> rfl
    · cases op <;> simp [CmpOp.flip, CmpOp.holds, h1, h2] <;> rfl

theorem cmpInt_cast (a b : Int) : cmpInt a b = cmpRat a b := by
  unfold cmpInt cmpRat
  simp only [Rat.intCast_lt_intCast]

-- ── Steps ───────────────────────────────────────────────────────────────────

theorem div_lt_div_of_lt {x y k : Rat} (h : x < y) (hk : 0 < k) : x / k < y / k := by
  rw [Rat.div_def, Rat.div_def]
  have hi : 0 < k⁻¹ := Rat.inv_pos.2 hk
  have := Rat.mul_lt_mul_of_pos_right h hi
  exact this

theorem rlt_of_le_ne {a b : Rat} (h1 : a ≤ b) (h2 : a ≠ b) : a < b := by
  rcases Rat.le_iff_lt_or_eq.mp h1 with h | h
  · exact h
  · exact absurd h h2

theorem den_one {x : Rat} (h : x.den = 1) : x = (x.num : Rat) := by
  have := rat_eq_num_div_den x
  rw [h] at this
  have e : (x.num : Rat) / ((1 : Nat) : Rat) = (x.num : Rat) := by
    have : ((1 : Nat) : Rat) = 1 := rfl
    rw [this]; grind
  rw [e] at this
  exact this

/-- A value on a step that is not 0 is a whole number of it. -/
theorem onStep_int {s p : Rat} (hs : s ≠ 0) (h : onStep s p = true) : ∃ k : Int, p = (k : Rat) * s := by
  unfold onStep at h
  simp only [hs, ite_false, beq_iff_eq] at h
  refine ⟨(p / s).num, ?_⟩
  rw [← den_one h, Rat.div_mul_cancel hs]

theorem belowStep_ge {s p v : Rat} (hs : 0 < s) (hp : onStep s p = true) (h : p < v) : p ≤ belowStep (some s) v := by
  obtain ⟨k, rfl⟩ := onStep_int (Rat.ne_of_gt hs) hp
  simp only [belowStep]
  have hk : (k : Rat) < v / s := by
    have := Rat.mul_div_cancel (a := (k : Rat)) (Rat.ne_of_gt hs)
    rw [← this]; exact div_lt_div_of_lt h hs
  apply Rat.mul_le_mul_of_nonneg_right _ (Rat.le_of_lt hs)
  split
  · rename_i hf
    have : k < (v / s).floor := by
      have := Rat.intCast_lt_intCast.mp (show (k : Rat) < ((v / s).floor : Rat) by rw [hf]; exact hk)
      exact this
    exact Rat.intCast_le_intCast.mpr (by omega)
  · exact Rat.intCast_le_intCast.mpr (Rat.le_floor_iff.mpr (Rat.le_of_lt hk))

theorem aboveStep_le {s p v : Rat} (hs : 0 < s) (hp : onStep s p = true) (h : v < p) : aboveStep (some s) v ≤ p := by
  obtain ⟨k, rfl⟩ := onStep_int (Rat.ne_of_gt hs) hp
  simp only [aboveStep]
  have hk : v / s < (k : Rat) := by
    have := Rat.mul_div_cancel (a := (k : Rat)) (Rat.ne_of_gt hs)
    rw [← this]; exact div_lt_div_of_lt h hs
  apply Rat.mul_le_mul_of_nonneg_right _ (Rat.le_of_lt hs)
  have : (v / s).floor < k := Rat.intCast_lt_intCast.mp (rlt_of_le_of_lt (Rat.floor_le _) hk)
  exact Rat.intCast_le_intCast.mpr (by omega)

/-- On the step a narrowing goes by (a positive one), where there is one. -/
def OnStepF (step : Option Rat) (p : Rat) : Prop := ∀ s, step = some s → 0 < s → onStep s p = true

theorem minHi_ge {a b : End} {p : Rat} (ha : EndGe a p) (hb : EndGe b p) : EndGe (minHi a b) p := by
  cases a <;> cases b <;> simp only [minHi] <;> try assumption
  split <;> assumption

theorem maxLo_le {a b : End} {p : Rat} (ha : EndLe a p) (hb : EndLe b p) : EndLe (maxLo a b) p := by
  cases a <;> cases b <;> simp only [maxLo] <;> try assumption
  split <;> assumption

theorem filter_pos {step : Option Rat} {s : Rat} (h : step.filter (fun s => s.num > 0) = some s) :
    step = some s ∧ 0 < s := by
  cases step with
  | none => simp at h
  | some t =>
    simp only [Option.filter] at h
    split at h
    · rename_i hp; cases h; exact ⟨rfl, (num_pos_iff _).mp (by simpa using hp)⟩
    · simp at h

theorem check_ok {o : Iv} {p : Rat} (h : o.In p) :
    (match o.lo, o.hi with
      | some a, some b => if b < a then none else some o
      | _, _ => some o) = some o := by
  split
  · rename_i a b hlo hhi
    have h1 := h.lo; have h2 := h.hi
    rw [hlo] at h1; rw [hhi] at h2
    simp only [EndLe, EndGe] at h1 h2
    have : ¬ b < a := Rat.not_lt.mpr (Rat.le_trans h1 h2)
    simp [this]
  · rfl

/-- **The values of `x` where `x op y` holds**, those on the step. -/
theorem narrowIv_sound {x y : Iv} {op : CmpOp} {step : Option Rat} {p q : Rat} (hp : x.In p) (hq : y.In q)
    (hop : OpRel op p q) (hst : OnStepF step p) : ∃ n, narrowIv x op y step = some n ∧ n.In p := by
  have hpos : ∀ s, (step.filter fun s => s.num > 0) = some s → 0 < s ∧ onStep s p = true := by
    intro s hs; obtain ⟨h1, h2⟩ := filter_pos hs; exact ⟨h2, hst s h1 h2⟩
  have below' : ∀ (st : Option Rat), (∀ s, st = some s → 0 < s ∧ onStep s p = true) →
      ∀ v, p < v → p ≤ belowStep st v := by
    intro st hs v hv
    cases st with
    | none => exact Rat.le_of_lt hv
    | some s => obtain ⟨h1, h2⟩ := hs s rfl; exact belowStep_ge h1 h2 hv
  have above' : ∀ (st : Option Rat), (∀ s, st = some s → 0 < s ∧ onStep s p = true) →
      ∀ v, v < p → aboveStep st v ≤ p := by
    intro st hs v hv
    cases st with
    | none => exact Rat.le_of_lt hv
    | some s => obtain ⟨h1, h2⟩ := hs s rfl; exact aboveStep_le h1 h2 hv
  have below := below' _ hpos
  have above := above' _ hpos
  unfold narrowIv
  cases op with
  | lt =>
    have hin : ({ x with hi := minHi x.hi (y.hi.map (belowStep (step.filter fun s => s.num > 0))) } : Iv).In p := by
      refine ⟨hp.lo, minHi_ge hp.hi ?_, hp.den⟩
      have := hq.hi
      cases hy : y.hi with
      | none => trivial
      | some h =>
        rw [hy] at this
        exact below h (rlt_of_lt_of_le (show p < q from hop) this)
    exact ⟨_, check_ok hin, hin⟩
  | le =>
    have hin : ({ x with hi := minHi x.hi y.hi } : Iv).In p := by
      refine ⟨hp.lo, minHi_ge hp.hi ?_, hp.den⟩
      have := hq.hi
      cases hy : y.hi with
      | none => trivial
      | some h => rw [hy] at this; exact Rat.le_trans (show p ≤ q from hop) this
    exact ⟨_, check_ok hin, hin⟩
  | gt =>
    have hin : ({ x with lo := maxLo x.lo (y.lo.map (aboveStep (step.filter fun s => s.num > 0))) } : Iv).In p := by
      refine ⟨maxLo_le hp.lo ?_, hp.hi, hp.den⟩
      have := hq.lo
      cases hy : y.lo with
      | none => trivial
      | some l =>
        rw [hy] at this
        exact above l (rlt_of_le_of_lt this (show q < p from hop))
    exact ⟨_, check_ok hin, hin⟩
  | ge =>
    have hin : ({ x with lo := maxLo x.lo y.lo } : Iv).In p := by
      refine ⟨maxLo_le hp.lo ?_, hp.hi, hp.den⟩
      have := hq.lo
      cases hy : y.lo with
      | none => trivial
      | some l => rw [hy] at this; exact Rat.le_trans this (show q ≤ p from hop)
    exact ⟨_, check_ok hin, hin⟩
  | eq =>
    have hpq : p = q := hop
    subst hpq
    obtain ⟨m, hm, hin⟩ := In_meet hp hq
    simp only
    rw [hm]
    exact ⟨m, check_ok hin, hin⟩
  | ne =>
    have hpq : p ≠ q := hop
    rcases hv : y.isExact with _ | v
    · simp only []; exact ⟨x, check_ok hp, hp⟩
    · have hqv := isExact_sound hq hv
      subst hqv
      simp only []
      by_cases hc1 : (x.lo == some q && (step.filter fun s => s.num > 0).isSome) = true
      · simp only [hc1, ↓reduceIte]
        simp only [Bool.and_eq_true, beq_iff_eq, Option.isSome_iff_exists] at hc1
        obtain ⟨hl, s, hs⟩ := hc1
        have hin : ({ x with lo := some (aboveStep (step.filter fun s => s.num > 0) q) } : Iv).In p := by
          refine ⟨?_, hp.hi, hp.den⟩
          have h1 := hp.lo; rw [hl] at h1
          exact above q (rlt_of_le_ne h1 (Ne.symm hpq))
        exact ⟨_, check_ok hin, hin⟩
      · simp only [hc1, Bool.false_eq_true, ↓reduceIte]
        by_cases hc2 : (x.hi == some q && (step.filter fun s => s.num > 0).isSome) = true
        · simp only [hc2, ↓reduceIte]
          simp only [Bool.and_eq_true, beq_iff_eq, Option.isSome_iff_exists] at hc2
          obtain ⟨hh, s, hs⟩ := hc2
          have hin : ({ x with hi := some (belowStep (step.filter fun s => s.num > 0) q) } : Iv).In p := by
            refine ⟨hp.lo, ?_, hp.den⟩
            have h1 := hp.hi; rw [hh] at h1
            exact below q (rlt_of_le_ne h1 hpq)
          exact ⟨_, check_ok hin, hin⟩
        · simp only [hc2, Bool.false_eq_true, ↓reduceIte]
          exact ⟨x, check_ok hp, hp⟩


-- ── What a condition says of the environment ────────────────────────────────

/-- The value of an expression a branch narrows, as the environment has it: the name it is, and the
    count of the list a `count` counts. -/
structure SymOK (ρ : Env) (e : Expr) (x : Val) : Prop where
  sym : ∀ s, sym e = some s → symVal ρ s = some x
  count : ∀ l, counted e = some l → ∀ w, symVal ρ l = some w →
    x = .num (match w with | .list xs => (xs.length : Rat) | _ => 0)

/-- A value on the step its type says. -/
def StepOK (s : Option Rat) (x : Val) : Prop := ∀ st r, s = some st → x = .num r → onStep st r = true

/-- **What a condition that came out `t` says of the environment it was evaluated in.** -/
def NarHolds (ρ : Env) : Nar → Bool → Prop
  | .none, _ => True
  | .sym s, t => symVal ρ s = some (.bool t)
  | .cmp op a b ca cb sa sb va vb, t => ∃ x y line ord, SymOK ρ a x ∧ SymOK ρ b y ∧ Mem va x ∧ Mem vb y ∧
      StepOK sa x ∧ StepOK sb y ∧ cmpVals line op ca cb ord x y = .ok (.bool t)
  | .not n, t => NarHolds ρ n (!t)
  | .and n m, t => t = true → NarHolds ρ n true ∧ NarHolds ρ m true
  | .or n m, t => t = false → NarHolds ρ n false ∧ NarHolds ρ m false

theorem symOK_of_eval {C : Ctx} {e : Expr} {n dep : Nat} {ρ : Env} {v : Val} (h : eval C n dep ρ e = .ok v) :
    SymOK ρ e v where
  sym s hs := sym_eval hs h
  count l hl w hw := by
    cases e <;> simp [counted] at hl
    rename_i id lst
    cases n with
    | zero => simp [eval] at h
    | succ n =>
      simp only [eval, bind, Except.bind] at h
      rcases hl' : eval C n dep ρ lst with e' | lv <;> rw [hl'] at h <;> simp at h
      have := sym_eval hl hl'
      rw [hw] at this; cases this
      exact h.symm

theorem stepOK_of_stepped {s : Option Rat} {x : Val} {line : Nat} (h : stepped s x line = .ok ()) : StepOK s x := by
  intro st r hs hx
  subst hs hx
  simp only [stepped] at h
  split at h
  · assumption
  · simp [failK] at h

theorem cmp_num {line : Nat} {op : CmpOp} {ca cb : Option Conv} {ord : Order} {p q : Rat} {t : Bool}
    (h : cmpVals line op ca cb ord (.num p) (.num q) = .ok (.bool t)) :
    ∃ p' q', convN ca p = .ok p' ∧ convN cb q = .ok q' ∧ OpRel (if t then op else op.negate) p' q' := by
  simp only [cmpVals, bind, Except.bind] at h
  rcases hp : convN ca p with e | p' <;> rw [hp] at h <;> simp at h
  rcases hq : convN cb q with e | q' <;> rw [hq] at h <;> simp [] at h
  exact ⟨p', q', rfl, rfl, (holds_cmpRat _ _ _).mp (holds_truth h)⟩

theorem cmp_date {line : Nat} {op : CmpOp} {ca cb : Option Conv} {ord : Order} {p q : Int} {t : Bool}
    (h : cmpVals line op ca cb ord (.date p) (.date q) = .ok (.bool t)) :
    OpRel (if t then op else op.negate) p q := by
  simp only [cmpVals] at h
  cases h
  rw [cmpInt_cast] at *
  exact (holds_cmpRat _ _ _).mp (holds_truth rfl)

/-- Two enum values or two bools, compared for equality. -/
theorem cmp_set {line : Nat} {op : CmpOp} {ca cb : Option Conv} {ord : Order} {x y : Val} {t : Bool}
    (h : cmpVals line op ca cb ord x y = .ok (.bool t))
    (hx : (∃ e, x = .enum e) ∨ (∃ b, x = .bool b)) (hy : (∃ e, y = .enum e) ∨ (∃ b, y = .bool b)) :
    ((if t then op else op.negate) = .eq → x = y) ∧ ((if t then op else op.negate) = .ne → x ≠ y) := by
  rcases hx with ⟨e1, rfl⟩ | ⟨b1, rfl⟩ <;> rcases hy with ⟨e2, rfl⟩ | ⟨b2, rfl⟩
  · simp only [cmpVals] at h
    cases op <;> cases t <;> simp [CmpOp.negate] at h ⊢ <;> (try split at h) <;> simp_all
  · simp [cmpVals, failK] at h
  · simp [cmpVals, failK] at h
  · simp only [cmpVals] at h
    cases op <;> cases t <;> simp [CmpOp.negate, BEq.beq, Val.beq] at h ⊢ <;> simp_all

theorem opRel_flip {op : CmpOp} {p q : Rat} (h : OpRel op p q) : OpRel op.flip q p := by
  cases op <;> simp_all [OpRel, CmpOp.flip] <;> exact Ne.symm h

theorem convN_none {p : Rat} : convN none p = .ok p := rfl

theorem Mem_scaleBy_num {c : Option Conv} {a : Abs} {q q' : Rat} (h : Mem a (.num q)) (hc : convN c q = .ok q') :
    Mem (scaleBy c a) (.num q') := by
  cases c with
  | none => simp [convN] at hc; subst hc; exact h
  | some c =>
    obtain ⟨rfl, _⟩ := convN_ok hc
    exact Mem_scaled_num c.f c.shift h

theorem onStep_one_int (p : Int) : onStep 1 (p : Rat) = true := by
  unfold onStep
  have h1 : (1 : Rat) ≠ 0 := by decide
  simp only [h1, ite_false, beq_iff_eq]
  rw [show ((p : Rat) / 1) = (p : Rat) from by grind]
  exact Rat.den_intCast p

theorem stepF_one_int (p : Int) : OnStepF (some 1) (p : Rat) := by
  intro s hs _; simp at hs; subst hs; exact onStep_one_int p

theorem stepF_of_stepOK {s : Option Rat} {p : Rat} (h : StepOK s (.num p)) : OnStepF s p :=
  fun st hs _ => h st p hs rfl


theorem onStep_one_nat (k : Nat) : onStep 1 (k : Rat) = true := by
  have := onStep_one_int (k : Int)
  rwa [show ((k : Int) : Rat) = (k : Rat) from Rat.intCast_natCast k] at this

-- ── Narrowing one side ──────────────────────────────────────────────────────

/-- The word a value of an enum or a bool is in a set. -/
def strOf : Val → Option String
  | .enum e => some e
  | .bool b => some (if b then "true" else "false")
  | _ => none

theorem Mem_set_str {xs : Option (List String)} {w : Val} (h : Mem (.set xs) w) :
    ∃ str, strOf w = some str ∧ ∀ l, xs = some l → str ∈ l := by
  cases h with
  | enumAll => exact ⟨_, rfl, fun l h => by cases h⟩
  | boolAll => exact ⟨_, rfl, fun l h => by cases h⟩
  | enum hx => exact ⟨_, rfl, fun l h => by cases h; exact hx⟩
  | bool hx => exact ⟨_, rfl, fun l h => by cases h; exact hx⟩

theorem Mem_set_of_str {l : List String} {w : Val} {str : String} (hs : strOf w = some str) (hl : str ∈ l) :
    Mem (.set (some l)) w := by
  cases w <;> simp [strOf] at hs
  · subst hs; exact Mem.bool hl
  · subst hs; exact Mem.enum hl

/-- **One side narrowed by the other holds its value**, where the comparison holds. -/
theorem narrowedOf_sound {me other : Abs} {op : CmpOp} {step : Option Rat} {w : Val}
    (hnum : ∀ i j, me = .num i → other = .num j → ∃ p q, w = .num p ∧ i.In p ∧ j.In q ∧ OpRel op p q ∧ OnStepF step p)
    (hdate : ∀ i j, me = .date i → other = .date j → ∃ p q : Int, w = .date p ∧ i.In p ∧ j.In q ∧ OpRel op p q)
    (hset : ∀ xs v, me = .set xs → other = .set (some [v]) →
      ∃ str, strOf w = some str ∧ (∀ l, xs = some l → str ∈ l) ∧ (op = .eq → str = v) ∧ (op = .ne → str ≠ v)) :
    ∀ n, narrowedOf me other op step = some n → Mem n w := by
  intro n hn
  unfold narrowedOf at hn
  split at hn
  · rename_i i j
    obtain ⟨p, q, rfl, hp, hq, hop, hst⟩ := hnum i j rfl rfl
    obtain ⟨m, hm, hin⟩ := narrowIv_sound hp hq hop hst
    rw [hm] at hn; simp at hn; subst hn; exact Mem.num hin
  · rename_i i j
    obtain ⟨p, q, rfl, hp, hq, hop⟩ := hdate i j rfl rfl
    obtain ⟨m, hm, hin⟩ := narrowIv_sound hp hq hop (stepF_one_int p)
    rw [hm] at hn; simp at hn; subst hn; exact Mem.date hin
  · rename_i xs v
    obtain ⟨str, hs, hxs, heq, hne⟩ := hset xs v rfl rfl
    split at hn
    · cases hn
      have e := heq rfl
      subst e
      apply Mem_set_of_str hs
      cases xs with
      | none => simp
      | some l => simp [hxs l rfl]
    · cases xs with
      | none => simp at hn
      | some l =>
        simp at hn; subst hn
        apply Mem_set_of_str hs
        simp [hxs l rfl, hne rfl]
    · simp at hn
  · simp at hn

theorem Sat.narrowSide {A : ACtx} {ρ : Env} (h : Sat A ρ) {sy : Option Sym} {conv : Option Conv} {me other : Abs}
    {op : CmpOp} {step : Option Rat}
    (hs : ∀ s, sy = some s → conv = none → ∀ w, symVal ρ s = some w → ∀ n, narrowedOf me other op step = some n → Mem n w) :
    Sat (narrowSide A sy conv me other op step) ρ := by
  unfold KanameCore.narrowSide
  split
  · exact h
  · rename_i s
    split
    · exact h
    · rename_i hc
      have hc' : conv = none := by cases conv <;> simp_all
      split
      · rename_i n hn
        exact h.narrowTo (fun w hw => hs s rfl hc' w hw n hn)
      · exact h

-- ── The count of a list ─────────────────────────────────────────────────────

theorem Sat.narrowCount {A : ACtx} {ρ : Env} (h : Sat A ρ) {x : Expr} {vy : Abs} {op : CmpOp} {xv yv : Val}
    (hx : SymOK ρ x xv) (hy : Mem vy yv) (hop : ∀ p q, xv = .num p → yv = .num q → OpRel op p q) :
    Sat (narrowCount A x vy op) ρ := by
  unfold KanameCore.narrowCount
  split
  · rename_i list hlist
    split
    · rename_i _ c el other hs
      split
      · rename_i n hn
        refine h.narrowTo ?_
        intro w hw
        have hm := h.symAbs hs hw
        cases hm with
        | @list _ _ xs hc he =>
          have hxv := hx.count list hlist _ hw
          simp only at hxv
          cases hy with
          | @num _ q hq =>
            have := hop _ _ hxv rfl
            obtain ⟨n', hn', hin⟩ := narrowIv_sound (step := some 1) hc hq this
              (fun s hs _ => by simp at hs; subst hs; exact onStep_one_nat _)
            rw [hn] at hn'; cases hn'
            exact Mem.list hin he
      · exact h
    · exact h
  · exact h

theorem Sat.narrowCounts {A : ACtx} {ρ : Env} (h : Sat A ρ) {op : CmpOp} {a b : Expr} {ca cb : Option Conv}
    {va vb : Abs} {x y : Val} (hxa : SymOK ρ a x) (hyb : SymOK ρ b y) (hmx : Mem va x) (hmy : Mem vb y)
    (hop : ca = none → cb = none → ∀ p q, x = .num p → y = .num q → OpRel op p q) :
    Sat (narrowCounts A op a b ca cb va vb) ρ := by
  unfold KanameCore.narrowCounts
  split
  · rename_i hown
    simp only [Bool.and_eq_true, Option.isNone_iff_eq_none] at hown
    obtain ⟨h1, h2⟩ := hown
    have h' := h.narrowCount hxa hmy (fun p q hp hq => hop h1 h2 p q hp hq)
    exact h'.narrowCount hyb hmx (fun q p hq hp => opRel_flip (hop h1 h2 p q hp hq))
  · exact h

-- ── Facts ────────────────────────────────────────────────────────────────────

theorem Sat.addFact {A : ACtx} {ρ : Env} (h : Sat A ρ) {op : CmpOp} {syA syB : Option Sym} {ca cb : Option Conv}
    {xa xb : Abs}
    (hf : ∀ p q f, syA = some p → syB = some q → ca = none → cb = none →
      ((∃ i j, xa = .num i ∧ xb = .num j) ∨ (∃ i j, xa = .date i ∧ xb = .date j)) → factOf op p q = some f →
      ∃ x y, symVal ρ f.1 = some x ∧ symVal ρ f.2.2 = some y ∧ RelSem f.2.1 x y) :
    Sat (addFact A op syA syB ca cb xa xb) ρ := by
  unfold KanameCore.addFact
  simp only
  split
  · rename_i p q hcomp
    split
    · rename_i f hfact
      simp only [Bool.and_eq_true, Option.isNone_iff_eq_none] at hcomp
      obtain ⟨⟨h1, h2⟩, h3⟩ := hcomp
      have hk : (∃ i j, xa = .num i ∧ xb = .num j) ∨ (∃ i j, xa = .date i ∧ xb = .date j) := by
        revert h3; split <;> simp_all
      obtain ⟨x, y, hx, hy, hr⟩ := hf p q f rfl rfl h1 h2 hk hfact
      refine ⟨h.env, ?_⟩
      intro p' r q' hm
      simp only [List.mem_append, List.mem_singleton] at hm
      rcases hm with hm | hm
      · exact h.facts p' r q' hm
      · subst hm; exact ⟨x, y, hx, hy, hr⟩
    · exact h
  · exact h

theorem factOf_sound {op : CmpOp} {p q : Sym} {f : Sym × Rel × Sym} {ρ : Env} {x y : Val}
    (hp : symVal ρ p = some x) (hq : symVal ρ q = some y) (hf : factOf op p q = some f)
    (hrel : ∀ a b, x = .num a → y = .num b → OpRel op a b) (hrel' : ∀ a b : Int, x = .date a → y = .date b → OpRel op a b)
    (hk : (∃ a b, x = .num a ∧ y = .num b) ∨ (∃ a b : Int, x = .date a ∧ y = .date b)) :
    ∃ x' y', symVal ρ f.1 = some x' ∧ symVal ρ f.2.2 = some y' ∧ RelSem f.2.1 x' y' := by
  rcases hk with ⟨a, b, rfl, rfl⟩ | ⟨a, b, rfl, rfl⟩
  · have := hrel a b rfl rfl
    cases op with
    | eq => simp only [factOf] at hf; cases hf; exact ⟨_, _, hp, hq, by simp only [RelSem]; rw [show a = b from this]⟩
    | ne => simp [factOf] at hf
    | lt => simp only [factOf] at hf; cases hf; exact ⟨_, _, hp, hq, this⟩
    | le => simp only [factOf] at hf; cases hf; exact ⟨_, _, hp, hq, this⟩
    | gt => simp only [factOf] at hf; cases hf; exact ⟨_, _, hq, hp, this⟩
    | ge => simp only [factOf] at hf; cases hf; exact ⟨_, _, hq, hp, this⟩
  · have := hrel' a b rfl rfl
    cases op with
    | eq =>
      simp only [factOf] at hf; cases hf
      exact ⟨_, _, hp, hq, by simp only [RelSem]; rw [Rat.intCast_inj.mp (show (a : Rat) = b from this)]⟩
    | ne => simp [factOf] at hf
    | lt => simp only [factOf] at hf; cases hf; exact ⟨_, _, hp, hq, Rat.intCast_lt_intCast.mp this⟩
    | le => simp only [factOf] at hf; cases hf; exact ⟨_, _, hp, hq, Rat.intCast_le_intCast.mp this⟩
    | gt => simp only [factOf] at hf; cases hf; exact ⟨_, _, hq, hp, Rat.intCast_lt_intCast.mp this⟩
    | ge => simp only [factOf] at hf; cases hf; exact ⟨_, _, hq, hp, Rat.intCast_le_intCast.mp this⟩


-- ── A comparison ────────────────────────────────────────────────────────────

theorem scaleBy_num {c : Option Conv} {a : Abs} {j : Iv} (h : scaleBy c a = .num j) : ∃ i, a = .num i := by
  cases c with
  | none => exact ⟨j, h⟩
  | some c => cases a <;> simp [scaleBy, Abs.scaled] at h ⊢

theorem scaleBy_date {c : Option Conv} {a : Abs} {j : Iv} (h : scaleBy c a = .date j) : a = .date j := by
  cases c with
  | none => exact h
  | some c => cases a <;> simp [scaleBy, Abs.scaled] at h ⊢; exact h

theorem scaleBy_set {c : Option Conv} {a : Abs} {xs : Option (List String)} (h : scaleBy c a = .set xs) : a = .set xs := by
  cases c with
  | none => exact h
  | some c => cases a <;> simp [scaleBy, Abs.scaled] at h ⊢; exact h

theorem strOf_ne {x y : Val} {sx sy : String} (hx : strOf x = some sx) (hy : strOf y = some sy) (hne : x ≠ y)
    (hmix : ∀ e b, ¬ (x = .enum e ∧ y = .bool b) ∧ ¬ (x = .bool b ∧ y = .enum e)) : sx ≠ sy := by
  intro e; subst e
  cases x <;> cases y <;> simp [strOf] at hx hy
  · rename_i a b; subst hx; apply hne; cases a <;> cases b <;> simp_all
  · rename_i a b; exact (hmix b a).2 ⟨rfl, rfl⟩
  · rename_i a b; exact (hmix a b).1 ⟨rfl, rfl⟩
  · subst hx; subst hy; exact hne rfl

theorem str_kind {x : Val} {s : String} (h : strOf x = some s) : (∃ e, x = .enum e) ∨ (∃ b, x = .bool b) := by
  cases x <;> simp [strOf] at h
  · exact Or.inr ⟨_, rfl⟩
  · exact Or.inl ⟨_, rfl⟩

/-- **A comparison that came out `t` narrows the scope to one the environment is still in.** -/
theorem Sat.narrowCmp {A : ACtx} {ρ : Env} (h : Sat A ρ) {op : CmpOp} {a b : Expr} {ca cb : Option Conv}
    {sa sb : Option Rat} {va vb : Abs} {x y : Val} {line : Nat} {ord : Order} {t : Bool}
    (hxa : SymOK ρ a x) (hyb : SymOK ρ b y) (hmx : Mem va x) (hmy : Mem vb y) (hsx : StepOK sa x) (hsy : StepOK sb y)
    (hc : cmpVals line op ca cb ord x y = .ok (.bool t)) :
    Sat (narrowCmp A (if t then op else op.negate) a b ca cb sa sb va vb) ρ := by
  generalize hop' : (if t then op else op.negate) = op'
  have fnum : ∀ p q, x = .num p → y = .num q → ∃ p' q', convN ca p = .ok p' ∧ convN cb q = .ok q' ∧ OpRel op' p' q' := by
    intro p q hp hq; subst hp hq; rw [← hop']; exact cmp_num hc
  have fdate : ∀ p q : Int, x = .date p → y = .date q → OpRel op' p q := by
    intro p q hp hq; subst hp hq; rw [← hop']; exact cmp_date hc
  have fset : (∃ e, x = .enum e) ∨ (∃ b, x = .bool b) → (∃ e, y = .enum e) ∨ (∃ b, y = .bool b) →
      (op' = .eq → x = y) ∧ (op' = .ne → x ≠ y) := fun h1 h2 => hop' ▸ cmp_set hc h1 h2
  have fmix : ∀ e b, ¬ (x = .enum e ∧ y = .bool b) ∧ ¬ (x = .bool b ∧ y = .enum e) := by
    intro e b; constructor <;> rintro ⟨rfl, rfl⟩ <;> simp [cmpVals, failK] at hc
  have h1 : Sat (KanameCore.narrowCounts A op' a b ca cb va vb) ρ := h.narrowCounts hxa hyb hmx hmy (fun hca hcb p q hp hq => by
      obtain ⟨p', q', e1, e2, hr⟩ := fnum p q hp hq
      subst hca hcb; simp [convN] at e1 e2; subst e1 e2; exact hr)
  unfold KanameCore.narrowCmp
  simp only
  split
  · exact h1
  · refine Sat.narrowSide (Sat.narrowSide (h1.addFact ?_) ?_) ?_
    · -- the fact
      intro p q f hp hq hca hcb hk hf
      subst hca hcb
      have hpx := hxa.sym p hp
      have hqy := hyb.sym q hq
      refine factOf_sound hpx hqy hf ?_ fdate ?_
      · intro u v hu hv
        obtain ⟨p', q', e1, e2, hr⟩ := fnum u v hu hv
        simp [convN] at e1 e2; subst e1 e2; exact hr
      · simp only [scaleBy] at hk
        rcases hk with ⟨i, j, rfl, rfl⟩ | ⟨i, j, rfl, rfl⟩
        · cases hmx; cases hmy; exact Or.inl ⟨_, _, rfl, rfl⟩
        · cases hmx; cases hmy; exact Or.inr ⟨_, _, rfl, rfl⟩
    · -- side a, by side b read in the unit they are compared in
      intro s hs hca w hw n hn
      subst hca
      have hwx : w = x := by rw [hxa.sym s hs] at hw; cases hw; rfl
      subst hwx
      refine narrowedOf_sound ?_ ?_ ?_ n hn
      · intro i j hi hj
        simp only [scaleBy] at hi
        subst hi
        cases hmx with
        | @num _ p hp =>
          obtain ⟨i0, hi0⟩ := scaleBy_num hj
          subst hi0
          cases hmy with
          | @num _ q hq0 =>
            obtain ⟨p', q', e1, e2, hr⟩ := fnum p q rfl rfl
            simp [convN] at e1; subst e1
            have hm' := Mem_scaleBy_num (Mem.num hq0) e2
            rw [hj] at hm'
            cases hm' with
            | num hq => exact ⟨p, q', rfl, hp, hq, hr, stepF_of_stepOK hsx⟩
      · intro i j hi hj
        simp only [scaleBy] at hi
        subst hi
        have hj' := scaleBy_date hj
        subst hj'
        cases hmx with
        | @date _ p hp =>
          cases hmy with
          | @date _ q hq => exact ⟨p, q, rfl, hp, hq, fdate p q rfl rfl⟩
      · intro xs v hi hj
        simp only [scaleBy] at hi
        subst hi
        have hj' := scaleBy_set hj
        subst hj'
        obtain ⟨sx, hsx', hxs⟩ := Mem_set_str hmx
        obtain ⟨sy, hsy', hys⟩ := Mem_set_str hmy
        have hv : sy = v := by have := hys [v] rfl; simpa using this
        subst hv
        have hk := fset (str_kind hsx') (str_kind hsy')
        refine ⟨sx, hsx', hxs, ?_, ?_⟩
        · intro he; have := hk.1 he; subst this; rw [hsx'] at hsy'; cases hsy'; rfl
        · intro hn'; exact strOf_ne hsx' hsy' (hk.2 hn') fmix
    · -- side b, by side a
      intro s hs hcb w hw n hn
      subst hcb
      have hwy : w = y := by rw [hyb.sym s hs] at hw; cases hw; rfl
      subst hwy
      refine narrowedOf_sound ?_ ?_ ?_ n hn
      · intro i j hi hj
        simp only [scaleBy] at hi
        subst hi
        cases hmy with
        | @num _ q hq =>
          obtain ⟨i0, hi0⟩ := scaleBy_num hj
          subst hi0
          cases hmx with
          | @num _ p hp0 =>
            obtain ⟨p', q', e1, e2, hr⟩ := fnum p q rfl rfl
            simp [convN] at e2; subst e2
            have hm' := Mem_scaleBy_num (Mem.num hp0) e1
            rw [hj] at hm'
            cases hm' with
            | num hp => exact ⟨q, p', rfl, hq, hp, opRel_flip hr, stepF_of_stepOK hsy⟩
      · intro i j hi hj
        simp only [scaleBy] at hi
        subst hi
        have hj' := scaleBy_date hj
        subst hj'
        cases hmy with
        | @date _ q hq =>
          cases hmx with
          | @date _ p hp =>
            exact ⟨q, p, rfl, hq, hp, opRel_flip (fdate p q rfl rfl)⟩
      · intro xs v hi hj
        simp only [scaleBy] at hi
        subst hi
        have hj' := scaleBy_set hj
        subst hj'
        obtain ⟨sy, hsy', hys⟩ := Mem_set_str hmy
        obtain ⟨sx, hsx', hxs⟩ := Mem_set_str hmx
        have hv : sx = v := by have := hxs [v] rfl; simpa using this
        subst hv
        have hk := fset (str_kind hsx') (str_kind hsy')
        refine ⟨sy, hsy', hys, ?_, ?_⟩
        · intro he
          have he' : op' = .eq := by cases op' <;> simp_all [CmpOp.flip]
          have := hk.1 he'; subst this; rw [hsx'] at hsy'; cases hsy'; rfl
        · intro hn'
          have hn'' : op' = .ne := by cases op' <;> simp_all [CmpOp.flip]
          have := strOf_ne hsx' hsy' (hk.2 hn'') fmix
          exact Ne.symm this

/-- **What a condition narrows holds where it came out as it did.** -/
theorem Sat.apply : ∀ (nar : Nar) (t : Bool) {A : ACtx} {ρ : Env}, Sat A ρ → NarHolds ρ nar t → Sat (nar.apply t A) ρ
  | .none, _, A, ρ, h, _ => h
  | .sym s, t, A, ρ, h, hn => by
    simp only [Nar.apply]
    refine h.narrowTo ?_
    intro w hw
    simp only [NarHolds] at hn
    rw [hn] at hw; cases hw
    exact Mem_boolAbs t
  | .cmp op a b ca cb sa sb va vb, t, A, ρ, h, hn => by
    simp only [Nar.apply]
    obtain ⟨x, y, line, ord, hxa, hyb, hmx, hmy, hsx, hsy, hc⟩ := hn
    exact h.narrowCmp hxa hyb hmx hmy hsx hsy hc
  | .not n, t, A, ρ, h, hn => by
    simp only [Nar.apply]
    exact Sat.apply n (!t) h hn
  | .and n m, t, A, ρ, h, hn => by
    simp only [Nar.apply]
    split
    · rename_i ht
      obtain ⟨h1, h2⟩ := hn ht
      exact Sat.apply m true (Sat.apply n true h h1) h2
    · exact h
  | .or n m, t, A, ρ, h, hn => by
    simp only [Nar.apply]
    split
    · rename_i ht
      have ht' : t = false := by simpa using ht
      obtain ⟨h1, h2⟩ := hn ht'
      exact Sat.apply m false (Sat.apply n false h h1) h2
    · exact h


-- ── Relations ────────────────────────────────────────────────────────────────

theorem RelSem.compose {r1 r2 : Rel} {x w z : Val} (h1 : RelSem r1 x w) (h2 : RelSem r2 w z) :
    RelSem (r1.compose r2) x z := by
  cases r1 with
  | eq => simp only [RelSem] at h1; subst h1; cases r2 <;> exact h2
  | lt =>
    cases r2 with
    | eq => simp only [RelSem] at h2; subst h2; exact h1
    | lt =>
      simp only [Rel.compose]
      cases x <;> cases w <;> cases z <;> simp only [RelSem] at h1 h2 ⊢
      all_goals first | exact rlt_of_lt_of_le h1 (Rat.le_of_lt h2) | omega
    | le =>
      simp only [Rel.compose]
      cases x <;> cases w <;> cases z <;> simp only [RelSem] at h1 h2 ⊢
      all_goals first | exact rlt_of_lt_of_le h1 h2 | omega
  | le =>
    cases r2 with
    | eq => simp only [RelSem] at h2; subst h2; exact h1
    | lt =>
      simp only [Rel.compose]
      cases x <;> cases w <;> cases z <;> simp only [RelSem] at h1 h2 ⊢
      all_goals first | exact rlt_of_le_of_lt h1 h2 | omega
    | le =>
      simp only [Rel.compose]
      cases x <;> cases w <;> cases z <;> simp only [RelSem] at h1 h2 ⊢
      all_goals first | exact Rat.le_trans h1 h2 | omega

/-- Each edge: the values of its two ends keep its relation. -/
def EdgesOK (ρ : Env) (edges : List (Sym × Rel × Sym)) : Prop :=
  ∀ e ∈ edges, ∃ u w, symVal ρ e.1 = some u ∧ symVal ρ e.2.2 = some w ∧ RelSem e.2.1 u w

/-- What the strongest relations so far say: from the value `x0` to the value of each name. -/
def BestOK (ρ : Env) (x0 : Val) (best : List (Sym × Rel)) : Prop :=
  ∀ s r, (s, r) ∈ best → ∃ w, symVal ρ s = some w ∧ RelSem r x0 w

theorem bestGet_mem {best : List (Sym × Rel)} {s : Sym} {r : Rel} (h : bestGet best s = some r) : (s, r) ∈ best := by
  unfold bestGet at h
  rcases hf : best.find? (·.1 == s) with _ | ⟨k, v⟩ <;> rw [hf] at h <;> simp at h
  subst h
  have hm := List.mem_of_find?_eq_some hf
  have hk := List.find?_some hf
  simp at hk; subst hk; exact hm

theorem BestOK.put {ρ : Env} {x0 : Val} {best : List (Sym × Rel)} (h : BestOK ρ x0 best) {s : Sym} {r : Rel} {w : Val}
    (hw : symVal ρ s = some w) (hr : RelSem r x0 w) : BestOK ρ x0 (bestPut best s r) := by
  intro s' r' hm
  unfold bestPut at hm
  split at hm
  · obtain ⟨⟨k, v⟩, hkv, he⟩ := List.mem_map.mp hm
    simp only at he
    split at he
    · rename_i hk
      simp only [beq_iff_eq] at hk
      cases he; subst hk; exact ⟨w, hw, hr⟩
    · cases he; exact h _ _ hkv
  · rcases List.mem_append.mp hm with hm | hm
    · exact h _ _ hm
    · simp at hm; obtain ⟨rfl, rfl⟩ := hm; exact ⟨w, hw, hr⟩

/-- One candidate of an edge, where the edge's relation holds between its values. -/
theorem cand_ok {ρ : Env} {x0 : Val} {yEdge : Sym} (acc : List (Sym × Rel) × Bool) (src dst : Sym) (rel : Rel)
    (h : BestOK ρ x0 acc.1)
    (he : (src == yEdge && rel != .eq) = false → ∃ u w, symVal ρ src = some u ∧ symVal ρ dst = some w ∧ RelSem rel u w) :
    BestOK ρ x0 (if (src == yEdge && rel != .eq) = true then acc
      else match bestGet acc.1 src with
        | none => acc
        | some cur =>
          let next := cur.compose rel
          let better := match bestGet acc.1 dst with
            | none => true
            | some old => next.strength > old.strength
          if better then (bestPut acc.1 dst next, true) else acc).1 := by
  split
  · exact h
  · rename_i hskip
    have hskip' : (src == yEdge && rel != .eq) = false := by simpa using hskip
    obtain ⟨u, w, hu, hw, hr⟩ := he hskip'
    split
    · exact h
    · rename_i cur hcur
      obtain ⟨u', hu', hcu⟩ := h src cur (bestGet_mem hcur)
      rw [hu] at hu'; cases hu'
      simp only
      repeat' split
      all_goals first | exact h | exact h.put hw (RelSem.compose hcu hr)

theorem foldl_inv_mem {α β : Type} (P : β → Prop) (f : β → α → β) :
    ∀ (l : List α), (∀ b a, a ∈ l → P b → P (f b a)) → ∀ (b : β), P b → P (l.foldl f b)
  | [], _, _, h => h
  | a :: l, hf, b, h => foldl_inv_mem P f l (fun b' a' ha' hb' => hf b' a' (List.mem_cons_of_mem a ha') hb') (f b a)
      (hf b a (List.mem_cons_self) h)

theorem relPass_ok {ρ : Env} {x0 : Val} {edges : List (Sym × Rel × Sym)} (he : EdgesOK ρ edges)
    {best : List (Sym × Rel)} (h : BestOK ρ x0 best) : BestOK ρ x0 (relPass edges best).1 := by
  unfold relPass
  refine foldl_inv_mem (fun acc : List (Sym × Rel) × Bool => BestOK ρ x0 acc.1) _ edges ?_ (best, false) h
  rintro acc ⟨x, r, y⟩ hmem hacc
  obtain ⟨u, w, hu, hw, hr⟩ := he _ hmem
  simp only [List.foldl_cons, List.foldl_nil]
  have step1 := cand_ok (yEdge := y) acc x y r hacc (fun _ => ⟨u, w, hu, hw, hr⟩)
  refine cand_ok (yEdge := y) _ y x r step1 ?_
  intro hskip
  have hr' : r = .eq := by
    simp only [beq_self_eq_true, Bool.true_and, bne_eq_false_iff_eq] at hskip
    simpa using hskip
  subst hr'
  simp only [RelSem] at hr
  subst hr
  exact ⟨u, u, hw, hu, rfl⟩

theorem relIterate_ok {ρ : Env} {x0 : Val} {edges : List (Sym × Rel × Sym)} (he : EdgesOK ρ edges) :
    ∀ (n : Nat) {best : List (Sym × Rel)}, BestOK ρ x0 best → BestOK ρ x0 (relIterate edges n best)
  | 0, best, h => h
  | n + 1, best, h => by
    simp only [relIterate]
    have := relPass_ok (x0 := x0) he h
    split
    · exact relIterate_ok he n this
    · exact this

/-- The relations a record keeps between its fields hold between the values of those fields. -/
theorem recordEdges_ok {A : ACtx} {ρ : Env} (h : Sat A ρ) {s : Sym} {v : Val} (hv : symVal ρ s = some v) :
    EdgesOK ρ (recordEdges A s) := by
  unfold recordEdges
  split
  · intro e he; simp at he
  · rename_i last restRev hrev
    simp only
    split
    · rename_i fs rels hrec
      intro e he
      obtain ⟨⟨a, r, b⟩, hm, rfl⟩ := List.mem_map.mp he
      -- the record the fields are of
      have hs2 : s.2 = restRev.reverse ++ [last] := by
        have := congrArg List.reverse hrev
        simpa using this
      unfold symVal at hv
      rcases hg : ρ.get? s.1 with _ | g <;> rw [hg] at hv <;> simp only [Option.bind_none, Option.bind_some, reduceCtorEq] at hv
      rw [hs2, pathVal_append] at hv
      rcases hb : pathVal g restRev.reverse with _ | wb <;> rw [hb] at hv <;> simp at hv
      have hbase : symVal ρ (s.1, restRev.reverse) = some wb := by simp [symVal, hg, hb]
      have hm' := h.symAbs hrec hbase
      cases hm' with
      | @record _ _ vs hf hr =>
        obtain ⟨u, w, hu, hw, hrel⟩ := hr a r b hm
        refine ⟨u, w, ?_, ?_, ?_⟩
        · simp only [symVal, hg, Option.bind_some, pathVal_append, hb, Option.bind_some, Val.field?]
          exact hu
        · simp only [symVal, hg, Option.bind_some, pathVal_append, hb, Option.bind_some, Val.field?]
          exact hw
        · cases u <;> cases w <;> simp only [RelVals] at hrel <;> try exact absurd hrel id
          cases r <;> simp only [Rel.holds, decide_eq_true_eq, beq_iff_eq] at hrel <;> simp only [RelSem] <;>
            first | exact hrel | rw [hrel]
    · intro e he; simp at he

/-- **What a relation says of two names holds between their values.** -/
theorem relation_sound {A : ACtx} {ρ : Env} (h : Sat A ρ) {a b : Expr} {x y : Val} (ha : SymOK ρ a x) (hb : SymOK ρ b y)
    {r : Rel} (hr : relation A a b = some r) : RelSem r x y := by
  unfold relation at hr
  split at hr
  · rename_i sa sb hsa hsb
    have hx := ha.sym sa hsa
    have hy := hb.sym sb hsb
    simp only at hr
    split at hr
    · rename_i heq
      simp only [beq_iff_eq] at heq
      subst heq
      cases hr
      rw [hx] at hy; cases hy
      rfl
    · have hedges : EdgesOK ρ (A.facts ++ recordEdges A sa ++ recordEdges A sb) := by
        intro e he
        simp only [List.mem_append] at he
        rcases he with (he | he) | he
        · obtain ⟨u, w, hu, hw, hrel⟩ := h.facts e.1 e.2.1 e.2.2 he
          exact ⟨u, w, hu, hw, hrel⟩
        · exact recordEdges_ok h hx e he
        · exact recordEdges_ok h hy e he
      have hbest := relIterate_ok (x0 := x) hedges ((A.facts ++ recordEdges A sa ++ recordEdges A sb).length + 1)
        (best := [(sa, .eq)]) (by
          intro s' r' hm
          simp at hm; obtain ⟨rfl, rfl⟩ := hm
          exact ⟨x, hx, rfl⟩)
      obtain ⟨w, hw, hrel⟩ := hbest sb r (bestGet_mem hr)
      rw [hy] at hw; cases hw
      exact hrel
  · simp at hr


-- ── A difference of two names ────────────────────────────────────────────────

theorem step_ge {s r : Rat} (hs : 0 < s) (hr : onStep s r = true) (hpos : 0 < r) : s ≤ r := by
  obtain ⟨k, rfl⟩ := onStep_int (Rat.ne_of_gt hs) hr
  have hk : 0 < k := by
    by_cases h0 : 0 < k
    · exact h0
    · have : (k : Rat) ≤ 0 := Rat.intCast_le_intCast.mpr (by omega)
      have := Rat.mul_le_mul_of_nonneg_right this (Rat.le_of_lt hs)
      simp only [Rat.zero_mul] at this
      exact absurd hpos (Rat.not_lt.mpr this)
  have : (1 : Rat) ≤ k := by exact_mod_cast (show (1 : Int) ≤ k by omega)
  have := Rat.mul_le_mul_of_nonneg_right this (Rat.le_of_lt hs)
  simpa [Rat.one_mul] using this

theorem onStep_neg {s r : Rat} (hs : s ≠ 0) (hr : onStep s r = true) : onStep s (-r) = true := by
  unfold onStep at hr ⊢
  simp only [hs, ite_false, beq_iff_eq] at hr ⊢
  rw [Rat.div_def, Rat.neg_mul, den_neg, ← Rat.div_def]
  exact hr

theorem step_le_neg {s r : Rat} (hs : 0 < s) (hr : onStep s r = true) (hneg : r < 0) : r ≤ 0 - s := by
  have := step_ge hs (onStep_neg (Rat.ne_of_gt hs) hr) (by grind)
  grind

/-- **A difference of two names whose order a relation says.** -/
theorem subRel_sound {A : ACtx} {ρ : Env} (h : Sat A ρ) {a b : Expr} {p q : Rat}
    (ha : SymOK ρ a (.num p)) (hb : SymOK ρ b (.num q)) {step : Option Rat} {iv : Iv}
    (hin : iv.In (p - q)) (hst : OnStepF step (p - q)) : (subRel A a b step iv).In (p - q) := by
  have hpos : ∀ s, (step.filter fun s => s.num > 0) = some s → 0 < s ∧ onStep s (p - q) = true := by
    intro s hs; obtain ⟨h1, h2⟩ := filter_pos hs; exact ⟨h2, hst s h1 h2⟩
  have pos' : ∀ (st : Option Rat), (∀ s, st = some s → 0 < s ∧ onStep s (p - q) = true) →
      0 < p - q → st.getD 0 ≤ p - q := by
    intro st hs hp
    cases st with
    | none => exact Rat.le_of_lt hp
    | some s => obtain ⟨h1, h2⟩ := hs s rfl; exact step_ge h1 h2 hp
  have neg' : ∀ (st : Option Rat), (∀ s, st = some s → 0 < s ∧ onStep s (p - q) = true) →
      p - q < 0 → p - q ≤ 0 - st.getD 0 := by
    intro st hs hn
    cases st with
    | none => simp only [Option.getD_none]; grind
    | some s => obtain ⟨h1, h2⟩ := hs s rfl; exact step_le_neg h1 h2 hn
  have pos := pos' _ hpos
  have neg := neg' _ hpos
  have h1 : (match relation A b a with
      | some .eq => Iv.exact 0
      | some .le => { iv with lo := maxLo iv.lo (some 0) }
      | some .lt => { iv with lo := maxLo iv.lo (some ((step.filter fun s => s.num > 0).getD 0)) }
      | none => iv : Iv).In (p - q) := by
    split
    · rename_i hr
      have := relation_sound h hb ha hr
      simp only [RelSem] at this; cases this
      rw [show p - p = 0 by grind]; exact In_exact 0
    · rename_i hr
      have := relation_sound h hb ha hr
      simp only [RelSem] at this
      exact ⟨maxLo_le hin.lo (by simp only [EndLe]; grind), hin.hi, hin.den⟩
    · rename_i hr
      have := relation_sound h hb ha hr
      simp only [RelSem] at this
      exact ⟨maxLo_le hin.lo (by simp only [EndLe]; exact pos (by grind)), hin.hi, hin.den⟩
    · exact hin
  have second : ∀ iv1 : Iv, iv1.In (p - q) → (match relation A a b with
      | some .le => { iv1 with hi := minHi iv1.hi (some 0) }
      | some .lt => { iv1 with hi := minHi iv1.hi (some ((0 : Rat) - (step.filter fun s => s.num > 0).getD 0)) }
      | _ => iv1 : Iv).In (p - q) := by
    intro iv1 h1
    split
    · rename_i hr
      have := relation_sound h ha hb hr
      simp only [RelSem] at this
      exact ⟨h1.lo, minHi_ge h1.hi (by simp only [EndGe]; grind), h1.den⟩
    · rename_i hr
      have := relation_sound h ha hb hr
      simp only [RelSem] at this
      exact ⟨h1.lo, minHi_ge h1.hi (by simp only [EndGe]; exact neg (by grind)), h1.den⟩
    · exact h1
  unfold subRel
  exact second _ h1


-- ── An arm of a `match` ─────────────────────────────────────────────────────

/-- The patterns of values of an enum or bools keep the scope as it is. -/
theorem armFold_values {sy : Option Sym} {sv : Abs} :
    ∀ (ps : List Pat), (ps.all fun p => match p with | .bool _ | .enum _ => true | _ => false) = true →
      ∀ (acc : ACtx × Bool), ps.foldl (armStep sy sv) acc = acc
  | [], _, acc => rfl
  | p :: rest, h, acc => by
    simp only [List.all_cons, Bool.and_eq_true] at h
    obtain ⟨hp, hrest⟩ := h
    simp only [List.foldl_cons]
    have : armStep sy sv acc p = acc := by
      cases p with
      | bool b => rfl
      | enum v => rfl
      | none => simp at hp
      | some x => simp at hp
    rw [this]
    exact armFold_values rest hrest acc

theorem takes_value {v : Val} : ∀ {ps : List Pat} {bnd : Option String},
    (ps.all fun p => match p with | .bool _ | .enum _ => true | _ => false) = true →
    takesAny v ps = some bnd → bnd = none ∧ ∃ s, strOf v = some s ∧ s ∈ armValues ps
  | [], _, _, ht => by simp [takesAny] at ht
  | p :: rest, bnd, h, ht => by
    simp only [List.all_cons, Bool.and_eq_true] at h
    obtain ⟨hp, hrest⟩ := h
    simp only [takesAny] at ht
    split at ht
    · rename_i b hb
      cases ht
      cases p with
      | bool c =>
        cases v <;> simp [Pat.takes] at hb
        obtain ⟨rfl, rfl⟩ := hb
        exact ⟨rfl, _, rfl, by simp [armValues]⟩
      | enum n =>
        cases n with
        | none => simp [Pat.takes] at hb
        | some n =>
          cases v <;> simp [Pat.takes] at hb
          obtain ⟨rfl, rfl⟩ := hb
          exact ⟨rfl, _, rfl, by simp [armValues]⟩
      | none => simp at hp
      | some x => simp at hp
    · obtain ⟨h1, s, hs, hm⟩ := takes_value hrest ht
      refine ⟨h1, s, hs, ?_⟩
      unfold armValues at hm ⊢
      simp only [List.filterMap_cons]
      split <;> simp_all

theorem Mem_innerOf {sv : Abs} {v : Val} (hm : Mem sv v) (hv : v ≠ .none) : Mem (innerOf sv) v := by
  cases hm with
  | optNone => exact absurd rfl hv
  | optSome _ hx => exact hx
  | any => exact Mem.any _
  | num hi => exact Mem.num hi
  | date hi => exact Mem.date hi
  | enumAll => exact Mem.enumAll
  | boolAll => exact Mem.boolAll
  | enum hx => exact Mem.enum hx
  | bool hx => exact Mem.bool hx
  | str => exact Mem.str
  | list hc he => exact Mem.list hc he
  | record hf hr => exact Mem.record hf hr
  | transfer hf => exact Mem.transfer hf

theorem armOK_cases {ps : List Pat} (h : armOK ps = true) :
    ps = [.none] ∨ (∃ x, ps = [.some x]) ∨
      (ps.all fun p => match p with | .bool _ | .enum _ => true | _ => false) = true := by
  unfold armOK at h
  split at h
  · exact Or.inl rfl
  · exact Or.inr (Or.inl ⟨_, rfl⟩)
  · exact Or.inr (Or.inr h)

/-- **The arm the evaluation takes is reached, in a scope its environment is in.** -/
theorem armScope_sound {A : ACtx} {ρ : Env} (h : Sat A ρ) {sy : Option Sym} {sv : Abs} {ps : List Pat} {v : Val}
    {bnd : Option String} (hok : armOK ps = true) (ht : takesAny v ps = some bnd) (hm : Mem sv v)
    (hsy : ∀ s, sy = some s → symVal ρ s = some v) :
    (armScope A sy sv ps).2 = true ∧
      Sat (armScope A sy sv ps).1 (match bnd with | some x => (x, v) :: ρ | none => ρ) := by
  unfold armScope
  simp only
  rcases armOK_cases hok with rfl | ⟨x, rfl⟩ | hall
  · simp only [takesAny, Pat.takes] at ht
    cases v with
    | none =>
      simp at ht; subst ht
      have hc : (sv.mayBeNone || sv.isAny) = true := by
        rcases hm.none_cases with rfl | ⟨x, rfl⟩ <;> simp [Abs.mayBeNone, Abs.isAny]
      simp only [List.foldl_cons, List.foldl_nil, armStep, hc, ite_true, armValues, List.filterMap_cons,
        List.filterMap_nil, List.isEmpty_nil]
      refine ⟨trivial, ?_⟩
      cases sy with
      | none => exact h
      | some s => exact h.narrowTo (fun w hw => by rw [hsy s rfl] at hw; cases hw; exact Mem_none')
    | _ => simp at ht
  · simp only [takesAny, Pat.takes] at ht
    have hv : v ≠ .none := by intro e; subst e; simp at ht
    have hb : bnd = some x := by cases v <;> simp_all
    subst hb
    have hin := Mem_innerOf hm hv
    have hnn : innerOf sv ≠ .never := by
      intro e; rw [e] at hin; exact Mem.not_never hin
    simp only [List.foldl_cons, List.foldl_nil, armStep, armValues, List.filterMap_cons, List.filterMap_nil,
      List.isEmpty_nil, ite_true]
    exact ⟨trivial, h.bindNew hin⟩
  · obtain ⟨hb, str, hstr, hin⟩ := takes_value hall ht
    subst hb
    rw [armFold_values ps hall (A, false)]
    have hne : (armValues ps).isEmpty = false := by
      cases hav : armValues ps with
      | nil => rw [hav] at hin; simp at hin
      | cons _ _ => rfl
    simp only [hne, Bool.false_eq_true, ite_false]
    -- the value's word is among those the arm names and the value can be
    have hcan : str ∈ canValues sv (armValues ps) := by
      unfold canValues
      split
      · rename_i vs
        obtain ⟨s', hs', hl⟩ := Mem_set_str hm
        rw [hstr] at hs'; cases hs'
        simp [List.mem_filter, hin, hl vs rfl]
      · exact hin
    have hne2 : (canValues sv (armValues ps)).isEmpty = false := by
      cases hc : canValues sv (armValues ps) with
      | nil => rw [hc] at hcan; simp at hcan
      | cons _ _ => rfl
    simp only [hne2, Bool.false_eq_true, ite_false]
    refine ⟨trivial, ?_⟩
    cases sy with
    | none => exact h
    | some s =>
      refine h.narrowTo (fun w hw => ?_)
      rw [hsy s rfl] at hw; cases hw
      exact Mem_set_of_str hstr (mem_setOf.mpr hcan)

end KanameCore
