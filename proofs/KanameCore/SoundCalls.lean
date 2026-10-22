/-
  The calls and what is built (kaname's DESIGN 5.4, 15.2.2, 15.2.3): a function of the module, a
  rule, a date of a dates file, a transfer and a record, each from the values of its arguments.

  Each argument is read in its place's unit (a place for int64 where that can leave it) and kept
  as a place the evaluation holds it to; where the evaluation holds an argument and fails, the
  place has that argument among its values. The names given as arguments keep what their places
  declare once the call is made (`assume`).
-/
import KanameCore.Sound

namespace KanameCore

-- ── Places that hold a value to what it declares ────────────────────────────

theorem outside_none {d : Nat} {s : Slot} {reached : Bool} : s.outsideD d reached .none = false := by
  cases d with
  | zero => rfl
  | succ d => cases s; rfl

theorem outside_inRange {s : Slot} {r : Range} (hr : s.range? = some r) {reached : Bool} {v : Val}
    (h : s.outside reached v = false) : InRange r v := by
  intro p hp
  obtain ⟨list, count, range, unit01, record, fields, values, keys, ref⟩ := s
  simp only [Slot.range?] at hr
  subst hr
  unfold Slot.outside at h
  rw [show D = 999 + 1 from rfl] at h
  have key : pointOut (rangeOf (some r) unit01 reached) v = false := by
    cases v with
    | num q =>
      simp only [Slot.outsideD, keysOut, Bool.and_false, Bool.false_or, elemsOf, List.any_cons, List.any_nil,
        Bool.or_false, Bool.or_eq_false_iff] at h
      exact h.1.1
    | date d =>
      simp only [Slot.outsideD, keysOut, Bool.and_false, Bool.false_or, elemsOf, List.any_cons, List.any_nil,
        Bool.or_false, Bool.or_eq_false_iff] at h
      exact h.1.1
    | _ => simp [Val.point] at hp
  simp only [pointOut, rangeOf, hp] at key
  unfold outRange at key
  simp only [Bool.or_eq_false_iff] at key
  obtain ⟨h2, h3⟩ := key
  constructor
  · rcases hm : r.min with _ | l
    · trivial
    · rw [hm] at h2; simp at h2; exact Rat.not_lt.mp h2
  · rcases hm : r.max with _ | u
    · trivial
    · rw [hm] at h3; simp at h3; exact Rat.not_lt.mp h3

-- ── The arguments of a call ─────────────────────────────────────────────────

theorem AllMem.length : ∀ {as : List Abs} {vs : List Val}, AllMem as vs → as.length = vs.length
  | [], [], _ => rfl
  | _ :: _, _ :: _, h => by simp [AllMem.length h.2]
  | [], _ :: _, h => by simp [AllMem] at h
  | _ :: _, [], h => by simp [AllMem] at h

theorem ArgSyms.length : ∀ {xs : List Expr} {vs : List Val} {ρ : Env}, ArgSyms ρ xs vs → xs.length = vs.length
  | [], [], _, _ => rfl
  | _ :: _, _ :: _, _, h => by simp [ArgSyms.length h.2]
  | [], _ :: _, _, h => by simp [ArgSyms] at h
  | _ :: _, [], _, h => by simp [ArgSyms] at h

/-- **The arguments of a call of a function, where the evaluation fails**: one read in its place's
    unit that leaves int64, or one outside its parameter's range; the analysis kept the place. -/
theorem callFold_wit {C : Ctx} {fn : Fn} :
    ∀ (args : List (Nat × Expr × Option Conv)) (avs : List Abs) (vals : List Val) (o : Out) (A : ACtx),
      AllMem avs vals →
      (∀ given, convArgs args vals = .ok given →
         ∀ k v, (k, v) ∈ given → ∀ p, fn.params[k]? = some p → p.slot.outside true v = true →
            Wit C ((args.zip avs).foldl (callStep fn) (o, A)).1) ∧
      (∀ e, convArgs args vals = .error e →
        (∃ line, e = .failed .overflow line) ∧ Wit C ((args.zip avs).foldl (callStep fn) (o, A)).1)
  | [], avs, vals, o, A, _ => by
    refine ⟨fun given hg => ?_, fun e he => by simp [convArgs] at he⟩
    simp [convArgs] at hg; subst hg
    intro k v h; simp at h
  | (k, x, c) :: rest, a :: as, v :: vs, o, A, hall => by
    obtain ⟨hm, hall'⟩ := hall
    simp only [List.zip_cons_cons, List.foldl_cons]
    have hgrow : ∀ (acc : Out × ACtx), Grows acc.1 ((rest.zip as).foldl (callStep fn) acc).1 := by
      intro acc
      exact (foldl_inv (fun (b : Out × ACtx) => Grows acc.1 b.1 ∧ Same acc.2 b.2) (callStep fn) callStep_ok
        (rest.zip as) acc ⟨Grows.refl _, Same.refl _⟩).1
    have hstep : Grows (convA o c a).2 (callStep fn (o, A) ((k, x, c), a)).1 := by
      simp only [callStep]
      split
      · exact Grows.refl _
      · exact grows_place (Grows.refl _)
    simp only [convArgs, bind, Except.bind]
    rcases hc : coerce c v with e | v'
    · refine ⟨fun given hg => by simp at hg, fun e' he => ?_⟩
      simp at he; subst he
      obtain ⟨⟨c', line, rfl, rfl⟩, hw⟩ := coerce_wit (o := o) hm hc
      exact ⟨⟨_, rfl⟩, Wit.grow (Wit.grow hw hstep) (hgrow _)⟩
    · have hmv' : Mem (convA o c a).1 v' := convA_mem hm hc
      obtain ⟨ih1, ih2⟩ := callFold_wit rest as vs (callStep fn (o, A) ((k, x, c), a)).1
        (callStep fn (o, A) ((k, x, c), a)).2 hall'
      constructor
      · intro given hg
        rcases hmore : convArgs rest vs with e | more <;> rw [hmore] at hg <;> simp [] at hg
        subst hg
        intro k' w hkw p hp hout
        simp only [List.mem_cons, Prod.mk.injEq] at hkw
        rcases hkw with ⟨rfl, rfl⟩ | hkw
        · refine Wit.grow ?_ (hgrow _)
          simp only [callStep, hp]
          exact wit_place hmv' hout
        · exact ih1 more hmore k' w hkw p hp hout
      · intro e he
        rcases hmore : convArgs rest vs with e' | more <;> rw [hmore] at he <;> simp [] at he
        subst he
        exact ih2 e' hmore
  | (k, x, c) :: rest, [], vals, o, A, hall => by
    cases vals with
    | nil => simp only [convArgs]; exact ⟨fun g hg => by simp at hg; subst hg; intro k v h; simp at h, fun e he => by simp at he⟩
    | cons _ _ => simp [AllMem] at hall
  | (k, x, c) :: rest, a :: as, [], o, A, hall => by simp [AllMem] at hall

/-- **The arguments of a call of a function, where none fails**: the names given keep their
    parameters' ranges. -/
theorem callFold_sat {fn : Fn} {ρ : Env} :
    ∀ (args : List (Nat × Expr × Option Conv)) (avs : List Abs) (vals : List Val) (o : Out) (A : ACtx),
      AllMem avs vals → ArgSyms ρ (args.map (·.2.1)) vals → Sat A ρ →
      ∀ given, convArgs args vals = .ok given →
        (∀ k v, (k, v) ∈ given → ∀ p, fn.params[k]? = some p → p.slot.outside true v = false) →
        Sat ((args.zip avs).foldl (callStep fn) (o, A)).2 ρ
  | [], avs, vals, o, A, _, _, hS, given, _, _ => hS
  | (k, x, c) :: rest, a :: as, v :: vs, o, A, hall, hsyms, hS, given, hg, hout => by
    obtain ⟨hm, hall'⟩ := hall
    obtain ⟨hsym, hsyms'⟩ := hsyms
    simp only [List.zip_cons_cons, List.foldl_cons]
    simp only [convArgs, bind, Except.bind] at hg
    rcases hc : coerce c v with e | v' <;> rw [hc] at hg <;> simp at hg
    rcases hmore : convArgs rest vs with e | more <;> rw [hmore] at hg <;> simp [] at hg
    subst hg
    have hS' : Sat (callStep fn (o, A) ((k, x, c), a)).2 ρ := by
      simp only [callStep]
      split
      · exact hS
      · rename_i prm hprm
        split
        · split
          · rename_i r hr
            refine hS.assume ?_
            intro hcn s hs w hw
            subst hcn
            simp [coerce] at hc; subst hc
            rw [hsym s hs] at hw; cases hw
            exact outside_inRange hr (hout k v (by simp) prm hprm)
          · exact hS
        · exact hS
    exact callFold_sat rest as vs _ _ hall' hsyms' hS' more hmore
      (fun k' w hkw p hp => hout k' w (by simp [hkw]) p hp)
  | (k, x, c) :: rest, [], vals, o, A, hall, hsyms, hS, given, hg, hout => by
    cases vals with
    | nil => simp [ArgSyms] at hsyms
    | cons _ _ => simp [AllMem] at hall
  | (k, x, c) :: rest, a :: as, [], o, A, hall, _, _, _, _, _ => by simp [AllMem] at hall


-- ── A call of a function of the module ──────────────────────────────────────

theorem mem_zip_place {α : Type} {ps : List α} {given : List (Nat × Val)} {p : α} {v : Val}
    (h : (p, v) ∈ ps.zip (place ps.length given)) :
    v = .none ∨ ∃ k, ps[k]? = some p ∧ (k, v) ∈ given := by
  obtain ⟨i, hi⟩ := List.mem_iff_getElem?.mp h
  rw [List.getElem?_zip_eq_some] at hi
  obtain ⟨h1, h2⟩ := hi
  simp only [place, List.getElem?_map] at h2
  have hlt : i < ps.length := by
    rcases Nat.lt_or_ge i ps.length with hlt | hge
    · exact hlt
    · rw [List.getElem?_eq_none hge] at h1; simp at h1
  rw [List.getElem?_range hlt] at h2
  simp only [Option.map_some, Option.some.injEq] at h2
  rcases hf : given.find? (·.1 == i) with _ | ⟨k, w⟩ <;> rw [hf] at h2 <;> simp at h2
  · exact Or.inl h2.symm
  · subst h2
    have hk := List.find?_some hf
    simp at hk; subst hk
    exact Or.inr ⟨k, h1, List.mem_of_find?_eq_some hf⟩

theorem valid_place {fn : Fn} {given : List (Nat × Val)}
    (hout : ∀ k v, (k, v) ∈ given → ∀ p, fn.params[k]? = some p → p.slot.outside true v = false)
    (hhas : (fn.params.zip (place fn.params.length given)).all (fun (p, v) => p.abs.has v) = true) :
    Valid fn (place fn.params.length given) := by
  refine ⟨by simp [place], ?_⟩
  intro p v hpv
  refine ⟨has_sound' (by have := List.all_eq_true.mp hhas (p, v) hpv; simpa using this), ?_⟩
  rcases mem_zip_place hpv with rfl | ⟨k, hk, hkv⟩
  · exact outside_none
  · exact hout k v hkv p hk

theorem callFn_declared {C : Ctx} {n dep f : Nat} {args : List Val} {v : Val} {fn : Fn}
    (hfn : C.M.fns[f]? = some fn) (h : callFn C n dep f args = .ok v) : Mem fn.declared v := by
  cases n with
  | zero => simp [callFn] at h
  | succ n =>
    simp only [callFn, hfn] at h
    split at h
    · simp at h
    · obtain ⟨y, _, h⟩ := bind_ok h
      obtain ⟨z, _, h⟩ := bind_ok h
      split at h
      · simp [failK] at h
      · split at h
        · rename_i hhas; cases h; exact has_sound' hhas
        · simp [failK] at h

/-- **A call of a function of the module.** -/
theorem call_case {C : Ctx} {res : List (Option Abs)} (hR : ResultsOK C res) {A : ACtx} {o : Out} {ρ : Env}
    {f line n dep : Nat} {fn0 : Fn} {args : List (Nat × Expr × Option Conv)} {avs : List Abs} {vals : List Val}
    (hfn0 : C.M.fns[f]? = some fn0)
    (hG : Good C res A) (hS : Sat A ρ) (hall : AllMem avs vals) (hsyms : ArgSyms ρ (args.map (·.2.1)) vals) :
    (∀ v, (convArgs args vals >>= fun given => callArgs C.M line f given >>= fun ps => callFn C n dep f ps) = .ok v →
      Mem (callAbs A o f args avs).1 v ∧ Sat (callAbs A o f args avs).2.2 ρ) ∧
    (∀ k l, (convArgs args vals >>= fun given => callArgs C.M line f given >>= fun ps => callFn C n dep f ps) =
      .error (.failed k l) → k.ofEffect = true → Wit C (callAbs A o f args avs).2.1) := by
  have hfn : A.M.fns[f]? = some fn0 := by rw [hG.1]; exact hfn0
  simp only [callAbs, hfn]
  have hgc : Grows ((args.zip avs).foldl (callStep fn0) (o, A)).1
      { ((args.zip avs).foldl (callStep fn0) (o, A)).1 with
        calls := ((args.zip avs).foldl (callStep fn0) (o, A)).1.calls ++ [f] } :=
    grows_calls (Grows.refl _)
  obtain ⟨hw1, hw2⟩ := callFold_wit (C := C) (fn := fn0) args avs vals o A hall
  constructor
  · intro v hv
    obtain ⟨given, hg, hv⟩ := bind_ok hv
    obtain ⟨ps, hps, hv⟩ := bind_ok hv
    simp only [callArgs, hfn0] at hps
    split at hps
    · simp [failK] at hps
    · rename_i hnout
      split at hps
      · rename_i hhas
        cases hps
        have hnout' : ∀ k w, (k, w) ∈ given → paramOutside fn0 k w = false := by
          simpa using hnout
        have hout : ∀ k v, (k, v) ∈ given → ∀ p, fn0.params[k]? = some p → p.slot.outside true v = false := by
          intro k w hkw p hp
          have := hnout' k w hkw
          simp only [paramOutside, hp] at this
          exact this
        refine ⟨?_, callFold_sat args avs vals o A hall hsyms hS given hg hout⟩
        split
        · exact callFn_declared hfn0 hv
        · rename_i hne
          have htf : fn0.takesFn = false := by
            cases h : fn0.takesFn
            · rfl
            · simp [h] at hne
          have hsame := (foldl_inv (fun (b : Out × ACtx) => Grows o b.1 ∧ Same A b.2) (callStep fn0) callStep_ok
            (args.zip avs) (o, A) ⟨Grows.refl _, Same.refl _⟩).2
          rw [hsame.2.1]
          rcases hres : (A.results[f]?).join with _ | a
          · simp only [Option.getD_none]; exact callFn_declared hfn0 hv
          · have : res[f]? = some (some a) := by
              rw [← hG.2]; simpa [Option.join_eq_some_iff] using hres
            simp only [Option.getD_some]
            exact hR f a this fn0 hfn0 htf _ _ _ _ (valid_place hout hhas) hv
      · simp [failK] at hps
  · intro k l hv hk
    rcases bind_err hv with hv | ⟨given, hg, hv⟩
    · exact Wit.grow (hw2 _ hv).2 hgc
    · rcases bind_err hv with hv | ⟨ps, hps, hv⟩
      · simp only [callArgs, hfn0] at hv
        split at hv
        · rename_i hany
          have := failK_err hv; subst this
          obtain ⟨⟨k', w⟩, hkw, hp⟩ := List.any_eq_true.mp hany
          simp only [paramOutside] at hp
          split at hp
          · rename_i p hpk
            exact Wit.grow (hw1 given hg k' w hkw p hpk hp) hgc
          · simp at hp
        · split at hv
          · simp at hv
          · have := failK_err hv; subst this; simp [Kind.ofEffect] at hk
      · -- the function called fails: it fails with values its parameters take
        simp only [callArgs, hfn0] at hps
        split at hps
        · simp [failK] at hps
        · rename_i hnout
          split at hps
          · rename_i hhas
            cases hps
            have hnout' : ∀ k w, (k, w) ∈ given → paramOutside fn0 k w = false := by
              simpa using hnout
            have hout : ∀ k v, (k, v) ∈ given → ∀ p, fn0.params[k]? = some p → p.slot.outside true v = false := by
              intro k w hkw p hp
              have := hnout' k w hkw
              simp only [paramOutside, hp] at this
              exact this
            refine Or.inr ⟨f, List.mem_append_right _ (List.mem_singleton_self f), ?_⟩
            intro hnf
            have := hnf fn0 hfn0 n dep _ (valid_place hout hhas) k l hv
            rw [this] at hk; exact absurd hk (by simp)
          · simp [failK] at hps


-- ── The arguments of a call, step by step ───────────────────────────────────

/-- Each value given to a call came from one of its arguments, read in its place's unit; the step of
    the analysis on that argument is kept to the end. -/
theorem fold_given {β : Type} (step : Out × β → (Nat × Expr × Option Conv) × Abs → Out × β)
    (hstep : ∀ acc arg, Grows acc.1 (step acc arg).1) :
    ∀ (args : List (Nat × Expr × Option Conv)) (avs : List Abs) (vals : List Val) (acc : Out × β),
      AllMem avs vals →
      (∀ given, convArgs args vals = .ok given → ∀ k v', (k, v') ∈ given →
        ∃ (x : Expr) (c : Option Conv) (a : Abs) (v : Val) (acc' : Out × β), Mem a v ∧ coerce c v = .ok v' ∧
          Grows (step acc' ((k, x, c), a)).1 ((args.zip avs).foldl step acc).1) ∧
      (∀ e, convArgs args vals = .error e →
        ∃ (x : Expr) (c : Option Conv) (a : Abs) (v : Val) (k : Nat) (acc' : Out × β), Mem a v ∧ coerce c v = .error e ∧
          Grows (step acc' ((k, x, c), a)).1 ((args.zip avs).foldl step acc).1)
  | [], _, _, acc, _ => by
    refine ⟨fun given hg => ?_, fun e he => by simp [convArgs] at he⟩
    simp [convArgs] at hg; subst hg; intro k v h; simp at h
  | (k, x, c) :: rest, a :: as, v :: vs, acc, hall => by
    obtain ⟨hm, hall'⟩ := hall
    simp only [List.zip_cons_cons, List.foldl_cons]
    have hgrow : ∀ (acc : Out × β), Grows acc.1 ((rest.zip as).foldl step acc).1 := by
      intro acc
      exact foldl_inv (fun (b : Out × β) => Grows acc.1 b.1) step (fun b arg h => h.trans (hstep b arg))
        (rest.zip as) acc (Grows.refl _)
    obtain ⟨ih1, ih2⟩ := fold_given step hstep rest as vs (step acc ((k, x, c), a)) hall'
    simp only [convArgs, bind, Except.bind]
    rcases hc : coerce c v with e | v'
    · refine ⟨fun given hg => by simp at hg, fun e' he => ?_⟩
      simp at he; subst he
      exact ⟨x, c, a, v, k, acc, hm, hc, hgrow _⟩
    · constructor
      · intro given hg
        rcases hmore : convArgs rest vs with e | more <;> rw [hmore] at hg <;> simp at hg
        subst hg
        intro k' w hkw
        simp only [List.mem_cons, Prod.mk.injEq] at hkw
        rcases hkw with ⟨rfl, rfl⟩ | hkw
        · exact ⟨x, c, a, v, acc, hm, hc, hgrow _⟩
        · exact ih1 more hmore k' w hkw
      · intro e he
        rcases hmore : convArgs rest vs with e' | more <;> rw [hmore] at he <;> simp at he
        subst he
        exact ih2 e' hmore
  | (k, x, c) :: rest, [], vals, acc, hall => by
    cases vals with
    | nil => simp only [convArgs]; exact ⟨fun g hg => by simp at hg; subst hg; intro k v h; simp at h, fun e he => by simp at he⟩
    | cons _ _ => simp [AllMem] at hall
  | (k, x, c) :: rest, a :: as, [], acc, hall => by simp [AllMem] at hall

theorem convArgs_err {args : List (Nat × Expr × Option Conv)} {vals : List Val} {e : Stop}
    (h : convArgs args vals = .error e) : ∃ line, e = .failed .overflow line := by
  induction args generalizing vals with
  | nil => simp [convArgs] at h
  | cons a rest ih =>
    obtain ⟨k, x, c⟩ := a
    cases vals with
    | nil => simp [convArgs] at h
    | cons v vs =>
      simp only [convArgs, bind, Except.bind] at h
      rcases hc : coerce c v with e' | v' <;> rw [hc] at h
      · simp at h; subst h
        cases c with
        | none => simp [coerce] at hc
        | some c => exact ⟨_, (coerceV_error c v e' hc)⟩
      · simp only at h
        rcases hmore : convArgs rest vs with e'' | more <;> rw [hmore] at h <;> simp at h
        subst h; exact ih hmore

-- ── A call of a rule ─────────────────────────────────────────────────────────

theorem ruleStep_grows {rd : RuleD} : ∀ acc arg, Grows acc.1 (ruleStep rd acc arg).1 :=
  fun acc arg => (ruleStep_ok (o₀ := acc.1) (A₀ := acc.2) acc arg ⟨Grows.refl _, Same.refl _⟩).1

theorem elemStep_grows {x : Expr} {inp : RuleInput} {elem : Abs} {o : Out} {col : String × Option Range} :
    Grows o (elemStep x inp elem o col) := by
  simp only [elemStep]; split
  · exact grows_place (Grows.refl _)
  · exact Grows.refl _

theorem elemFold_grows {x : Expr} {inp : RuleInput} {elem : Abs} :
    ∀ (cols : List (String × Option Range)) (o : Out), Grows o (cols.foldl (elemStep x inp elem) o)
  | [], o => Grows.refl o
  | _ :: rest, _ => elemStep_grows.trans (elemFold_grows rest _)

theorem ruleStep_conv {rd : RuleD} {acc : Out × ACtx} {k : Nat} {x : Expr} {c : Option Conv} {a : Abs} :
    Grows (convA acc.1 c a).2 (ruleStep rd acc ((k, x, c), a)).1 := by
  obtain ⟨o, A⟩ := acc
  simp only [ruleStep]
  split
  · exact Grows.refl _
  · split
    · split
      · exact grows_place (Grows.refl _)
      · split
        · exact grows_place (Grows.refl _)
        · exact grows_enumPlace (Grows.refl _)
    · exact elemFold_grows _ _

/-- A value of an enum rulec refuses for the range of its input: one of the enum, not among the
    values the range lets in, given to an input of an enum held to a range. -/
theorem RuleInput.enumOut_true {i : RuleInput} {v : Val} (h : i.enumOut v = true) :
    ∃ x vs, v = .enum x ∧ i.range = none ∧ i.dateRanged = false ∧ i.values = some vs ∧ vs.contains x = false := by
  cases v with
  | enum x =>
    simp only [RuleInput.enumOut] at h
    rcases hr : i.range with _ | r <;> rcases hd : i.dateRanged <;> rcases hv : i.values with _ | vs <;>
      simp only [hr, hd, hv, Bool.false_eq_true] at h
    exact ⟨x, vs, rfl, rfl, rfl, rfl, by simpa using h⟩
  | _ => simp [RuleInput.enumOut] at h

/-- The places of a rule's input for a value given to it, and what fails there. -/
theorem ruleStep_wit {C : Ctx} {rd : RuleD} {acc : Out × ACtx} {k : Nat} {x : Expr} {c : Option Conv} {a : Abs}
    {v' : Val} {i : RuleInput} (hk : rd.inputs[k]? = some i) (hm : Mem (convA acc.1 c a).1 v') :
    (i.refuses v' = true → Wit C (ruleStep rd acc ((k, x, c), a)).1) ∧
    (i.elements = none → i.range = none → i.dateRanged = true → v' ≠ .none → Wit C (ruleStep rd acc ((k, x, c), a)).1) := by
  obtain ⟨o, A⟩ := acc
  simp only [ruleStep, hk]
  constructor
  · intro hr
    unfold RuleInput.refuses at hr
    split at hr
    · rename_i hel
      simp only [hel]
      simp only [Bool.or_eq_true] at hr
      rcases hr with hr | hr
      · split at hr
        · rename_i rg q hrg hq
          simp only [hrg]
          exact wit_place hm ⟨q, hq, hr⟩
        · simp at hr
      · -- a value of an enum the range of its input does not let in
        obtain ⟨y, vs, rfl, hrg, hdr, hvs, hout⟩ := RuleInput.enumOut_true hr
        simp only [hrg, hdr, Bool.false_eq_true, ite_false, enumPlace, hvs]
        exact wit_place hm ⟨y, rfl, hout⟩
    · rename_i fs hel
      simp only [hel]
      split at hr
      · rename_i els
        obtain ⟨el, hel', hr⟩ := List.any_eq_true.mp hr
        split at hr
        · rename_i vs
          obtain ⟨col, hcol, hr⟩ := List.any_eq_true.mp hr
          unfold colRefuses at hr
          split at hr
          · rename_i rr q hrr hq
            have hmel : Mem (elemOfList (convA o c a).1) (.record vs) := by
              unfold elemOfList
              split
              · rename_i cnt e he; rw [he] at hm
                cases hm with
                | list _ hxs => exact hxs _ hel'
              · exact Mem.any _
            have hfield : Mem ((elemOfList (convA o c a).1).field col.1) (.num q) :=
              Mem_field hmel (by simpa [Val.field?] using hq)
            have : ∀ (cols : List (String × Option Range)) (o' : Out), col ∈ cols →
                Wit C (cols.foldl (elemStep x i (elemOfList (convA o c a).1)) o') := by
              intro cols
              induction cols with
              | nil => intro o' h; simp at h
              | cons col' rest ih =>
                intro o' h
                simp only [List.foldl_cons]
                rcases List.mem_cons.mp h with rfl | h
                · have hw : Wit C (elemStep x i (elemOfList (convA o c a).1) o' col) := by
                    simp only [elemStep, hrr]
                    exact wit_place hfield ⟨q, rfl, hr⟩
                  exact Wit.grow hw (elemFold_grows _ _)
                · exact ih _ h
            exact this fs _ hcol
          · simp at hr
        · simp at hr
      · simp at hr
  · intro hel hrange hdr hne
    simp only [hel, hrange, hdr, ite_true]
    exact wit_place hm trivial


theorem inRange_of_not_out {r : Range} {v : Val} (h : ∀ p, v.point = some p → outRange r p = false) : InRange r v := by
  intro p hp
  have h1 := h p hp
  unfold outRange at h1
  simp only [Bool.or_eq_false_iff] at h1
  obtain ⟨h2, h3⟩ := h1
  constructor
  · rcases hm : r.min with _ | l
    · trivial
    · rw [hm] at h2; simp at h2; exact Rat.not_lt.mp h2
  · rcases hm : r.max with _ | u
    · trivial
    · rw [hm] at h3; simp at h3; exact Rat.not_lt.mp h3

theorem refuses_inRange {i : RuleInput} {rg : Range} {v : Val} (he : i.elements = none) (hr : i.range = some rg)
    (h : i.refuses v = false) : InRange rg v := by
  refine inRange_of_not_out fun p hp => ?_
  unfold RuleInput.refuses at h
  simp only [he, hr, hp, Bool.or_eq_false_iff] at h
  exact h.1

theorem ruleFold_sat {rd : RuleD} {ρ : Env} :
    ∀ (args : List (Nat × Expr × Option Conv)) (avs : List Abs) (vals : List Val) (o : Out) (A : ACtx),
      AllMem avs vals → ArgSyms ρ (args.map (·.2.1)) vals → Sat A ρ →
      ∀ given, convArgs args vals = .ok given → (∀ k v, (k, v) ∈ given → inputRefuses rd k v = false) →
      Sat ((args.zip avs).foldl (ruleStep rd) (o, A)).2 ρ
  | [], avs, vals, o, A, _, _, hS, given, _, _ => hS
  | (k, x, c) :: rest, a :: as, v :: vs, o, A, hall, hsyms, hS, given, hg, hno => by
    obtain ⟨hm, hall'⟩ := hall
    obtain ⟨hsym, hsyms'⟩ := hsyms
    simp only [List.zip_cons_cons, List.foldl_cons]
    simp only [convArgs, bind, Except.bind] at hg
    rcases hc : coerce c v with e | v' <;> rw [hc] at hg <;> simp at hg
    rcases hmore : convArgs rest vs with e | more <;> rw [hmore] at hg <;> simp at hg
    subst hg
    have hS' : Sat (ruleStep rd (o, A) ((k, x, c), a)).2 ρ := by
      simp only [ruleStep]
      split
      · exact hS
      · rename_i inp hinp
        split
        · rename_i hel
          split
          · rename_i rg hrg
            split
            · exact hS
            · refine hS.assume ?_
              intro hcn s hs w hw
              subst hcn
              simp [coerce] at hc; subst hc
              rw [hsym s hs] at hw; cases hw
              have := hno k v (by simp)
              simp only [inputRefuses, hinp] at this
              exact refuses_inRange hel hrg this
          · split <;> exact hS
        · exact hS
    exact ruleFold_sat rest as vs _ _ hall' hsyms' hS' more hmore (fun k' w hkw => hno k' w (by simp [hkw]))
  | (k, x, c) :: rest, [], vals, o, A, hall, hsyms, hS, given, hg, hout => by
    cases vals with
    | nil => simp [ArgSyms] at hsyms
    | cons _ _ => simp [AllMem] at hall
  | (k, x, c) :: rest, a :: as, [], o, A, hall, _, _, _, _, _ => by simp [AllMem] at hall

theorem preFold_grows {id : Nat} : ∀ (pres : List String) (o : Out),
    Grows o (pres.foldl (fun o said => o.place id ("pre:" ++ said) .rule .pending .any) o)
  | [], o => Grows.refl o
  | p :: rest, o => (grows_place (Grows.refl o)).trans (preFold_grows rest _)

theorem preFold_wit {C : Ctx} {id : Nat} : ∀ (pres : List String) (o : Out), pres ≠ [] →
    Wit C (pres.foldl (fun o said => o.place id ("pre:" ++ said) .rule .pending .any) o)
  | [], o, h => absurd rfl h
  | p :: rest, o, _ => by
    simp only [List.foldl_cons]
    exact Wit.grow (wit_place (what := .pending) (c := .none) (Mem.any _) trivial) (preFold_grows rest _)

/-- **A call of a rule.** -/
theorem rule_case {C : Ctx} {res : List (Option Abs)} (hO : OracleOK C) {A : ACtx} {o : Out} {ρ : Env}
    {id line r : Nat} {rd0 : RuleD} {args : List (Nat × Expr × Option Conv)} {avs : List Abs} {vals : List Val}
    (hrd0 : C.M.rules[r]? = some rd0) (hG : Good C res A) (hS : Sat A ρ) (hall : AllMem avs vals)
    (hsyms : ArgSyms ρ (args.map (·.2.1)) vals) :
    (∀ v, (convArgs args vals >>= fun given => ruleAnswer C line r given) = .ok v →
      Mem (ruleAbs A o id r args avs).1 v ∧ Sat (ruleAbs A o id r args avs).2.2 ρ) ∧
    (∀ k l, (convArgs args vals >>= fun given => ruleAnswer C line r given) = .error (.failed k l) →
      k.ofEffect = true → Wit C (ruleAbs A o id r args avs).2.1) := by
  have hrd : A.M.rules[r]? = some rd0 := by rw [hG.1]; exact hrd0
  simp only [ruleAbs, hrd]
  obtain ⟨hg1, hg2⟩ := fold_given (ruleStep rd0) ruleStep_grows args avs vals (o, A) hall
  have hpre := grows_keepIf (preFold_grows (id := id) rd0.pres ((args.zip avs).foldl (ruleStep rd0) (o, A)).1)
    ((args.zip avs).foldl (ruleStep rd0) (o, A)).2.lam id
    (callRels ((args.zip avs).foldl (ruleStep rd0) (o, A)).2 rd0 args)
  constructor
  · intro v hv
    obtain ⟨given, hg, hv⟩ := bind_ok hv
    simp only [ruleAnswer, hrd0] at hv
    split at hv
    · simp [failK] at hv
    · split at hv
      · simp [failK] at hv
      · rename_i hnref
        split at hv
        · simp [failK] at hv
        · rename_i outs hout
          split at hv
          · simp [failK] at hv
          · rename_i hlen
            split at hv
            · simp [failK] at hv
            · cases hv
              have hl : outs.length = rd0.outputs.length := by simpa using hlen
              refine ⟨(hO.rule r rd0 _ outs hrd0 hout hl).1, ?_⟩
              refine ruleFold_sat args avs vals o A hall hsyms hS given hg ?_
              intro k w hkw
              have : given.any (fun (k, v) => inputRefuses rd0 k v) = false := by simpa using hnref
              have := List.any_eq_false.mp this (k, w) hkw
              simpa using this
  · intro k l hv hk
    rcases bind_err hv with hv | ⟨given, hg, hv⟩
    · obtain ⟨x, c, a, w, k', acc', hm, hc, hgr⟩ := hg2 _ hv
      obtain ⟨_, hw⟩ := coerce_wit (o := acc'.1) hm hc
      exact Wit.grow (Wit.grow hw ruleStep_conv) (hgr.trans hpre)
    · simp only [ruleAnswer, hrd0] at hv
      split at hv
      · have := failK_err hv; subst this; simp [Kind.ofEffect] at hk
      · split at hv
        · -- rulec refuses a value given
          rename_i href
          obtain ⟨⟨k', w⟩, hkw, hr⟩ := List.any_eq_true.mp href
          simp only [inputRefuses] at hr
          split at hr
          · rename_i i hi
            obtain ⟨x, c, a, w0, acc', hm, hc, hgr⟩ := hg1 given hg k' w hkw
            exact Wit.grow ((ruleStep_wit hi (convA_mem hm hc)).1 hr) (hgr.trans hpre)
          · simp at hr
        · rename_i hnref
          split at hv
          · -- rulec does not answer: a precondition, or a date held to a range of days, was given
            rename_i hnone
            by_cases hp : rd0.pres = []
            · by_cases hd : ∃ i v, (i, v) ∈ rd0.inputs.zip (place rd0.inputs.length given) ∧
                  i.elements = none ∧ i.range = none ∧ i.dateRanged = true ∧ v ≠ .none
              · -- a date held to a range of days was given
                obtain ⟨i, v, hiv, he, hr, hdr, hne⟩ := hd
                rcases mem_zip_place hiv with rfl | ⟨k', hk', hkv⟩
                · exact absurd rfl hne
                · obtain ⟨x, c, a, w0, acc', hm, hc, hgr⟩ := hg1 given hg k' v hkv
                  exact Wit.grow ((ruleStep_wit hk' (convA_mem hm hc)).2 he hr hdr hne) (hgr.trans hpre)
              · exfalso
                have hd' : ∀ i v, (i, v) ∈ rd0.inputs.zip (place rd0.inputs.length given) →
                    i.elements = none → i.range = none → i.dateRanged = true → v = .none :=
                  fun i v hiv he hr hdr => Classical.byContradiction fun hne => hd ⟨i, v, hiv, he, hr, hdr, hne⟩
                have hnr : ∀ i v, (i, v) ∈ rd0.inputs.zip (place rd0.inputs.length given) → i.refuses v = false := by
                  intro i v hiv
                  rcases mem_zip_place hiv with rfl | ⟨k', hk', hkv⟩
                  · unfold RuleInput.refuses
                    split
                    · simp [Val.point, RuleInput.enumOut]
                    · simp
                  · have : given.any (fun (k, v) => inputRefuses rd0 k v) = false := by simpa using hnref
                    have := List.any_eq_false.mp this (k', v) hkv
                    simp only [inputRefuses, hk'] at this
                    simpa using this
                obtain ⟨outs, ho⟩ := hO.ruleAnswers r rd0 _ hrd0 hp (by simp [place]) hd' hnr
                rw [ho] at hnone; simp at hnone
            · exact Wit.grow (preFold_wit rd0.pres _ hp) (grows_keepIf (Grows.refl _) _ _ _)
          · rename_i outs hout
            split at hv
            · have := failK_err hv; subst this; simp [Kind.ofEffect] at hk
            · rename_i hlen
              split at hv
              · -- a number of the rule's answer outside int64: what rulec shows does not happen
                rename_i hfit
                exfalso
                have hl : outs.length = rd0.outputs.length := by simpa using hlen
                have hf := (hO.rule r rd0 _ outs hrd0 hout hl).2
                obtain ⟨x, hx, hnf⟩ := List.any_eq_true.mp hfit
                split at hnf
                · rename_i q
                  rw [hf _ hx q rfl] at hnf; simp at hnf
                · simp at hnf
              · simp at hv


-- ── A date of a dates file ───────────────────────────────────────────────────

theorem mapM_length {α β : Type} {f : α → Option β} : ∀ {l : List α} {r : List β}, l.mapM f = some r →
    r.length = l.length
  | [], r, h => by simp [List.mapM_nil, pure] at h; subst h; rfl
  | a :: l, r, h => by
    rw [List.mapM_cons] at h
    rcases ha : f a with _ | b <;> rw [ha] at h <;> simp [bind] at h
    rcases hl : l.mapM f with _ | bs <;> rw [hl] at h <;> simp [] at h
    subst h
    simp [mapM_length hl]

theorem mapM_some {α β : Type} {f : α → Option β} : ∀ {l : List α} {r : List β}, l.mapM f = some r →
    (∀ p ∈ l.zip r, f p.1 = some p.2) ∧ (∀ a ∈ l, ∃ b ∈ r, f a = some b) ∧ (∀ b ∈ r, ∃ a ∈ l, f a = some b)
  | [], r, h => by simp [List.mapM_nil, pure] at h; subst h; simp
  | a :: l, r, h => by
    rw [List.mapM_cons] at h
    rcases ha : f a with _ | b <;> rw [ha] at h <;> simp [bind] at h
    rcases hl : l.mapM f with _ | bs <;> rw [hl] at h <;> simp [] at h
    subst h
    obtain ⟨h1, h2, h3⟩ := mapM_some hl
    refine ⟨?_, ?_, ?_⟩
    · intro p hp
      simp only [List.zip_cons_cons, List.mem_cons] at hp
      rcases hp with rfl | hp
      · exact ha
      · exact h1 p hp
    · intro x hx
      rcases List.mem_cons.mp hx with rfl | hx
      · exact ⟨b, by simp, ha⟩
      · obtain ⟨y, hy, hfy⟩ := h2 x hx; exact ⟨y, by simp [hy], hfy⟩
    · intro y hy
      rcases List.mem_cons.mp hy with rfl | hy
      · exact ⟨a, by simp, ha⟩
      · obtain ⟨x, hx, hfx⟩ := h3 y hy; exact ⟨x, by simp [hx], hfx⟩

theorem intOf_point {v : Val} {z : Int} (h : intOf v = some z) : v.point = some (z : Rat) := by
  unfold intOf at h
  split at h
  · cases h; rfl
  · rename_i x
    split at h
    · rename_i hd; cases h; simp only [Val.point]; rw [← den_one hd]
    · simp at h
  · simp at h

theorem intOf_in {a : Abs} {v : Val} {z : Int} {gi : Iv} (hm : Mem a v) (hi : a.iv = some gi) (h : intOf v = some z) :
    gi.In (z : Rat) := point_in hm (intOf_point h) hi

theorem dateStep_grows {fd : DatesD} {reads : List String} : ∀ acc arg, Grows acc.1 (dateStep fd reads acc arg).1 :=
  fun acc arg => (dateStep_ok (o₀ := acc.1) (A₀ := acc.2.1) acc arg ⟨Grows.refl _, Same.refl _⟩).1

theorem dateStep_conv {fd : DatesD} {reads : List String} {acc : Out × ACtx × Option Abs} {k : Nat} {x : Expr}
    {c : Option Conv} {a : Abs} : Grows (convA acc.1 c a).2 (dateStep fd reads acc ((k, x, c), a)).1 := by
  obtain ⟨o, A, g⟩ := acc
  simp only [dateStep]
  split
  · exact Grows.refl _
  · split
    · exact grows_place (Grows.refl _)
    · exact grows_place (Grows.refl _)

theorem dateStep_wit {C : Ctx} {fd : DatesD} {reads : List String} {acc : Out × ACtx × Option Abs} {k : Nat}
    {x : Expr} {c : Option Conv} {a : Abs} {v' : Val} {inp : DateInput} (hk : fd.inputs[k]? = some inp)
    (hm : Mem (convA acc.1 c a).1 v') :
    (inp.isDate = true → Wit C (dateStep fd reads acc ((k, x, c), a)).1) ∧
    (inp.isDate = false → ∀ p, v'.point = some p → outRange ⟨some (inp.min : Rat), some (inp.max : Rat)⟩ p = true →
      Wit C (dateStep fd reads acc ((k, x, c), a)).1) := by
  obtain ⟨o, A, g⟩ := acc
  simp only [dateStep, hk]
  constructor
  · intro hd; simp only [hd, ite_true]; exact wit_place hm trivial
  · intro hd p hp hout; simp only [hd, Bool.false_eq_true, ite_false]; exact wit_place hm ⟨p, hp, hout⟩

/-- The date the analysis counts the days from came from a value given to a date input it reads. -/
theorem dateFold_given {fd : DatesD} {reads : List String} :
    ∀ (args : List (Nat × Expr × Option Conv)) (avs : List Abs) (vals : List Val) (acc : Out × ACtx × Option Abs),
      AllMem avs vals → ∀ given, convArgs args vals = .ok given → ∀ g, ((args.zip avs).foldl (dateStep fd reads) acc).2.2 = some g →
        acc.2.2 = some g ∨ ∃ k inp v', fd.inputs[k]? = some inp ∧ inp.isDate = true ∧ reads.contains inp.name = true ∧
          (k, v') ∈ given ∧ Mem g v'
  | [], _, _, acc, _, given, _, g, hg => Or.inl hg
  | (k, x, c) :: rest, a :: as, v :: vs, acc, hall, given, hconv, g, hg => by
    obtain ⟨hm, hall'⟩ := hall
    simp only [List.zip_cons_cons, List.foldl_cons] at hg
    simp only [convArgs, bind, Except.bind] at hconv
    rcases hc : coerce c v with e | v' <;> rw [hc] at hconv <;> simp at hconv
    rcases hmore : convArgs rest vs with e | more <;> rw [hmore] at hconv <;> simp at hconv
    subst hconv
    rcases dateFold_given rest as vs _ hall' more hmore g hg with h | ⟨k', inp, w, h1, h2, h3, h4, h5⟩
    · -- the date after this argument
      obtain ⟨o, A, g0⟩ := acc
      simp only [dateStep] at h
      split at h
      · exact Or.inl h
      · rename_i inp hinp
        split at h
        · rename_i hd
          split at h
          · rename_i hr
            cases h
            exact Or.inr ⟨k, inp, v', hinp, hd, hr, by simp, convA_mem hm hc⟩
          · exact Or.inl h
        · exact Or.inl h
    · exact Or.inr ⟨k', inp, w, h1, h2, h3, by simp [h4], h5⟩
  | (k, x, c) :: rest, [], vals, acc, hall, given, hconv, g, hg => by
    cases vals with
    | nil => exact Or.inl hg
    | cons _ _ => simp [AllMem] at hall
  | (k, x, c) :: rest, a :: as, [], acc, hall, _, _, _, _ => by simp [AllMem] at hall

theorem dateFold_sat {fd : DatesD} {reads : List String} {ρ : Env} :
    ∀ (args : List (Nat × Expr × Option Conv)) (avs : List Abs) (vals : List Val) (acc : Out × ACtx × Option Abs),
      AllMem avs vals → ArgSyms ρ (args.map (·.2.1)) vals → Sat acc.2.1 ρ →
      ∀ given, convArgs args vals = .ok given →
        (∀ k v inp, (k, v) ∈ given → fd.inputs[k]? = some inp → ∃ z, intOf v = some z ∧ inp.min ≤ z ∧ z ≤ inp.max) →
        Sat ((args.zip avs).foldl (dateStep fd reads) acc).2.1 ρ
  | [], _, _, acc, _, _, hS, _, _, _ => hS
  | (k, x, c) :: rest, a :: as, v :: vs, acc, hall, hsyms, hS, given, hg, hin => by
    obtain ⟨hm, hall'⟩ := hall
    obtain ⟨hsym, hsyms'⟩ := hsyms
    simp only [List.zip_cons_cons, List.foldl_cons]
    simp only [convArgs, bind, Except.bind] at hg
    rcases hc : coerce c v with e | v' <;> rw [hc] at hg <;> simp at hg
    rcases hmore : convArgs rest vs with e | more <;> rw [hmore] at hg <;> simp at hg
    subst hg
    have hS' : Sat (dateStep fd reads acc ((k, x, c), a)).2.1 ρ := by
      obtain ⟨o, A, g0⟩ := acc
      simp only [dateStep]
      split
      · exact hS
      · rename_i inp hinp
        have hr : c = none → ∀ s, sym x = some s → ∀ w, symVal ρ s = some w →
            InRange ⟨some (inp.min : Rat), some (inp.max : Rat)⟩ w := by
          intro hcn s hs w hw
          subst hcn
          simp [coerce] at hc; subst hc
          rw [hsym s hs] at hw; cases hw
          obtain ⟨z, hz, h1, h2⟩ := hin k v inp (by simp) hinp
          intro p hp
          rw [intOf_point hz] at hp; cases hp
          exact ⟨Rat.intCast_le_intCast.mpr h1, Rat.intCast_le_intCast.mpr h2⟩
        split
        · exact hS.assume hr
        · exact hS.assume hr
    exact dateFold_sat rest as vs _ hall' hsyms' hS' more hmore
      (fun k' w inp hkw hinp => hin k' w inp (by simp [hkw]) hinp)
  | (k, x, c) :: rest, [], vals, acc, hall, hsyms, _, _, _, _ => by
    cases vals with
    | nil => simp [ArgSyms] at hsyms
    | cons _ _ => simp [AllMem] at hall
  | (k, x, c) :: rest, a :: as, [], acc, hall, _, _, _, _, _ => by simp [AllMem] at hall


theorem mem_hullOf {dd : Option DateD} {d : Int}
    (hhull : ∀ dd0, dd = some dd0 → ∀ lo hi, dd0.hull = some (lo, hi) → lo ≤ d ∧ d ≤ hi) :
    (hullOf dd).In (d : Rat) := by
  unfold hullOf
  split
  · rename_i lo hi hb
    rcases hdd : dd with _ | dd0 <;> rw [hdd] at hb <;> simp at hb
    obtain ⟨h1, h2⟩ := hhull dd0 hdd lo hi hb
    exact In_ints h1 h2
  · exact ⟨trivial, trivial, by simp [DenLe, Rat.den_intCast]⟩

theorem mem_dateIv {dd : Option DateD} {g : Option Abs} {d : Int}
    (hhull : ∀ dd0, dd = some dd0 → ∀ lo hi, dd0.hull = some (lo, hi) → lo ≤ d ∧ d ≤ hi)
    (hdays : ∀ dd0 g0 gi fewest most, dd = some dd0 → g = some g0 → g0.iv = some gi → dd0.days = some (fewest, most) →
      ∃ z : Int, gi.In (z : Rat) ∧ fewest ≤ d - z ∧ d - z ≤ most) :
    (dateIv dd g).In (d : Rat) := by
  have hh := mem_hullOf hhull
  unfold dateIv
  split
  · rename_i g0 fewest most hb
    split
    · rename_i gi hgi
      rcases hdd : dd with _ | dd0 <;> rw [hdd] at hb <;> simp at hb
      obtain ⟨z, hz, h1, h2⟩ := hdays dd0 g0 gi fewest most hdd rfl hgi hb
      have hp : (gi.plus (Iv.ints fewest most)).In (d : Rat) := by
        have := In_plus hz (In_ints h1 h2)
        rwa [show ((z : Rat) + ((d - z : Int) : Rat)) = (d : Rat) by push_cast; grind] at this
      obtain ⟨m, hm, hin⟩ := In_meet hh hp
      rw [← hdd, hm]; exact hin
    · exact hh
  · exact hh

theorem mem_zip_of_mem {α β : Type} : ∀ {l : List α} {r : List β} {a : α}, r.length = l.length → a ∈ l →
    ∃ b, (a, b) ∈ l.zip r
  | [], _, _, _, h => by simp at h
  | x :: l, [], _, hl, _ => by simp at hl
  | x :: l, y :: r, a, hl, h => by
    rcases List.mem_cons.mp h with rfl | h
    · exact ⟨y, by simp⟩
    · obtain ⟨b, hb⟩ := mem_zip_of_mem (r := r) (by simpa using hl) h
      exact ⟨b, by simp [hb]⟩

/-- **A date of a dates file.** -/
theorem date_case {C : Ctx} {res : List (Option Abs)} (hO : OracleOK C) {A : ACtx} {o : Out} {ρ : Env}
    {line file : Nat} {date : String} {fd0 : DatesD} {args : List (Nat × Expr × Option Conv)} {avs : List Abs}
    {vals : List Val} (hfd0 : C.M.dates[file]? = some fd0) (hG : Good C res A) (hS : Sat A ρ)
    (hall : AllMem avs vals) (hsyms : ArgSyms ρ (args.map (·.2.1)) vals) :
    (∀ v, (convArgs args vals >>= fun given => dateAnswer C line file date given) = .ok v →
      Mem (dateAbs A o file date args avs).1 v ∧ Sat (dateAbs A o file date args avs).2.2 ρ) ∧
    (∀ k l, (convArgs args vals >>= fun given => dateAnswer C line file date given) = .error (.failed k l) →
      k.ofEffect = true → Wit C (dateAbs A o file date args avs).2.1) := by
  have hfd : A.M.dates[file]? = some fd0 := by rw [hG.1]; exact hfd0
  simp only [dateAbs, hfd]
  generalize hreads : ((fd0.dates.find? (·.name == date)).map (·.reads)).getD [] = reads
  obtain ⟨hg1, hg2⟩ := fold_given (dateStep fd0 reads) dateStep_grows args avs vals (o, A, none) hall
  constructor
  · intro v hv
    obtain ⟨given, hg, hv⟩ := bind_ok hv
    simp only [dateAnswer, hfd0] at hv
    split at hv
    · simp [failK] at hv
    · rename_i ints hints
      obtain ⟨hz1, hz2, hz3⟩ := mapM_some hints
      split at hv
      · simp [failK] at hv
      · rename_i hnout
        split at hv
        · rename_i d hd
          cases hv
          have hor := hO.date file fd0 date ints d hfd0 hd
          -- every value given to an input is a whole number in its range
          have hin : ∀ k v inp, (k, v) ∈ given → fd0.inputs[k]? = some inp →
              ∃ z, intOf v = some z ∧ inp.min ≤ z ∧ z ≤ inp.max := by
            intro k w inp hkw hinp
            have hn : (inp, w) ∈ given.filterMap fun (k, v) => (fd0.inputs[k]?).map fun i => (i, v) :=
              List.mem_filterMap.mpr ⟨(k, w), hkw, by simp [hinp]⟩
            obtain ⟨b, hb, hfb⟩ := hz2 _ hn
            rcases hiw : intOf w with _ | z <;> rw [hiw] at hfb <;> simp at hfb
            subst hfb
            refine ⟨z, rfl, ?_⟩
            have hzip : ((inp, w), (inp.name, z)) ∈ (given.filterMap fun (k, v) => (fd0.inputs[k]?).map fun i => (i, v)).zip ints := by
              obtain ⟨b', hb'⟩ := mem_zip_of_mem (mapM_length hints) hn
              have := hz1 _ hb'
              simp only [hiw, Option.map_some, Option.some.injEq] at this
              subst this; exact hb'
            have hnout' := by simpa using hnout
            exact hnout' inp w inp.name z hzip
          refine ⟨?_, dateFold_sat args avs vals (o, A, none) hall hsyms hS given hg hin⟩
          refine Mem.date (mem_dateIv ?_ ?_)
          · intro dd0 hdd lo hi hh
            exact (hor dd0 hdd).1 lo hi hh
          · intro dd0 g0 gi fewest most hdd hg0 hgi hdays
            rcases dateFold_given args avs vals (o, A, none) hall given hg g0 hg0 with h | ⟨k, inp, w, hk, hdate, hr, hkw, hmw⟩
            · simp at h
            · have hn : (inp, w) ∈ given.filterMap fun (k, v) => (fd0.inputs[k]?).map fun i => (i, v) :=
                List.mem_filterMap.mpr ⟨(k, w), hkw, by simp [hk]⟩
              obtain ⟨b, hb, hfb⟩ := hz2 _ hn
              rcases hiw : intOf w with _ | z <;> rw [hiw] at hfb <;> simp at hfb
              subst hfb
              have hr' : dd0.reads.contains inp.name = true := by
                rw [← hreads, hdd] at hr; simpa using hr
              have := (hor dd0 hdd).2 fewest most hdays inp z (List.mem_of_getElem? hk) hdate hr' hb
              exact ⟨z, intOf_in hmw hgi hiw, this⟩
        · simp [failK] at hv
  · intro k l hv hk
    rcases bind_err hv with hv | ⟨given, hg, hv⟩
    · obtain ⟨x, c, a, w, k', acc', hm, hc, hgr⟩ := hg2 _ hv
      obtain ⟨_, hw⟩ := coerce_wit (o := acc'.1) hm hc
      exact Wit.grow (Wit.grow hw dateStep_conv) hgr
    · simp only [dateAnswer, hfd0] at hv
      split at hv
      · have := failK_err hv; subst this; simp [Kind.ofEffect] at hk
      · rename_i ints hints
        obtain ⟨hz1, hz2, hz3⟩ := mapM_some hints
        -- a value given came from an argument, to an input of the file
        have hfrom : ∀ inp w, (inp, w) ∈ given.filterMap (fun (k, v) => (fd0.inputs[k]?).map fun i => (i, v)) →
            ∃ k', (k', w) ∈ given ∧ fd0.inputs[k']? = some inp := by
          intro inp w hn
          obtain ⟨⟨k', w'⟩, hkw, hfm⟩ := List.mem_filterMap.mp hn
          simp only at hfm
          rcases hik : fd0.inputs[k']? with _ | i <;> rw [hik] at hfm <;> simp at hfm
          obtain ⟨rfl, rfl⟩ := hfm
          exact ⟨k', hkw, hik⟩
        split at hv
        · -- a whole number outside its input's range
          rename_i hany
          obtain ⟨⟨⟨inp, w⟩, nm, z0⟩, hmem, hout⟩ := List.any_eq_true.mp hany
          have hfz := hz1 _ hmem
          rcases hiw : intOf w with _ | z <;> rw [hiw] at hfz <;> simp at hfz
          obtain ⟨rfl, rfl⟩ := hfz
          obtain ⟨k', hkw, hk'⟩ := hfrom inp w (List.of_mem_zip hmem).1
          obtain ⟨x, c, a, w0, acc', hm, hc, hgr⟩ := hg1 given hg k' w hkw
          cases hd : inp.isDate
          · have hout' : outRange ⟨some (inp.min : Rat), some (inp.max : Rat)⟩ (z : Rat) = true := by
              simp only [Bool.or_eq_true, decide_eq_true_eq] at hout
              unfold outRange
              simp only [Option.any_some, Bool.or_eq_true, decide_eq_true_eq]
              rcases hout with h | h
              · exact Or.inl (Rat.intCast_lt_intCast.mpr h)
              · exact Or.inr (Rat.intCast_lt_intCast.mpr h)
            exact Wit.grow ((dateStep_wit hk' (convA_mem hm hc)).2 hd z (intOf_point hiw) hout') hgr
          · exact Wit.grow ((dateStep_wit hk' (convA_mem hm hc)).1 hd) hgr
        · rename_i hnout
          split at hv
          · simp at hv
          · -- koyomi gives no day: a date was given
            rename_i hnone
            by_cases hall' : ∀ nm g, (nm, g) ∈ ints →
                ∃ inp ∈ fd0.inputs, inp.name = nm ∧ inp.isDate = false ∧ inp.min ≤ g ∧ g ≤ inp.max
            · exfalso
              obtain ⟨d, hd⟩ := hO.dateAnswers file fd0 date ints hfd0 hall'
              rw [hd] at hnone; simp at hnone
            · have : ∃ nm g, (nm, g) ∈ ints ∧
                  ¬ ∃ inp ∈ fd0.inputs, inp.name = nm ∧ inp.isDate = false ∧ inp.min ≤ g ∧ g ≤ inp.max :=
                Classical.byContradiction fun h => hall' fun nm g hng =>
                  Classical.byContradiction fun h' => h ⟨nm, g, hng, h'⟩
              obtain ⟨nm, g0, hng, hno⟩ := this
              obtain ⟨⟨inp, w⟩, hn, hfb⟩ := hz3 _ hng
              rcases hiw : intOf w with _ | g <;> rw [hiw] at hfb <;> simp at hfb
              obtain ⟨rfl, rfl⟩ := hfb
              obtain ⟨k', hkw, hk'⟩ := hfrom inp w hn
              obtain ⟨x, c, a, w0, acc', hm, hc, hgr⟩ := hg1 given hg k' w hkw
              cases hd : inp.isDate
              · exfalso
                -- the value is in its input's range, as the check of the range found
                obtain ⟨b', hb'⟩ := mem_zip_of_mem (mapM_length hints) hn
                have := hz1 _ hb'
                simp only [hiw, Option.map_some, Option.some.injEq] at this
                subst this
                have hnout' := by simpa using hnout
                obtain ⟨h1, h2⟩ := hnout' inp w inp.name g hb'
                exact hno ⟨inp, List.mem_of_getElem? hk', rfl, hd, h1, h2⟩
              · exact Wit.grow ((dateStep_wit hk' (convA_mem hm hc)).1 hd) hgr


-- ── A transfer ───────────────────────────────────────────────────────────────

/-- The values of what a transfer's analysis kept, field by field, each holding the value given. -/
def Align : List (String × Abs) → List (String × Val) → Prop
  | [], [] => True
  | (p, a) :: as, (q, v) :: vs => p = q ∧ Mem a v ∧ Align as vs
  | _, _ => False

theorem Align.append : ∀ {as : List (String × Abs)} {vs : List (String × Val)} {bs : List (String × Abs)}
    {ws : List (String × Val)}, Align as vs → Align bs ws → Align (as ++ bs) (vs ++ ws)
  | [], [], _, _, _, h => h
  | (p, a) :: as, (q, v) :: vs, _, _, h, h' => ⟨h.1, h.2.1, Align.append h.2.2 h'⟩
  | [], _ :: _, _, _, h, _ => by simp [Align] at h
  | _ :: _, [], _, _, h, _ => by simp [Align] at h

theorem Align.find : ∀ {as : List (String × Abs)} {vs : List (String × Val)}, Align as vs → ∀ {n : String} {a : Abs} {x : Val},
    Abs.fieldOf as n = some a → (vs.find? (·.1 == n)).map (·.2) = some x → Mem a x
  | [], [], _, _, _, _, h, _ => by simp [Abs.fieldOf] at h
  | (p, a) :: as, (q, v) :: vs, h, n, b, x, hb, hx => by
    obtain ⟨rfl, hm, h'⟩ := h
    by_cases hp : p = n
    · subst hp
      simp [Abs.fieldOf] at hb; simp at hx
      subst hb; subst hx; exact hm
    · have h1 : (p == n) = false := by simp [hp]
      simp only [Abs.fieldOf, List.find?, h1] at hb hx
      exact Align.find h' hb hx
  | [], _ :: _, h, _, _, _, _, _ => by simp [Align] at h
  | _ :: _, [], h, _, _, _, _, _ => by simp [Align] at h

theorem fieldV_transfer {given : List (String × Val)} {n : String} {x : Val} :
    ∀ (params : List String), fieldV (params.filterMap fun p => (given.find? (·.1 == p)).map fun (_, v) => (p, v)) n = some x →
      (given.find? (·.1 == n)).map (·.2) = some x
  | [], h => by simp [fieldV] at h
  | p :: ps, h => by
    simp only [List.filterMap_cons] at h
    rcases hf : given.find? (·.1 == p) with _ | ⟨q, v⟩ <;> rw [hf] at h
    · exact fieldV_transfer ps h
    · simp only [Option.map_some] at h
      unfold fieldV at h
      by_cases hp : p = n
      · subst hp
        simp at h; subst h
        rw [hf]; rfl
      · have h1 : (p == n) = false := by simp [hp]
        simp only [List.find?, h1] at h
        exact fieldV_transfer ps h

theorem transferStep_grows : ∀ (acc : List (String × Abs) × Out) arg, Grows acc.2 (transferStep acc arg).2 := by
  rintro ⟨out, o⟩ ⟨⟨p, x, c, l, amount⟩, a⟩
  simp only [transferStep]
  split
  · exact grows_place (grows_convA (Grows.refl _))
  · exact grows_convA (Grows.refl _)

/-- **The arguments of a transfer.** -/
theorem transferFold {C : Ctx} :
    ∀ (args : List (String × Expr × Option Conv × Nat × Bool)) (avs : List Abs) (vals : List Val)
      (out : List (String × Abs)) (o : Out) (outV : List (String × Val)), AllMem avs vals → Align out outV →
      (∀ given, convTArgs args vals = .ok given →
        Align ((args.zip avs).foldl transferStep (out, o)).1 (outV ++ given)) ∧
      (∀ k l, convTArgs args vals = .error (.failed k l) → k.ofEffect = true →
        Wit C ((args.zip avs).foldl transferStep (out, o)).2)
  | [], _, _, out, o, outV, _, hal => by
    refine ⟨fun given hg => ?_, fun k l h => by simp [convTArgs] at h⟩
    simp [convTArgs] at hg; subst hg; simpa using hal
  | (p, x, c, line, amount) :: rest, a :: as, v :: vs, out, o, outV, hall, hal => by
    obtain ⟨hm, hall'⟩ := hall
    simp only [List.zip_cons_cons, List.foldl_cons]
    have hgrow : ∀ (acc : List (String × Abs) × Out), Grows acc.2 ((rest.zip as).foldl transferStep acc).2 := by
      intro acc
      exact foldl_inv (fun (b : List (String × Abs) × Out) => Grows acc.2 b.2) transferStep
        (fun b arg h => h.trans (transferStep_grows b arg)) (rest.zip as) acc (Grows.refl _)
    simp only [convTArgs, bind, Except.bind]
    rcases hc : coerce c v with e | v'
    · refine ⟨fun given hg => by simp at hg, fun k l he hk => ?_⟩
      simp at he; subst he
      obtain ⟨_, hw⟩ := coerce_wit (o := o) hm hc
      refine Wit.grow ?_ (hgrow _)
      simp only [transferStep]
      split
      · exact Wit.grow hw (grows_place (Grows.refl _))
      · exact hw
    · have hmv := convA_mem (o := o) hm hc
      have hal' : Align (out ++ [(p, (convA o c a).1)]) (outV ++ [(p, v')]) :=
        Align.append hal ⟨rfl, hmv, trivial⟩
      obtain ⟨ih1, ih2⟩ := transferFold rest as vs (transferStep (out, o) ((p, x, c, line, amount), a)).1
        (transferStep (out, o) ((p, x, c, line, amount), a)).2 (outV ++ [(p, v')]) hall' (by
          simp only [transferStep]; exact hal')
      simp only
      split
      · rename_i hneg
        refine ⟨fun given hg => by simp [failK] at hg, fun k l he hk => ?_⟩
        refine Wit.grow ?_ (hgrow _)
        simp only [Bool.and_eq_true] at hneg
        simp only [transferStep, hneg.1, ite_true]
        exact wit_place hmv trivial
      · constructor
        · intro given hg
          rcases hmore : convTArgs rest vs with e | more <;> rw [hmore] at hg <;> simp at hg
          subst hg
          have := ih1 more hmore
          simpa using this
        · intro k l he hk
          rcases hmore : convTArgs rest vs with e | more <;> rw [hmore] at he <;> simp at he
          subst he
          exact ih2 k l hmore hk
  | (p, x, c, line, amount) :: rest, [], vals, out, o, outV, hall, hal => by
    cases vals with
    | nil =>
      simp only [convTArgs]
      exact ⟨fun g hg => by simp at hg; subst hg; simpa using hal, fun k l h => by simp at h⟩
    | cons _ _ => simp [AllMem] at hall
  | (p, x, c, line, amount) :: rest, a :: as, [], out, o, outV, hall, _ => by simp [AllMem] at hall

/-- **A transfer built.** -/
theorem transfer_case {C : Ctx} {o : Out} {tid : String} {params : List String}
    {args : List (String × Expr × Option Conv × Nat × Bool)} {avs : List Abs} {vals : List Val} (hall : AllMem avs vals) :
    (∀ v, (convTArgs args vals >>= fun given => Except.ok (transferVal tid params given)) = .ok v →
      Mem (transferAbs o args avs).1 v) ∧
    (∀ k l, (convTArgs args vals >>= fun given => Except.ok (transferVal tid params given)) = .error (.failed k l) →
      k.ofEffect = true → Wit C (transferAbs o args avs).2) := by
  obtain ⟨h1, h2⟩ := transferFold (C := C) args avs vals [] o [] hall trivial
  simp only [transferAbs]
  constructor
  · intro v hv
    obtain ⟨given, hg, hv⟩ := bind_ok hv
    cases hv
    have hal := h1 given hg
    simp only [List.nil_append] at hal
    refine Mem.transfer ?_
    intro n x hx b hb
    exact hal.find hb (fieldV_transfer params hx)
  · intro k l hv hk
    rcases bind_err hv with hv | ⟨given, _, hv⟩
    · exact h2 k l hv hk
    · simp at hv


-- ── A record ─────────────────────────────────────────────────────────────────

/-- The fields of a record as the analysis read them and as the evaluation did, written for the same
    fields in the same order, each value among what its field's analysis says (and, where it is read
    in its own unit, the value of the name it is). -/
def RecAlign (ρ : Env) : List (Nat × Abs × Expr × Option Conv) → List (Nat × Val × Nat) → Prop
  | [], [] => True
  | (k, a, x, c) :: as, (k', v, _) :: vs =>
    k = k' ∧ Mem a v ∧ (c = none → ∀ s, sym x = some s → symVal ρ s = some v) ∧ RecAlign ρ as vs
  | _, _ => False

theorem RecAlign.append {ρ : Env} : ∀ {as : List (Nat × Abs × Expr × Option Conv)} {vs : List (Nat × Val × Nat)}
    {bs : List (Nat × Abs × Expr × Option Conv)} {ws : List (Nat × Val × Nat)},
    RecAlign ρ as vs → RecAlign ρ bs ws → RecAlign ρ (as ++ bs) (vs ++ ws)
  | [], [], _, _, _, h => h
  | (k, a, x, c) :: as, (k', v, l) :: vs, _, _, h, h' => ⟨h.1, h.2.1, h.2.2.1, RecAlign.append h.2.2.2 h'⟩
  | [], _ :: _, _, _, h, _ => by simp [RecAlign] at h
  | _ :: _, [], _, _, h, _ => by simp [RecAlign] at h

theorem RecAlign.reverse {ρ : Env} : ∀ {as : List (Nat × Abs × Expr × Option Conv)} {vs : List (Nat × Val × Nat)},
    RecAlign ρ as vs → RecAlign ρ as.reverse vs.reverse
  | [], [], _ => trivial
  | (k, a, x, c) :: as, (k', v, l) :: vs, h => by
    simp only [List.reverse_cons]
    exact RecAlign.append (RecAlign.reverse h.2.2.2) ⟨h.1, h.2.1, h.2.2.1, trivial⟩
  | [], _ :: _, h => by simp [RecAlign] at h
  | _ :: _, [], h => by simp [RecAlign] at h

/-- The value written for a field, found in both. -/
theorem RecAlign.find {ρ : Env} {k : Nat} : ∀ {as : List (Nat × Abs × Expr × Option Conv)} {vs : List (Nat × Val × Nat)},
    RecAlign ρ as vs →
      (as.find? (·.1 == k) = none ∧ vs.find? (·.1 == k) = none) ∨
      ∃ a x c v l, as.find? (·.1 == k) = some (k, a, x, c) ∧ vs.find? (·.1 == k) = some (k, v, l) ∧ Mem a v ∧
        (c = none → ∀ s, sym x = some s → symVal ρ s = some v)
  | [], [], _ => Or.inl ⟨rfl, rfl⟩
  | (k1, a, x, c) :: as, (k2, v, l) :: vs, h => by
    obtain ⟨rfl, hm, hs, h'⟩ := h
    by_cases hk : k1 = k
    · subst hk
      exact Or.inr ⟨a, x, c, v, l, by simp, by simp, hm, hs⟩
    · have h1 : (k1 == k) = false := by simp [hk]
      simp only [List.find?, h1]
      exact RecAlign.find h'
  | [], _ :: _, h => by simp [RecAlign] at h
  | _ :: _, [], h => by simp [RecAlign] at h

/-- The fields as written, read in their places' units. -/
theorem initFold {C : Ctx} {ρ : Env} :
    ∀ (inits : List (Nat × Expr × Option Conv × Nat)) (avs : List Abs) (vals : List Val)
      (g0 : List (Nat × Abs × Expr × Option Conv)) (o : Out) (gv0 : List (Nat × Val × Nat)),
      AllMem avs vals → ArgSyms ρ (inits.map (·.2.1)) vals → RecAlign ρ g0 gv0 →
      Grows o ((inits.zip avs).foldl initStep (g0, o)).2 ∧
      (∀ given, convInits inits vals = .ok given → RecAlign ρ ((inits.zip avs).foldl initStep (g0, o)).1 (gv0 ++ given)) ∧
      (∀ e, convInits inits vals = .error e →
        (∃ line, e = .failed .overflow line) ∧ Wit C ((inits.zip avs).foldl initStep (g0, o)).2)
  | [], _, _, g0, o, gv0, _, _, hal => by
    refine ⟨Grows.refl o, fun given hg => ?_, fun e he => by simp [convInits] at he⟩
    simp [convInits] at hg; subst hg; simpa using hal
  | (k, x, c, line) :: rest, a :: as, v :: vs, g0, o, gv0, hall, hsyms, hal => by
    obtain ⟨hm, hall'⟩ := hall
    obtain ⟨hsym, hsyms'⟩ := hsyms
    simp only [List.zip_cons_cons, List.foldl_cons, initStep]
    have hgrow : ∀ (acc : List (Nat × Abs × Expr × Option Conv) × Out), Grows acc.2 ((rest.zip as).foldl initStep acc).2 := by
      intro acc
      exact foldl_inv (fun (b : List (Nat × Abs × Expr × Option Conv) × Out) => Grows acc.2 b.2) initStep
        (fun b arg h => by
          obtain ⟨g, o'⟩ := b; obtain ⟨⟨k, x, c, l⟩, a⟩ := arg
          simp only [initStep]; exact grows_convA h) (rest.zip as) acc (Grows.refl _)
    simp only [convInits, bind, Except.bind]
    rcases hc : coerce c v with e | v'
    · have hg' := hgrow (g0 ++ [(k, (convA o c a).1, x, c)], (convA o c a).2)
      refine ⟨(grows_convA (c := c) (a := a) (Grows.refl o)).trans hg', fun given hg => by simp at hg, fun e' he => ?_⟩
      simp at he; subst he
      obtain ⟨⟨c', l', rfl, rfl⟩, hw⟩ := coerce_wit (o := o) hm hc
      exact ⟨⟨_, rfl⟩, Wit.grow hw hg'⟩
    · have hal' : RecAlign ρ (g0 ++ [(k, (convA o c a).1, x, c)]) (gv0 ++ [(k, v', line)]) :=
        RecAlign.append hal ⟨rfl, convA_mem hm hc, fun hcn s hs => by
          subst hcn; simp [coerce] at hc; subst hc; exact hsym s hs, trivial⟩
      obtain ⟨ih0, ih1, ih2⟩ := initFold rest as vs (g0 ++ [(k, (convA o c a).1, x, c)]) (convA o c a).2
        (gv0 ++ [(k, v', line)]) hall' hsyms' hal'
      refine ⟨(grows_convA (Grows.refl o)).trans ih0, ?_, ?_⟩
      · intro given hg
        rcases hmore : convInits rest vs with e | more <;> rw [hmore] at hg <;> simp at hg
        subst hg
        have := ih1 more hmore
        simpa using this
      · intro e he
        rcases hmore : convInits rest vs with e' | more <;> rw [hmore] at he <;> simp at he
        subst he
        exact ih2 e' hmore
  | (k, x, c, line) :: rest, [], vals, g0, o, gv0, hall, hsyms, hal => by
    cases vals with
    | nil => simp [ArgSyms] at hsyms
    | cons _ _ => simp [AllMem] at hall
  | (k, x, c, line) :: rest, a :: as, [], g0, o, gv0, hall, _, _ => by simp [AllMem] at hall

theorem outside_noRange {s : Slot} (h1 : s.range? = none) (h2 : s.count? = none) {v : Val} : s.outside false v = false := by
  obtain ⟨list, count, range, unit01, record, fields, values, keys, ref⟩ := s
  simp only [Slot.range?, Slot.count?] at h1 h2
  subst h1 h2
  unfold Slot.outside
  rw [show D = 999 + 1 from rfl]
  simp only [Slot.outsideD, Bool.false_and, Bool.false_or]
  split
  · rfl
  · have he : ∃ xs, elemsOf list none v = some xs := by
      unfold elemsOf; split <;> exact ⟨_, rfl⟩
    obtain ⟨xs, hxs⟩ := he
    rw [hxs]
    simp only
    rw [List.any_eq_false]
    intro x _
    split
    · simp
    · simp [pointOut, rangeOf, strOut]
      cases x <;> simp

theorem fold_step {α β : Type} (out : β → Out) (f : β → α → β) (hf : ∀ b a, Grows (out b) (out (f b a))) :
    ∀ (L : List α) (b : β) (a : α), a ∈ L → ∃ b', Grows (out (f b' a)) (out (L.foldl f b))
  | [], _, _, h => by simp at h
  | x :: L, b, a, h => by
    simp only [List.foldl_cons]
    have hg : ∀ b', Grows (out b') (out (L.foldl f b')) := fun b' =>
      foldl_inv (fun b'' => Grows (out b') (out b'')) f (fun c a h => h.trans (hf c a)) L b' (Grows.refl _)
    rcases List.mem_cons.mp h with rfl | h
    · exact ⟨b, hg _⟩
    · exact fold_step out f hf L (f b x) a h

theorem fieldFold_out {module : Bool} {given : List (Nat × Abs × Expr × Option Conv)} {src : Option (Abs × Nat)} :
    ∀ (L : List ((String × Slot × Bool) × Nat)) (acc : List (String × Abs) × Out × ACtx),
      (L.foldl (fieldStep module given src) acc).1 =
        acc.1 ++ L.map (fun e => (e.1.1, fieldAbs module e.1.2.1 given src e.1.1 e.2))
  | [], acc => by simp
  | e :: L, acc => by
    simp only [List.foldl_cons, List.map_cons]
    rw [fieldFold_out L]
    obtain ⟨out, o, A⟩ := acc
    obtain ⟨⟨f, slot, sc⟩, k⟩ := e
    simp only [fieldStep, List.append_assoc, List.singleton_append]

theorem fieldStep_grows {module : Bool} {given : List (Nat × Abs × Expr × Option Conv)} {src : Option (Abs × Nat)} :
    ∀ (acc : List (String × Abs) × Out × ACtx) fk, Grows acc.2.1 (fieldStep module given src acc fk).2.1 :=
  fun acc fk => (fieldStep_ok (o₀ := acc.2.1) (A₀ := acc.2.2) acc fk ⟨Grows.refl _, Same.refl _⟩).1

/-- The first field of a name in two lists made from the same fields. -/
theorem find_map_pair {α : Type} {L : List α} {key : α → String} {g1 : α → Abs} {g2 : α → Val} {n : String}
    {b : Abs} {x : Val} (hb : Abs.fieldOf (L.map fun e => (key e, g1 e)) n = some b)
    (hx : fieldV (L.map fun e => (key e, g2 e)) n = some x) : ∃ e ∈ L, b = g1 e ∧ x = g2 e := by
  induction L with
  | nil => simp [Abs.fieldOf] at hb
  | cons e L ih =>
    by_cases hk : key e = n
    · simp [Abs.fieldOf, hk] at hb
      simp [fieldV, hk] at hx
      exact ⟨e, by simp, hb.symm, hx.symm⟩
    · have h1 : (key e == n) = false := by simp [hk]
      simp only [Abs.fieldOf, fieldV, List.map_cons, List.find?, h1] at hb hx
      obtain ⟨e', he', h2, h3⟩ := ih hb hx
      exact ⟨e', by simp [he'], h2, h3⟩

theorem Mem_cutTo {a : Abs} {v : Val} {r : Option Range} {module : Bool} (hm : Mem a v)
    (hr : ∀ r', r = some r' → module = true → InRange r' v) : Mem (cutTo a r module) v := by
  cases a <;> cases r <;> cases module <;> simp only [cutTo] <;> try exact hm
  · rename_i i r'
    cases hm with
    | @num _ q hq =>
      obtain ⟨b1, b2⟩ := hr r' rfl rfl q rfl
      exact Mem.num (In_cut hq b1 b2)
  · rename_i i r'
    cases hm with
    | @date _ q hq =>
      obtain ⟨b1, b2⟩ := hr r' rfl rfl q rfl
      exact Mem.date (In_cut hq b1 b2)

/-- A field of a name in one of two lists made from the same fields, where it is in the other. -/
theorem fieldOf_map_isSome {α : Type} {L : List α} {key : α → String} {g1 : α → Abs} {g2 : α → Val} {n : String}
    {x : Val} (hx : fieldV (L.map fun e => (key e, g2 e)) n = some x) :
    (Abs.fieldOf (L.map fun e => (key e, g1 e)) n).isSome = true := by
  induction L with
  | nil => simp [fieldV] at hx
  | cons e L ih =>
    by_cases hk : key e = n
    · simp [Abs.fieldOf, hk]
    · have h1 : (key e == n) = false := by simp [hk]
      simp only [Abs.fieldOf, fieldV, List.map_cons, List.find?, h1] at hx ⊢
      exact ih hx

/-- What the value a record is made from gives: where the record is made from one (`..x`), the
    evaluation's value of it is among what the analysis says it is. -/
def SrcOK : Option (Abs × Nat) → Option Val → Prop
  | some (fa, _), some w => Mem fa w
  | none, none => True
  | _, _ => False

theorem find_none_reverse {α : Type} {l : List α} {p : α → Bool} (h : l.reverse.find? p = none) : l.find? p = none := by
  rw [List.find?_eq_none] at h ⊢
  intro x hx; exact h x (List.mem_reverse.mpr hx)

/-- **A record built**, or a value of an enum, from the values of its fields as written and the
    value it is made from. -/
theorem record_case {C : Ctx} {A : ACtx} {o : Out} {ρ : Env} {line : Nat} {module : Bool}
    {fields : List (String × Slot × Bool)} {inits : List (Nat × Expr × Option Conv × Nat)} {avs : List Abs}
    {vals : List Val} {tag : Option String} {src : Option (Abs × Nat)} {srcV : Option Val}
    (hS : Sat A ρ) (hall : AllMem avs vals) (hsyms : ArgSyms ρ (inits.map (·.2.1)) vals) (hsrc : SrcOK src srcV) :
    (∀ v, (convInits inits vals >>= fun given => recordVal line module fields given srcV tag) = .ok v →
      Mem (recordAbs A o module fields inits avs tag src).1 v ∧ Sat (recordAbs A o module fields inits avs tag src).2.2 ρ) ∧
    (∀ k l, (convInits inits vals >>= fun given => recordVal line module fields given srcV tag) = .error (.failed k l) →
      k.ofEffect = true → Wit C (recordAbs A o module fields inits avs tag src).2.1) := by
  simp only [recordAbs]
  obtain ⟨hgi, hgi1, hgi2⟩ := initFold (C := C) inits avs vals [] o [] hall hsyms trivial
  generalize ((inits.zip avs).foldl initStep ([], o)) = GI at hgi hgi1 hgi2 ⊢
  obtain ⟨gA, oI⟩ := GI
  simp only at hgi hgi1 hgi2 ⊢
  -- the fields in the order declared
  generalize hL : fields.zipIdx = L
  have hF : fields = L.map Prod.fst := by rw [← hL, List.zipIdx_map_fst]
  -- the fold over the fields: what it keeps, and the places it makes
  have hgf : ∀ (acc : List (String × Abs) × Out × ACtx), Grows acc.2.1 (L.foldl (fieldStep module gA src) acc).2.1 :=
    fun acc => foldl_inv (fun b => Grows acc.2.1 b.2.1) (fieldStep module gA src)
      (fun b a h => h.trans (fieldStep_grows b a)) L acc (Grows.refl _)
  have hout := fieldFold_out (module := module) (given := gA) (src := src) L ([], oI, A)
  simp only [List.nil_append] at hout
  -- the value of each field, as the evaluation and the analysis read it
  have hval : ∀ given, RecAlign ρ gA given → ∀ k,
      ((gA.reverse.find? (·.1 == k)) = none ∧ (given.reverse.find? (·.1 == k)) = none) ∨
      ∃ a x c v l, gA.reverse.find? (·.1 == k) = some (k, a, x, c) ∧ given.reverse.find? (·.1 == k) = some (k, v, l) ∧
        Mem a v ∧ (c = none → ∀ s, sym x = some s → symVal ρ s = some v) := by
    intro given hal k
    exact (RecAlign.reverse hal).find
  -- a field not written: the field of the value it is made from, where there is one
  have hbase : ∀ given, baseMissing fields given srcV = false → ∀ e ∈ L, given.reverse.find? (·.1 == e.2) = none →
      (src = none ∧ srcV = none) ∨
        ∃ fa b w x, src = some (fa, b) ∧ srcV = some w ∧ w.field? e.1.1 = some x ∧ Mem (fa.field e.1.1) x := by
    intro given hbm e he hnw
    cases src with
    | none =>
      cases srcV with
      | none => exact Or.inl ⟨rfl, rfl⟩
      | some _ => exact absurd hsrc (by simp [SrcOK])
    | some fb =>
      obtain ⟨fa, b⟩ := fb
      cases srcV with
      | none => exact absurd hsrc (by simp [SrcOK])
      | some w =>
        simp only [SrcOK] at hsrc
        have hnw' := find_none_reverse hnw
        unfold baseMissing at hbm
        rw [hL] at hbm
        have := (List.any_eq_false.mp hbm) e he
        simp only [hnw', Option.isNone_none, Bool.true_and, Bool.not_eq_true] at this
        rcases hx : w.field? e.1.1 with _ | x
        · rw [hx] at this; simp at this
        · exact Or.inr ⟨fa, b, w, x, rfl, rfl, hx, Mem_field hsrc hx⟩
  -- every field's place for a value given that is outside what the field declares
  have hwit : ∀ given, RecAlign ρ gA given → baseMissing fields given srcV = false → ∀ e ∈ L, ∀ v,
      ((given.reverse.find? (·.1 == e.2)).map (·.2.1)).getD ((srcV.bind (·.field? e.1.1)).getD .none) = v →
      module = true → e.1.2.1.outside false v = true → Wit C (L.foldl (fieldStep module gA src) ([], oI, A)).2.1 := by
    intro given hal hbm e he v hv hmod hout'
    obtain ⟨⟨f, slot, sc⟩, k⟩ := e
    simp only at hv hout'
    obtain ⟨acc', hacc'⟩ := fold_step (fun b : List (String × Abs) × Out × ACtx => b.2.1) (fieldStep module gA src)
      fieldStep_grows L ([], oI, A) _ he
    refine Wit.grow ?_ hacc'
    rcases hval given hal k with ⟨h1, h2⟩ | ⟨a, x, c, w, l, h1, h2, hm, _⟩
    · rw [h2] at hv; simp only [Option.map_none, Option.getD_none] at hv
      rcases hbase given hbm _ he h2 with ⟨hs1, hs2⟩ | ⟨fa, b, w, x, hs1, hs2, hx, hmx⟩
      · subst hs2; simp at hv; subst hv
        unfold Slot.outside at hout'
        rw [outside_none] at hout'; simp at hout'
      · subst hs1; subst hs2
        simp only [Option.bind_some, hx, Option.getD_some] at hv
        subst hv
        obtain ⟨out', o', A'⟩ := acc'
        simp only [fieldStep, h1, hmod, Bool.true_and]
        split
        · exact wit_place hmx hout'
        · rename_i hno
          simp only [Bool.or_eq_true, Option.isSome_iff_ne_none, not_or, ne_eq, Decidable.not_not] at hno
          rw [outside_noRange hno.1 hno.2] at hout'; simp at hout'
    · rw [h2] at hv; simp at hv; subst hv
      obtain ⟨out', o', A'⟩ := acc'
      simp only [fieldStep, h1, hmod, Bool.true_and]
      split
      · exact wit_place hm hout'
      · rename_i hno
        simp only [Bool.or_eq_true, Option.isSome_iff_ne_none, not_or, ne_eq, Decidable.not_not] at hno
        rw [outside_noRange hno.1 hno.2] at hout'; simp at hout'
  -- the scope after the fields: each name keeps the range of its field
  have hsat : ∀ given, RecAlign ρ gA given →
      (∀ e ∈ L, module = true → e.1.2.1.outside false
        (((given.reverse.find? (·.1 == e.2)).map (·.2.1)).getD ((srcV.bind (·.field? e.1.1)).getD .none)) = false) →
      ∀ (L' : List ((String × Slot × Bool) × Nat)) (acc : List (String × Abs) × Out × ACtx), (∀ e ∈ L', e ∈ L) →
        Sat acc.2.2 ρ → Sat (L'.foldl (fieldStep module gA src) acc).2.2 ρ := by
    intro given hal hok L'
    induction L' with
    | nil => intro acc _ h; exact h
    | cons e L' ih =>
      intro acc hsub hS'
      simp only [List.foldl_cons]
      apply ih _ (fun e' h => hsub e' (by simp [h]))
      obtain ⟨out', o', A'⟩ := acc
      obtain ⟨⟨f, slot, sc⟩, k⟩ := e
      simp only [fieldStep]
      split
      · rename_i a x c hfa
        split
        · rename_i hmc
          simp only [Bool.and_eq_true] at hmc
          split
          · split
            · rename_i r hr
              refine hS'.assume ?_
              intro hcn s hs w hw
              rcases hval given hal k with ⟨h1, _⟩ | ⟨a', x', c', v, l, h1, h2, hm, hsym⟩
              · rw [h1] at hfa; simp at hfa
              · rw [h1] at hfa; simp at hfa
                obtain ⟨rfl, rfl, rfl, rfl⟩ := hfa
                rw [hsym hcn s hs] at hw; cases hw
                have := hok ((f, slot, sc), k) (hsub _ (by simp)) hmc.1
                simp only [h2, Option.map_some, Option.getD_some] at this
                exact outside_inRange hr this
            · exact hS'
          · exact hS'
        · exact hS'
      · split <;> exact hS'
  -- the record the evaluation builds, field by field
  have hlist : ∀ (g1 : (String × Slot × Bool) × Nat → Val) (g2 : (String × Slot × Bool) × Nat → Nat),
      (fields.zip (L.map g1)).zip (L.map g2) = L.map (fun e => ((e.1, g1 e), g2 e)) := by
    intro g1 g2
    rw [hF, List.zip_map', List.zip_map']
  have hnames : ∀ (g1 : (String × Slot × Bool) × Nat → Val), (fields.map (·.1)).zip (L.map g1) = L.map (fun e => (e.1.1, g1 e)) := by
    intro g1
    rw [hF, List.map_map, List.zip_map']
    rfl
  constructor
  · intro v hv
    obtain ⟨given, hg, hv⟩ := bind_ok hv
    have hal := hgi1 given hg
    simp only [List.nil_append] at hal
    unfold recordVal at hv
    split at hv
    · simp [failK] at hv
    rename_i hbm
    simp only [Bool.not_eq_true] at hbm
    -- every field keeps what it declares, where the record is a module's
    have hok : ∀ e ∈ L, module = true → e.1.2.1.outside false
        (((given.reverse.find? (·.1 == e.2)).map (·.2.1)).getD ((srcV.bind (·.field? e.1.1)).getD .none)) = false := by
      intro e he hmod
      subst hmod
      cases hol : outsideLine ((fields.zip (recordVals fields given srcV)).zip (recordLines line fields given)) with
      | some l => rw [hol] at hv; simp [failK] at hv
      | none =>
        unfold outsideLine at hol
        simp only [Option.map_eq_none_iff] at hol
        rw [List.find?_eq_none] at hol
        have hmem : ((e.1, ((given.reverse.find? (·.1 == e.2)).map (·.2.1)).getD ((srcV.bind (·.field? e.1.1)).getD .none)),
            ((given.reverse.find? (·.1 == e.2)).map (·.2.2)).getD line) ∈
            (fields.zip (recordVals fields given srcV)).zip (recordLines line fields given) := by
          unfold recordVals recordLines; rw [hL, hlist]; exact List.mem_map_of_mem he
        have := hol _ hmem
        simpa using this
    have hv' : v = built tag ((fields.map (·.1)).zip (recordVals fields given srcV)) := by
      split at hv
      · simp [failK] at hv
      · cases hv; rfl
    subst hv'
    refine ⟨?_, hsat given hal hok L ([], oI, A) (fun e h => h) hS⟩
    rw [hout]
    unfold recordVals
    rw [hL, hnames]
    -- each field of the value among what the analysis says of it
    have hfield : ∀ n x b, fieldV (L.map fun e => (e.1.1, ((given.reverse.find? (·.1 == e.2)).map (·.2.1)).getD
          ((srcV.bind (·.field? e.1.1)).getD .none))) n = some x →
        Abs.fieldOf (L.map fun e => (e.1.1, fieldAbs module e.1.2.1 gA src e.1.1 e.2)) n = some b → Mem b x := by
      intro n x b hx hb
      obtain ⟨e, he, rfl, rfl⟩ := find_map_pair hb hx
      obtain ⟨⟨f, slot, sc⟩, k⟩ := e
      simp only
      rcases hval given hal k with ⟨h1, h2⟩ | ⟨a, x, c, w, l, h1, h2, hm, _⟩
      · rcases hbase given hbm _ he h2 with ⟨hs1, hs2⟩ | ⟨fa, b', w, x', hs1, hs2, hfx, hmx⟩
        · subst hs1; subst hs2
          simp only [fieldAbs, h1, h2, Option.map_none, Option.getD_none, Option.bind_none]; exact Mem_none'
        · subst hs1; subst hs2
          simp only at hfx hmx
          simp only [fieldAbs, h1, h2, Option.map_none, Option.getD_none, Option.bind_some]
          rw [hfx, Option.getD_some]
          refine Mem_cutTo hmx ?_
          intro r hr hmod
          exact outside_inRange hr (by have := hok _ he hmod; simpa [h2, hfx] using this)
      · simp only [fieldAbs, h1, h2, Option.map_some, Option.getD_some]
        refine Mem_cutTo hm ?_
        intro r hr hmod
        exact outside_inRange hr (by have := hok _ he hmod; simpa [h2] using this)
    cases tag with
    | none =>
      simp only [built]
      exact Mem.record (fun n x hx b hb => hfield n x b hx hb) (fun _ _ _ h => by simp at h)
    | some t =>
      simp only [built]
      split
      · exact Mem.variantEnum (by simp [Abs.recOf])
      · rename_i hne
        refine Mem.variant (by simp [Abs.recOf]) (by simpa using hne) (fun n x hx => fieldOf_map_isSome hx)
          (fun n x hx b hb => hfield n x b hx hb)
  · intro k l hv hk
    rcases bind_err hv with hv | ⟨given, hg, hv⟩
    · exact Wit.grow (hgi2 _ hv).2 (hgf ([], oI, A))
    · have hal := hgi1 given hg
      simp only [List.nil_append] at hal
      unfold recordVal at hv
      split at hv
      · simp only [failK, Except.error.injEq, Stop.failed.injEq] at hv
        rw [← hv.1] at hk; simp [Kind.ofEffect] at hk
      rename_i hbm
      simp only [Bool.not_eq_true] at hbm
      cases hmod : module
      · rw [hmod] at hv; simp at hv
      · cases hol : outsideLine ((fields.zip (recordVals fields given srcV)).zip (recordLines line fields given)) with
        | none => rw [hmod, hol] at hv; simp at hv
        | some l' =>
          unfold outsideLine at hol
          rcases hfind : ((fields.zip (recordVals fields given srcV)).zip (recordLines line fields given)).find?
              (fun (((_, s, _), v), _) => s.outside false v) with _ | ⟨⟨fsv, v⟩, ln⟩ <;> rw [hfind] at hol <;> simp at hol
          have hpred := List.find?_some hfind
          have hmem := List.mem_of_find?_eq_some hfind
          unfold recordVals recordLines at hmem
          rw [hL, hlist] at hmem
          obtain ⟨e, he, heq⟩ := List.mem_map.mp hmem
          simp only [Prod.mk.injEq] at heq
          obtain ⟨⟨rfl, rfl⟩, rfl⟩ := heq
          have := hwit given hal hbm e he _ rfl hmod (by simpa using hpred)
          rw [hmod] at this; exact this

end KanameCore
