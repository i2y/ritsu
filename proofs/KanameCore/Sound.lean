/-
  The analysis of ranges is sound (kaname's DESIGN 15.2.2, 15.2.3).

  For an environment of the evaluation among those the scope of the analysis says (`Sat`), where
  the evaluation of an expression answers, its answer is among the values the analysis says the
  expression can be, the scope the analysis goes on in still holds the environment, and what the
  expression narrows as a condition holds where it came out as it did. Where the evaluation fails
  with one of the five kinds of the effects, the analysis kept a place that is not shown, or the
  failure is one of a function it calls with values its parameters take (`Wit`).

  What the layers kaname calls answer comes from outside the model (`OracleOK`): what rulec and
  koyomi show of their own (DESIGN 15.1). A function the analysis read the answer of answers
  within it (`ResultsOK`); `Total.lean` shows it does, ring by ring.

  The analysis of a `reduce` whose running value it lets go (DESIGN 5.5) is left out (DESIGN 15.4):
  the theorems are for an analysis that did not (`widened` is false).
-/
import KanameCore.Places

namespace KanameCore

-- ── What comes from outside ──────────────────────────────────────────────────

/-- What a rule and a date of a dates file answer, as rulec and koyomi show of their own: a rule's
    outputs among what each can be over every input it takes, its numbers within int64, and an
    answer for inputs in their ranges where it has no preconditions and no date input held to a
    range of days given; a date of a file within the first and last of its days and within the
    days from the date it reads, and an answer for whole-number inputs in their ranges where no
    date is given. -/
structure OracleOK (C : Ctx) : Prop where
  rule : ∀ r rd ins outs, C.M.rules[r]? = some rd → C.O.rule r ins = some outs →
    outs.length = rd.outputs.length →
    Mem (.record rd.outputs []) (.record ((rd.outputs.map (·.1)).zip outs)) ∧ ∀ x ∈ outs, ∀ q, x = .num q → fits q = true
  ruleAnswers : ∀ r rd ins, C.M.rules[r]? = some rd → rd.pres = [] → ins.length = rd.inputs.length →
    (∀ i v, (i, v) ∈ rd.inputs.zip ins → i.elements = none → i.range = none → i.dateRanged = true → v = .none) →
    (∀ i v, (i, v) ∈ rd.inputs.zip ins → i.refuses v = false) →
    ∃ outs, C.O.rule r ins = some outs
  date : ∀ file fd date ints d, C.M.dates[file]? = some fd → C.O.date file date ints = some d →
    (∀ dd, fd.dates.find? (·.name == date) = some dd →
      (∀ lo hi, dd.hull = some (lo, hi) → lo ≤ d ∧ d ≤ hi) ∧
      (∀ fewest most, dd.days = some (fewest, most) → ∀ inp g, inp ∈ fd.inputs → inp.isDate = true →
        dd.reads.contains inp.name = true → (inp.name, g) ∈ ints → fewest ≤ d - g ∧ d - g ≤ most))
  dateAnswers : ∀ file fd date ints, C.M.dates[file]? = some fd →
    (∀ nm g, (nm, g) ∈ ints → ∃ inp ∈ fd.inputs, inp.name = nm ∧ inp.isDate = false ∧ inp.min ≤ g ∧ g ≤ inp.max) →
    ∃ d, C.O.date file date ints = some d

/-- A function the analysis read the answer of answers within it, called with values its
    parameters take. A function that takes functions is not read so: a call takes what it declares
    (kaname's DESIGN 27.4). -/
def ResultsOK (C : Ctx) (res : List (Option Abs)) : Prop :=
  ∀ f a, res[f]? = some (some a) → ∀ fn, C.M.fns[f]? = some fn → fn.takesFn = false → ∀ n dep args v, Valid fn args →
    callFn C n dep f args = .ok v → Mem a v

/-- A scope of the analysis of a function of the module, with the answers of the functions
    analysed before it. -/
def Good (C : Ctx) (res : List (Option Abs)) (A : ACtx) : Prop := A.M = C.M ∧ A.results = res

theorem Good.same {C : Ctx} {res : List (Option Abs)} {A B : ACtx} (h : Good C res A) (s : Same A B) : Good C res B :=
  ⟨s.1.trans h.1, s.2.1.trans h.2⟩

-- ── int64 ────────────────────────────────────────────────────────────────────

theorem fits_bounds {r : Rat} (h : fits r = true) : (i64min : Rat) ≤ r ∧ r ≤ (i64max : Rat) := by
  unfold fits at h
  simp only [Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨h1, h2⟩, _⟩ := h
  have hd : (1 : Rat) ≤ (r.den : Rat) := by exact_mod_cast r.den_pos
  have hnum : ((r.num : Int) : Rat) = r * (r.den : Rat) := by
    have : (r.num : Rat) / r.den * r.den = r.num := Rat.div_mul_cancel (den_ne_zero r)
    rw [← rat_eq_num_div_den r] at this
    exact this.symm
  have c1 : ((i64min : Int) : Rat) ≤ ((r.num : Int) : Rat) := Rat.intCast_le_intCast.mpr h1
  have c2 : ((r.num : Int) : Rat) ≤ ((i64max : Int) : Rat) := Rat.intCast_le_intCast.mpr h2
  rcases Rat.le_total (a := 0) (b := r) with hr | hr
  · constructor
    · have : ((i64min : Int) : Rat) ≤ 0 := by decide
      exact Rat.le_trans this hr
    · have : r ≤ r * (r.den : Rat) := by
        have := Rat.mul_le_mul_of_nonneg_left hd hr
        simpa [Rat.mul_one] using this
      rw [← hnum] at this
      exact Rat.le_trans this c2
  · constructor
    · have : r * (r.den : Rat) ≤ r := by
        have hn : 0 ≤ -r := by grind
        have := Rat.mul_le_mul_of_nonneg_left hd hn
        grind
      rw [← hnum] at this
      exact Rat.le_trans c1 this
    · have : (0 : Rat) ≤ ((i64max : Int) : Rat) := by decide
      exact Rat.le_trans hr this

-- ── Places that hold a value to int64, and values read in another unit ──────

theorem heldA_mem {o : Out} {e : Nat} {a : Abs} {r : Rat} (h : Mem a (.num r)) (hf : fits r = true) :
    Mem (heldA o e a).1 (.num r) := by
  simp only [heldA, heldAbs]
  split
  · rename_i i
    cases h with
    | num hi =>
      obtain ⟨b1, b2⟩ := fits_bounds hf
      obtain ⟨m, hm, hin⟩ := In_meet hi (⟨b1, b2, trivial⟩ : (⟨some i64min, some i64max, none⟩ : Iv).In r)
      rw [hm]
      simp only [Option.map_some, Option.getD_some]
      exact Mem.num ⟨hin.lo, hin.hi, hi.den⟩
  · exact h

theorem heldA_wit {C : Ctx} {o : Out} {e : Nat} {a : Abs} {r : Rat} (h : Mem a (.num r)) (hf : fits r = false) :
    Wit C (heldA o e a).2 := by
  simp only [heldA]
  exact wit_place h ⟨r, rfl, hf⟩

theorem heldAbs_other {a : Abs} (h : ∀ i, a ≠ .num i) : heldAbs a = a := by
  cases a <;> simp_all [heldAbs]

theorem convA_fst {o : Out} {c : Option Conv} {a : Abs} : (convA o c a).1 = scaleBy c a := by
  cases c with
  | none => rfl
  | some c =>
    simp only [convA, scaleBy]
    split
    · split <;> rfl
    · split <;> rfl

theorem convA_mem {o : Out} {c : Option Conv} {a : Abs} {v v' : Val} (h : Mem a v) (hc : coerce c v = .ok v') :
    Mem (convA o c a).1 v' := by
  rw [convA_fst]; exact Mem_coerce h hc

theorem convA_wit {C : Ctx} {o : Out} {c : Conv} {a : Abs} {v : Val} {e : Stop} (h : Mem a v)
    (hc : coerceV c v = .error e) : e = .failed .overflow c.line ∧ Wit C (convA o (some c) a).2 := by
  obtain ⟨he, r, hm, hr⟩ := Mem_coerceV_error h hc
  refine ⟨he, ?_⟩
  simp only [convA]
  split
  · rename_i i hi
    split
    · rename_i hfit
      exact absurd (fits_sound (ivDeep_numPart hm hi) hfit) (by rw [hr]; simp)
    · exact wit_place hm ⟨r, rfl, hr⟩
  · rename_i hi
    rw [hasAny_numPart hm hi]
    exact wit_place hm ⟨r, rfl, hr⟩

theorem convN_coerce {c : Option Conv} {p p' : Rat} (h : convN c p = .ok p') : coerce c (.num p) = .ok (.num p') := by
  cases c with
  | none => simp [convN] at h; subst h; rfl
  | some c => simp only [coerce, coerceV, h]; rfl

theorem convN_mem {o : Out} {c : Option Conv} {a : Abs} {p p' : Rat} (h : Mem a (.num p)) (hc : convN c p = .ok p') :
    Mem (convA o c a).1 (.num p') := convA_mem h (convN_coerce hc)

theorem convN_wit {C : Ctx} {o : Out} {c : Option Conv} {a : Abs} {p : Rat} {e : Stop} (h : Mem a (.num p))
    (hc : convN c p = .error e) : e = .failed .overflow (match c with | some c => c.line | none => 0) ∧ Wit C (convA o c a).2 := by
  cases c with
  | none => simp [convN] at hc
  | some c =>
    have : coerceV c (.num p) = .error e := by
      simp only [coerceV, Functor.map, Except.map, hc]
    exact convA_wit h this

theorem coerce_wit {C : Ctx} {o : Out} {c : Option Conv} {a : Abs} {v : Val} {e : Stop} (h : Mem a v)
    (hc : coerce c v = .error e) : (∃ c' line, c = some c' ∧ e = .failed .overflow line) ∧ Wit C (convA o c a).2 := by
  cases c with
  | none => simp [coerce] at hc
  | some c =>
    obtain ⟨h1, h2⟩ := convA_wit h hc
    exact ⟨⟨c, c.line, rfl, h1⟩, h2⟩


-- ── The steps of the evaluation ──────────────────────────────────────────────

theorem bind_ok {α β : Type} {x : R α} {f : α → R β} {v : β} (h : (x >>= f) = .ok v) : ∃ y, x = .ok y ∧ f y = .ok v := by
  rcases hx : x with e | y <;> rw [hx] at h
  · cases h
  · exact ⟨y, rfl, h⟩

theorem bind_err {α β : Type} {x : R α} {f : α → R β} {e : Stop} (h : (x >>= f) = .error e) :
    x = .error e ∨ ∃ y, x = .ok y ∧ f y = .error e := by
  rcases hx : x with e' | y <;> rw [hx] at h
  · cases h; exact Or.inl rfl
  · exact Or.inr ⟨y, rfl, h⟩

theorem map_ok {α β : Type} {x : R α} {f : α → β} {v : β} (h : (f <$> x) = .ok v) : ∃ y, x = .ok y ∧ f y = v := by
  rcases hx : x with e | y <;> rw [hx] at h
  · cases h
  · simp only [Functor.map, Except.map] at h; cases h; exact ⟨y, rfl, rfl⟩

theorem map_err {α β : Type} {x : R α} {f : α → β} {e : Stop} (h : (f <$> x) = .error e) : x = .error e := by
  rcases hx : x with e' | y <;> rw [hx] at h
  · simp only [Functor.map, Except.map] at h; cases h; rfl
  · simp [Functor.map, Except.map] at h

theorem liftA_ok {line : Nat} {x : Except Arith Rat} {v : Rat} (h : liftA line x = .ok v) : x = .ok v := by
  cases x with
  | ok y => simp [liftA] at h; subst h; rfl
  | error a => cases a <;> simp [liftA] at h

theorem liftA_err {line : Nat} {x : Except Arith Rat} {e : Stop} (h : liftA line x = .error e) :
    (x = .error .overflow ∧ e = .failed .overflow line) ∨ (x = .error .zero ∧ e = .failed .zero line) := by
  cases x with
  | ok y => simp [liftA] at h
  | error a => cases a <;> simp [liftA] at h <;> subst h <;> simp

theorem held_ok {r v : Rat} (h : held r = .ok v) : v = r ∧ fits r = true := by
  unfold held at h; split at h <;> simp_all

theorem held_err {r : Rat} {a : Arith} (h : held r = .error a) : a = .overflow ∧ fits r = false := by
  unfold held at h; split at h <;> simp_all

theorem failK_ne_ok {α : Type} {k : Kind} {line : Nat} {v : α} : (failK k line : R α) ≠ .ok v := by simp [failK]

theorem failK_err {α : Type} {k k' : Kind} {line l : Nat} (h : (failK k line : R α) = .error (.failed k' l)) : k' = k := by
  simp [failK] at h; exact h.1.symm

-- ── What the evaluation of an expression comes to, as its analysis says ──────

/-- The values of a list of expressions, each among what its abstract value says. -/
def AllMem : List Abs → List Val → Prop
  | [], [] => True
  | a :: as, v :: vs => Mem a v ∧ AllMem as vs
  | _, _ => False

/-- The value each expression of a list came to, as the environment has the names among them. -/
def ArgSyms (ρ : Env) : List Expr → List Val → Prop
  | [], [] => True
  | x :: xs, v :: vs => (∀ s, sym x = some s → symVal ρ s = some v) ∧ ArgSyms ρ xs vs
  | _, _ => False

/-- An evaluation and the analysis of the same expression: an answer among the values the analysis
    says, in a scope that still holds the environment, narrowing what holds; a failure of one of
    the five kinds kept as evidence. -/
def Concl (C : Ctx) (ρ : Env) (res : R Val) (r : AR) : Prop :=
  (∀ v, res = .ok v → Mem r.abs v ∧ Sat r.ctx ρ ∧ ∀ t, v = .bool t → NarHolds ρ r.nar t) ∧
  (∀ k l, res = .error (.failed k l) → k.ofEffect = true → Wit C r.out)

/-- What the function given to a walk answered for an element, as the analysis of its body says. -/
def AnsOK (ρ : Env) (x : String) (r : AR) (c : Option Conv) (el a : Val) : Prop :=
  ∃ raw, Mem r.abs raw ∧ Sat r.ctx ((x, el) :: ρ) ∧ (∀ t, raw = .bool t → NarHolds ((x, el) :: ρ) r.nar t) ∧
    coerce c raw = .ok a

/-- **What the analysis says of every evaluation at `n` steps.** -/
structure SoundAt (C : Ctx) (res : List (Option Abs)) (n : Nat) : Prop where
  expr : ∀ (m : Nat) (A : ACtx) (ρ : Env) (e : Expr) (o : Out) (dep : Nat), Good C res A → Sat A ρ →
    (ev A m e o).out.widened = false → Concl C ρ (eval C n dep ρ e) (ev A m e o)
  args : ∀ (m : Nat) (A : ACtx) (ρ : Env) (args : List (Nat × Expr × Option Conv)) (o : Out) (dep : Nat),
    Good C res A → Sat A ρ → (evArgs A m args o).2.1.widened = false →
    (∀ vs, evalArgs C n dep ρ args = .ok vs → ∃ avs, (evArgs A m args o).1 = some avs ∧ AllMem avs vs ∧
        ArgSyms ρ (args.map (·.2.1)) vs ∧ Sat (evArgs A m args o).2.2 ρ) ∧
    (∀ k l, evalArgs C n dep ρ args = .error (.failed k l) → k.ofEffect = true → Wit C (evArgs A m args o).2.1)
  args5 : ∀ (m : Nat) (A : ACtx) (ρ : Env) (args : List (String × Expr × Option Conv × Nat × Bool)) (o : Out) (dep : Nat),
    Good C res A → Sat A ρ → (evArgs5 A m args o).2.1.widened = false →
    (∀ vs, evalArgs4 C n dep ρ args = .ok vs → ∃ avs, (evArgs5 A m args o).1 = some avs ∧ AllMem avs vs ∧
        Sat (evArgs5 A m args o).2.2 ρ) ∧
    (∀ k l, evalArgs4 C n dep ρ args = .error (.failed k l) → k.ofEffect = true → Wit C (evArgs5 A m args o).2.1)
  inits : ∀ (m : Nat) (A : ACtx) (ρ : Env) (inits : List (Nat × Expr × Option Conv × Nat)) (o : Out) (dep : Nat),
    Good C res A → Sat A ρ → (evInits A m inits o).2.1.widened = false →
    (∀ vs, evalInits C n dep ρ inits = .ok vs → ∃ avs, (evInits A m inits o).1 = some avs ∧ AllMem avs vs ∧
        ArgSyms ρ (inits.map (·.2.1)) vs ∧ Sat (evInits A m inits o).2.2 ρ) ∧
    (∀ k l, evalInits C n dep ρ inits = .error (.failed k l) → k.ofEffect = true → Wit C (evInits A m inits o).2.1)
  list : ∀ (m : Nat) (A : ACtx) (ρ : Env) (args : List Expr) (o : Out) (dep : Nat),
    Good C res A → Sat A ρ → (evList A m args o).2.1.widened = false →
    (∀ vs, evalList C n dep ρ args = .ok vs → ∃ avs, (evList A m args o).1 = some avs ∧ AllMem avs vs ∧
        Sat (evList A m args o).2.2 ρ) ∧
    (∀ k l, evalList C n dep ρ args = .error (.failed k l) → k.ofEffect = true → Wit C (evList A m args o).2.1)
  elems : ∀ (m : Nat) (A : ACtx) (ρ : Env) (elems : List (Expr × Option Conv)) (elem : Abs) (k : Nat) (o : Out)
      (dep : Nat) (pre : List Val), Good C res A → Sat A ρ → (listAbs A m elems elem k o).out.widened = false →
    (∀ x ∈ pre, Mem elem x) → pre.length = k →
    (∀ vs, evalElems C n dep ρ elems = .ok vs → Mem (listAbs A m elems elem k o).abs (.list (pre ++ vs)) ∧
        Sat (listAbs A m elems elem k o).ctx ρ) ∧
    (∀ k' l, evalElems C n dep ρ elems = .error (.failed k' l) → k'.ofEffect = true → Wit C (listAbs A m elems elem k o).out)
  each : ∀ (m : Nat) (A : ACtx) (ρ : Env) (x : String) (body : Expr) (c : Option Conv) (w : Walk) (xs : List Val)
      (elem : Abs) (o : Out) (dep : Nat), Good C res A → Sat A ρ → (∀ el ∈ xs, Mem elem el) →
    (ev (A.bindNew x elem) m body o).out.widened = false →
    (∀ as, evalEach C n dep ρ x body c w xs = .ok as →
      as.length ≤ xs.length ∧ ((∀ a ∈ as, stopAt w a = false) → as.length = xs.length) ∧ (xs ≠ [] → as ≠ []) ∧
      ∀ el a, (el, a) ∈ xs.zip as → AnsOK ρ x (ev (A.bindNew x elem) m body o) c el a) ∧
    (∀ k l, evalEach C n dep ρ x body c w xs = .error (.failed k l) → k.ofEffect = true →
      Wit C (convA (ev (A.bindNew x elem) m body o).out c (ev (A.bindNew x elem) m body o).abs).2)
  pairs : ∀ (m : Nat) (A : ACtx) (ρ : Env) (pairs : List (Expr × Option Conv × Expr × Option Conv)) (ks vs : Abs)
      (k : Nat) (o : Out) (dep : Nat) (pre : List (Val × Val)) (key : Key), Good C res A → Sat A ρ →
    (pairsAbs A m pairs ks vs k o).out.widened = false → (∀ p ∈ pre, Mem ks p.1 ∧ Mem vs p.2) → pre.length = k →
    (∀ ps, evalPairs C n dep ρ pairs = .ok ps →
        Mem (pairsAbs A m pairs ks vs k o).abs (.dict (dictFrom key (pre ++ ps))) ∧ Sat (pairsAbs A m pairs ks vs k o).ctx ρ) ∧
    (∀ k' l, evalPairs C n dep ρ pairs = .error (.failed k' l) → k'.ofEffect = true →
      Wit C (pairsAbs A m pairs ks vs k o).out)
  each2 : ∀ (m : Nat) (A : ACtx) (ρ : Env) (x : String) (kb : Expr) (kc : Option Conv) (y : String) (vb : Expr)
      (vc : Option Conv) (xs : List Val) (elem : Abs) (o : Out) (dep : Nat), Good C res A → Sat A ρ →
    (∀ el ∈ xs, Mem elem el) → (lam1 A m y elem vb vc (lam1 A m x elem kb kc o).2).2.widened = false →
    (∀ ps, evalEach2 C n dep ρ x kb kc y vb vc xs = .ok ps → ps.length = xs.length ∧
      ∀ p ∈ ps, Mem (lam1 A m x elem kb kc o).1 p.1 ∧ Mem (lam1 A m y elem vb vc (lam1 A m x elem kb kc o).2).1 p.2) ∧
    (∀ k l, evalEach2 C n dep ρ x kb kc y vb vc xs = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam1 A m y elem vb vc (lam1 A m x elem kb kc o).2).2)
  merge : ∀ (m : Nat) (A : ACtx) (ρ : Env) (key : Key) (ps : List (Val × Val)) (x y : String) (body : Expr)
      (c : Option Conv) (va vb : Abs) (qs : List (Val × Val)) (o : Out) (dep : Nat), Good C res A → Sat A ρ →
    (∀ p ∈ ps, Mem va p.2) → (∀ q ∈ qs, Mem vb q.2) → (lam2 A m x y va vb body c o).2.widened = false →
    (∀ more, evalMerge C n dep ρ key ps x y body c qs = .ok more → more.map (·.1) = qs.map (·.1) ∧
      ∀ q ∈ more, Mem vb q.2 ∨ Mem (lam2 A m x y va vb body c o).1 q.2) ∧
    (∀ k l, evalMerge C n dep ρ key ps x y body c qs = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam2 A m x y va vb body c o).2)
  fix : ∀ (m : Nat) (A : ACtx) (ρ : Env) (acc x : String) (body : Expr) (c : Option Conv) (S elem : Abs)
      (xs : List Val) (a : Val) (o : Out) (dep : Nat), Good C res A → Sat A ρ → (∀ el ∈ xs, Mem elem el) → Mem S a →
    (S.join (lam2 A m acc x S elem body c o).1 == S) = true → (lam2 A m acc x S elem body c o).2.widened = false →
    (∀ fin, evalFold C n dep ρ acc x body c xs a = .ok fin → Mem S fin) ∧
    (∀ k l, evalFold C n dep ρ acc x body c xs a = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam2 A m acc x S elem body c o).2)
  rounds : ∀ (m : Nat) (A : ACtx) (ρ : Env) (acc x : String) (body : Expr) (c : Option Conv) (elem : Abs) (K : Nat)
      (S : Abs) (xs : List Val) (a : Val) (o : Out) (dep : Nat), Good C res A → Sat A ρ → (∀ el ∈ xs, Mem elem el) →
    Mem S a → (xs.length ≤ K ∨ (reduceRounds A m acc x body c elem K S o).1.2 = true) →
    (reduceRounds A m acc x body c elem K S o).2.widened = false →
    (∀ fin, evalFold C n dep ρ acc x body c xs a = .ok fin → Mem (reduceRounds A m acc x body c elem K S o).1.1 fin) ∧
    (∀ k l, evalFold C n dep ρ acc x body c xs a = .error (.failed k l) → k.ofEffect = true →
      Wit C (reduceRounds A m acc x body c elem K S o).2)

theorem sound_zero {C : Ctx} {res : List (Option Abs)} : SoundAt C res 0 where
  expr m A ρ e o dep _ _ _ := ⟨fun v h => by simp [eval] at h, fun k l h => by simp [eval] at h⟩
  args m A ρ args o dep _ _ _ := ⟨fun v h => by simp [evalArgs] at h, fun k l h => by simp [evalArgs] at h⟩
  args5 m A ρ args o dep _ _ _ := ⟨fun v h => by simp [evalArgs4] at h, fun k l h => by simp [evalArgs4] at h⟩
  inits m A ρ inits o dep _ _ _ := ⟨fun v h => by simp [evalInits] at h, fun k l h => by simp [evalInits] at h⟩
  list m A ρ args o dep _ _ _ := ⟨fun v h => by simp [evalList] at h, fun k l h => by simp [evalList] at h⟩
  elems m A ρ elems elem k o dep pre _ _ _ _ _ :=
    ⟨fun v h => by simp [evalElems] at h, fun k l h => by simp [evalElems] at h⟩
  each m A ρ x body c w xs elem o dep _ _ _ _ :=
    ⟨fun v h => by simp [evalEach] at h, fun k l h => by simp [evalEach] at h⟩
  pairs m A ρ pairs ks vs k o dep pre key _ _ _ _ _ :=
    ⟨fun v h => by simp [evalPairs] at h, fun k l h => by simp [evalPairs] at h⟩
  each2 m A ρ x kb kc y vb vc xs elem o dep _ _ _ _ :=
    ⟨fun v h => by simp [evalEach2] at h, fun k l h => by simp [evalEach2] at h⟩
  merge m A ρ key ps x y body c va vb qs o dep _ _ _ _ _ :=
    ⟨fun v h => by simp [evalMerge] at h, fun k l h => by simp [evalMerge] at h⟩
  fix m A ρ acc x body c S elem xs a o dep _ _ _ _ _ _ :=
    ⟨fun v h => by simp [evalFold] at h, fun k l h => by simp [evalFold] at h⟩
  rounds m A ρ acc x body c elem K S xs a o dep _ _ _ _ _ _ :=
    ⟨fun v h => by simp [evalFold] at h, fun k l h => by simp [evalFold] at h⟩

-- ── Out of steps ─────────────────────────────────────────────────────────────

theorem wit_fuel {C : Ctx} {o : Out} : Wit C (outOfFuel o) := wit_place (c := .none) (Mem.any _) trivial

theorem concl_fuel {C : Ctx} {ρ : Env} {res : R Val} {A : ACtx} {o : Out} (h : Sat A ρ) :
    Concl C ρ res (AR.of .any (outOfFuel o) A) :=
  ⟨fun v _ => ⟨Mem.any v, h, fun _ _ => trivial⟩, fun _ _ _ _ => wit_fuel⟩

theorem grows_not_widened {o o' : Out} (g : Grows o o') (h : o'.widened = false) : o.widened = false := by
  cases hw : o.widened
  · rfl
  · exact absurd (g.widened hw) (by rw [h]; simp)

end KanameCore
