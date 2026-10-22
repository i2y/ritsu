/-
  Functions and the module (kaname's DESIGN 4.5, 15.2.3): a function's answer is among what its
  analysis says, for any values its parameters take, and a failure of one of the five kinds is one
  of its places that is not shown or one of a function it calls (`analyzeFn_sound`). Where a
  function's places are all shown and the functions it calls do not fail, it does not fail
  (`fn_noFail`); a function the module's analysis infers total does not fail (`total_sound`).

  The theorems of the module are for an analysis that let no running value of a `reduce` go
  (DESIGN 15.4): every function's facts say `widened` is false. A function that takes functions is
  left out (kaname's DESIGN 27.4): its own analysis answers for functions it does not see, and a call
  of it is read again with the functions it is given, or takes what it declares. A function whose
  effect goes on to its callers (`through`, kaname's DESIGN 27.5) is left out of `total_sound`: what
  it does depends on the functions it is given.
-/
import KanameCore.SoundAll

namespace KanameCore

-- ── The scope a function starts in ──────────────────────────────────────────

/-- **The scope a function starts in holds the values its parameters take.** -/
theorem sat_start {M : Module} {R : List (Option Abs)} {ring : List Nat} {fn : Fn} {args : List Val}
    (h : Valid fn args) :
    Sat ⟨M, R, ring, fn.params.foldl (fun env p => (p.name, p.abs) :: env) [], [], 0, true, false⟩
      ((fn.params.map (·.name)).zip args).reverse := by
  rw [foldl_params, List.append_nil]
  exact sat_paired _ _ (Paired.reverse (paired_params fn.params args h.1 fun p v hpv => (h.2 p v hpv).1))

-- ── A call of a function ─────────────────────────────────────────────────────

/-- What the analysis of a function that takes no function says of every call of it with values its
    parameters take: its answer among the function's result, a failure of one of the five kinds one
    of its places or one of a function it calls; every place of one of the five kinds. Facts that
    say the function takes no function and calls none it does not see (`unseen`) are of such a
    function. -/
def FnSound (C : Ctx) (f : Nat) (ff : FnFacts) : Prop :=
  ∀ fn, C.M.fns[f]? = some fn → (ff.unseen = false → fn.takesFn = false) ∧ (fn.takesFn = false →
    (∀ n dep args v, Valid fn args → callFn C n dep f args = .ok v → Mem ff.result v) ∧
    (∀ n dep args k l, Valid fn args → callFn C n dep f args = .error (.failed k l) → k.ofEffect = true →
      Wit C ⟨ff.places, [], ff.calls, false, [], [], [], [], false⟩) ∧
    (∀ p ∈ ff.places, p.kind ∈ kindOrder))

theorem wit_facts {C : Ctx} {o : Out} (h : Wit C o) : Wit C ⟨o.places, [], o.calls, false, [], [], [], [], false⟩ := h

theorem kinds_start {vals : List (Nat × Abs)} : KindsOK ⟨[], vals, [], false, [], [], [], [], false⟩ := fun _ h => by cases h

/-- **The analysis of a function is sound** (DESIGN 15.2.2 for a function's answer and places),
    for an analysis that let no running value go, of a function that takes no function. -/
theorem analyzeFn_sound {C : Ctx} {res : List (Option Abs)} (hR : ResultsOK C res) (hO : OracleOK C)
    {ring : List Nat} {f : Nat} {vals : List (Nat × Abs)}
    (hw : ∀ fn, C.M.fns[f]? = some fn → fn.takesFn = false → (analyzeFn C.M res ring f vals).1.widened = false) :
    FnSound C f (analyzeFn C.M res ring f vals).1 := by
  intro fn hf
  have hw := hw fn hf
  simp only [analyzeFn, hf] at hw ⊢
  refine ⟨fun hu => by simp only [Bool.or_eq_false_iff] at hu; exact hu.2, fun htf => ?_⟩
  have hw := hw htf
  generalize hA : (⟨C.M, res, ring, fn.params.foldl (fun env p => (p.name, p.abs) :: env) [], [], 0, true, false⟩ : ACtx)
    = A0 at hw ⊢
  generalize hO0 : (⟨[], vals, [], false, [], [], [], [], false⟩ : Out) = O0 at hw ⊢
  have hG : Good C res A0 := by rw [← hA]; exact ⟨rfl, rfl⟩
  have g0 : Grows O0 (ev A0 FUEL fn.body O0).out := ((sat FUEL).ev _ _ _).1
  -- the out at the end grows from the body's
  have gend : Grows (ev A0 FUEL fn.body O0).out
      (if (fn.ret.range?.isSome || fn.ret.count?.isSome) = true then
        (convA (ev A0 FUEL fn.body O0).out fn.conv (ev A0 FUEL fn.body O0).abs).2.place
          fn.answer "answer" .range (.slot fn.ret)
          (convA (ev A0 FUEL fn.body O0).out fn.conv (ev A0 FUEL fn.body O0).abs).1
      else (convA (ev A0 FUEL fn.body O0).out fn.conv (ev A0 FUEL fn.body O0).abs).2).merged := by
    apply grows_merged
    split
    · exact grows_place (grows_convA (Grows.refl _))
    · exact grows_convA (Grows.refl _)
  have hwb := grows_not_widened gend hw
  refine ⟨fun n dep args v hv h => ?_, fun n dep args k l hv h hk => ?_, ?_⟩
  · obtain ⟨n', raw, rfl, hraw, hc, _, hhas⟩ := callFn_ok hf h
    have hS : Sat A0 ((fn.params.map (·.name)).zip args).reverse := by rw [← hA]; exact sat_start hv
    obtain ⟨hm, _, _⟩ := (ranges_sound hR hO hG hS hwb).1 raw hraw
    exact cutD_sound D _ _ _ (convA_mem hm hc) (has_sound' hhas)
  · have hS : Sat A0 ((fn.params.map (·.name)).zip args).reverse := by rw [← hA]; exact sat_start hv
    obtain ⟨n', rfl, h1 | ⟨raw, hraw, h1⟩ | ⟨raw, v, hraw, hc, hout⟩⟩ := callFn_err hf h hk
    all_goals have R := ranges_sound (n := n') (dep := dep + 1) hR hO hG hS hwb
    all_goals apply wit_facts
    · exact Wit.grow (R.2 k l h1 hk) gend
    · refine Wit.grow (coerce_wit (o := (ev A0 FUEL fn.body O0).out) (R.1 raw hraw).1 h1).2 (grows_merged ?_)
      split
      · exact grows_place (Grows.refl _)
      · exact Grows.refl _
    · have hm := convA_mem (o := (ev A0 FUEL fn.body O0).out) (R.1 raw hraw).1 hc
      refine Wit.grow ?_ (grows_merged (Grows.refl _))
      split
      · exact wit_place hm hout
      · rename_i hno
        simp only [Bool.or_eq_true, not_or, Option.isSome_iff_ne_none, ne_eq, Decidable.not_not] at hno
        rw [outside_noRange hno.1 hno.2] at hout; cases hout
  · have k0 : KindsOK O0 := by rw [← hO0]; exact kinds_start
    exact gend.kinds (g0.kinds k0)

-- ── Where a function does not fail ──────────────────────────────────────────

/-- **A function whose places are all shown, and which calls only functions that do not fail,
    does not fail** for any values its parameters take (DESIGN 15.2.3). -/
theorem fn_noFail {C : Ctx} {f : Nat} {ff : FnFacts} (hf : FnSound C f ff) (hu : ff.unseen = false)
    (hs : ∀ p ∈ ff.places, p.shown = true) (hc : ∀ g ∈ ff.calls, NoFail C g) : NoFail C f := by
  intro fn hfn n dep args hv k l h
  cases hk : k.ofEffect
  · rfl
  · exact absurd (((hf fn hfn).2 ((hf fn hfn).1 hu)).2.1 n dep args k l hv h hk)
      (no_wit (o := ⟨ff.places, [], ff.calls, false, [], [], [], [], false⟩) hs hc)

/-- **An expression whose places are all shown, and which calls only functions that do not fail,
    does not fail** in any environment the scope of its analysis says (DESIGN 15.2.3). -/
theorem no_failure {C : Ctx} {res : List (Option Abs)} (hR : ResultsOK C res) (hO : OracleOK C) {n m dep : Nat}
    {A : ACtx} {ρ : Env} {e : Expr} {o : Out} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (ev A m e o).out.widened = false) (hs : ∀ p ∈ (ev A m e o).out.places, p.shown = true)
    (hc : ∀ g ∈ (ev A m e o).out.calls, NoFail C g) :
    ∀ k l, eval C n dep ρ e = .error (.failed k l) → k.ofEffect = false := by
  intro k l h
  cases hk : k.ofEffect
  · rfl
  · exact absurd ((ranges_sound hR hO hG hS hw).2 k l h hk) (no_wit hs hc)

-- ── The functions of a module, ring by ring ─────────────────────────────────

abbrev AState := List FnFacts × List (Option Abs) × List (Nat × Abs)

/-- One function of a ring analysed, the rings laid out one after another. -/
def flatStep (M : Module) (acc : AState) (y : List Nat × Nat) : AState := analyzeStep M y.1 acc y.2

/-- What the analysis of a module keeps so far: a function analysed has its facts, sound. -/
def FoldOK (C : Ctx) (acc : AState) : Prop :=
  acc.1.length = acc.2.1.length ∧
    ∀ g a, acc.2.1[g]? = some (some a) → ∃ ff, acc.1[g]? = some ff ∧ ff.result = a ∧ FnSound C g ff

theorem FoldOK.results {C : Ctx} {acc : AState} (h : FoldOK C acc) : ResultsOK C acc.2.1 := by
  intro f a hf fn hfn htf n dep args v hv hc
  obtain ⟨ff, _, rfl, hs⟩ := h.2 f a hf
  exact ((hs fn hfn).2 htf).1 n dep args v hv hc

theorem analyzeStep_keep {M : Module} {ring : List Nat} {acc : AState} {f g : Nat}
    (hg : (acc.2.1[g]?).join.isSome = true) :
    (analyzeStep M ring acc f).1[g]? = acc.1[g]? ∧ (analyzeStep M ring acc f).2.1[g]? = acc.2.1[g]? := by
  unfold analyzeStep
  split
  · exact ⟨rfl, rfl⟩
  · rename_i hf
    have hne : f ≠ g := by intro e; subst e; exact hf hg
    exact ⟨List.getElem?_set_ne hne, List.getElem?_set_ne hne⟩

theorem fold_keep {M : Module} : ∀ (L : List (List Nat × Nat)) (acc : AState) (g : Nat),
    (acc.2.1[g]?).join.isSome = true →
    (L.foldl (flatStep M) acc).1[g]? = acc.1[g]? ∧
    (L.foldl (flatStep M) acc).2.1[g]? = acc.2.1[g]?
  | [], _, _, _ => ⟨rfl, rfl⟩
  | y :: L, acc, g, hg => by
    simp only [List.foldl_cons]
    have h1 := analyzeStep_keep (M := M) (ring := y.1) (f := y.2) hg
    have hg' : ((flatStep M acc y).2.1[g]?).join.isSome = true := by rw [flatStep, h1.2]; exact hg
    obtain ⟨h2, h3⟩ := fold_keep L _ g hg'
    exact ⟨h2.trans h1.1, h3.trans h1.2⟩

theorem analyzeStep_length {M : Module} {ring : List Nat} {acc : AState} {f : Nat} :
    (analyzeStep M ring acc f).1.length = acc.1.length ∧ (analyzeStep M ring acc f).2.1.length = acc.2.1.length := by
  unfold analyzeStep
  split
  · exact ⟨rfl, rfl⟩
  · simp

theorem fold_length {M : Module} : ∀ (L : List (List Nat × Nat)) (acc : AState),
    (L.foldl (flatStep M) acc).2.1.length = acc.2.1.length
  | [], _ => rfl
  | y :: L, acc => by simp only [List.foldl_cons]; rw [fold_length L _, flatStep, analyzeStep_length.2]

theorem analyzeStep_ok {C : Ctx} (hO : OracleOK C) {ring : List Nat} {acc : AState} {f : Nat} (h : FoldOK C acc)
    (hw : f < acc.2.1.length → (acc.2.1[f]?).join.isSome = false → ∀ fn, C.M.fns[f]? = some fn → fn.takesFn = false →
      (analyzeFn C.M acc.2.1 ring f acc.2.2).1.widened = false) :
    FoldOK C (analyzeStep C.M ring acc f) := by
  unfold analyzeStep
  split
  · exact h
  · rename_i hf
    have hf' : (acc.2.1[f]?).join.isSome = false := by simpa using hf
    refine ⟨by simp [h.1], fun g a hg => ?_⟩
    simp only at hg ⊢
    by_cases hfg : f = g
    · subst hfg
      have hlt : f < acc.2.1.length := by
        rcases Nat.lt_or_ge f acc.2.1.length with hlt | hge
        · exact hlt
        · rw [List.set_eq_of_length_le hge, List.getElem?_eq_none hge] at hg; cases hg
      rw [List.getElem?_set_self hlt] at hg
      cases hg
      refine ⟨_, List.getElem?_set_self (by rw [h.1]; exact hlt), rfl, ?_⟩
      exact analyzeFn_sound h.results hO (hw hlt hf')
    · rw [List.getElem?_set_ne hfg] at hg ⊢
      exact h.2 g a hg

theorem fold_ok {C : Ctx} (hO : OracleOK C) : ∀ (L : List (List Nat × Nat)) (acc : AState), FoldOK C acc →
    (∀ (g : Nat) fn, C.M.fns[g]? = some fn → fn.takesFn = false →
      (((L.foldl (flatStep C.M) acc).1)[g]?.getD (default : FnFacts)).widened = false) →
    FoldOK C (L.foldl (flatStep C.M) acc)
  | [], _, h, _ => h
  | y :: L, acc, h, hw => by
    simp only [List.foldl_cons] at hw ⊢
    apply fold_ok hO L _ _ hw
    apply analyzeStep_ok hO h
    intro hlt hf fn hfn htf
    -- the facts the function is given here are its facts at the end
    have hsome : ((flatStep C.M acc y).2.1[y.2]?).join.isSome = true := by
      unfold flatStep analyzeStep
      rw [ite_eq_right (by simp [hf])]
      simp [List.getElem?_set_self hlt]
    have hk := (fold_keep (M := C.M) L _ y.2 hsome).1
    have := hw y.2 fn hfn htf
    rw [hk] at this
    unfold flatStep analyzeStep at this
    rw [ite_eq_right (by simp [hf])] at this
    simp only at this
    rw [List.getElem?_set_self (by rw [h.1]; exact hlt)] at this
    simpa using this

theorem fold_analyzed {M : Module} : ∀ (L : List (List Nat × Nat)) (acc : AState) (ring : List Nat) (f : Nat),
    (ring, f) ∈ L → f < acc.2.1.length →
    ((L.foldl (flatStep M) acc).2.1[f]?).join.isSome = true
  | [], _, _, _, h, _ => by cases h
  | y :: L, acc, ring, f, hm, hlt => by
    simp only [List.foldl_cons]
    rcases List.mem_cons.mp hm with he | hm
    · subst he
      have hsome : ((flatStep M acc (ring, f)).2.1[f]?).join.isSome = true := by
        unfold flatStep analyzeStep
        split
        · assumption
        · simp [List.getElem?_set_self hlt]
      rw [(fold_keep L _ f hsome).2]; exact hsome
    · exact fold_analyzed L _ ring f hm (by rw [flatStep, analyzeStep_length.2]; exact hlt)

theorem inner_flatten {M : Module} (ring : List Nat) (acc : AState) :
    ring.foldl (analyzeStep M ring) acc = (ring.map fun f => (ring, f)).foldl (flatStep M) acc := by
  rw [List.foldl_map]; rfl

theorem fold_flatten {M : Module} : ∀ (rings : List (List Nat × Bool)) (acc : AState),
    rings.foldl (fun acc x => x.1.foldl (analyzeStep M x.1) acc) acc =
      (rings.flatMap fun x => x.1.map fun f => (x.1, f)).foldl (flatStep M) acc
  | [], _ => rfl
  | x :: rings, acc => by
    simp only [List.foldl_cons, List.flatMap_cons, List.foldl_append]
    rw [inner_flatten]
    exact fold_flatten rings _

-- ── The effects, ring by ring ───────────────────────────────────────────────

/-- An effect with no kind, that does not go round, and that does not take on the effect of a
    function its function is given (kaname's DESIGN 27.5). -/
def Clean (e : Effect) : Prop := e.fails = [] ∧ e.diverges = false ∧ e.through = false

/-- The kinds of an effect are among the five. -/
def SubK (e : Effect) : Prop := ∀ k ∈ e.fails, k ∈ kindOrder

theorem total_clean {e : Effect} (ht : e.total = true) (hth : e.through = false) : Clean e := by
  simp only [Effect.total, Bool.and_eq_true, Bool.not_eq_true', List.isEmpty_iff] at ht
  exact ⟨ht.1, ht.2, hth⟩

theorem addKind_sub {ks : List Kind} {k : Kind} : ∀ x ∈ addKind ks k, x ∈ kindOrder := by
  intro x hx; unfold addKind at hx; exact (List.mem_filter.mp hx).1

theorem addKind_has {ks : List Kind} {k : Kind} (hk : k ∈ kindOrder) : k ∈ addKind ks k := by
  unfold addKind; exact List.mem_filter.mpr ⟨hk, by simp⟩

theorem addKind_keep {ks : List Kind} {k x : Kind} (hx : x ∈ ks) (hxk : x ∈ kindOrder) : x ∈ addKind ks k := by
  unfold addKind; exact List.mem_filter.mpr ⟨hxk, by simp [hx]⟩

theorem foldl_addKind_sub : ∀ (ls ks : List Kind), (∀ x ∈ ks, x ∈ kindOrder) → ∀ x ∈ ls.foldl addKind ks, x ∈ kindOrder
  | [], _, h => h
  | _ :: ls, _, _ => foldl_addKind_sub ls _ (fun _ hx => addKind_sub _ hx)

theorem foldl_addKind_keep : ∀ (ls ks : List Kind) (x : Kind), x ∈ ks → x ∈ kindOrder → x ∈ ls.foldl addKind ks
  | [], _, _, hx, _ => hx
  | _ :: ls, _, x, hx, hxk => foldl_addKind_keep ls _ x (addKind_keep hx hxk) hxk

theorem foldl_addKind_has : ∀ (ls ks : List Kind) (x : Kind), x ∈ ls → x ∈ kindOrder → x ∈ ls.foldl addKind ks
  | [], _, _, hx, _ => by cases hx
  | y :: ls, ks, x, hx, hxk => by
    rcases List.mem_cons.mp hx with rfl | hx
    · exact foldl_addKind_keep ls _ x (addKind_has hxk) hxk
    · exact foldl_addKind_has ls _ x hx hxk

theorem nil_of_sub {ks : List Kind} (h : ∀ x ∈ ks, x ∈ kindOrder) (h0 : ∀ x ∈ kindOrder, x ∉ ks) : ks = [] := by
  cases ks with
  | nil => rfl
  | cons x _ => exact absurd (List.mem_cons_self ..) (h0 x (h x (List.mem_cons_self ..)))

/-- A fold of steps that only add kinds and only go round: where it ends clean, it started clean and
    each step's condition held. -/
theorem foldl_clean {α : Type} {s : Effect → α → Effect} {cond : α → Prop}
    (hsub : ∀ e x, SubK e → SubK (s e x)) (hs : ∀ e x, SubK e → Clean (s e x) → Clean e ∧ cond x) :
    ∀ (l : List α) (e : Effect), SubK e → (SubK (l.foldl s e) ∧ (Clean (l.foldl s e) → Clean e ∧ ∀ x ∈ l, cond x))
  | [], _, h => ⟨h, fun hc => ⟨hc, fun _ h => by cases h⟩⟩
  | x :: l, e, h => by
    simp only [List.foldl_cons]
    obtain ⟨h1, h2⟩ := foldl_clean hsub hs l (s e x) (hsub e x h)
    refine ⟨h1, fun hc => ?_⟩
    obtain ⟨hc', hl⟩ := h2 hc
    obtain ⟨he, hx⟩ := hs e x h hc'
    exact ⟨he, fun y hy => by rcases List.mem_cons.mp hy with rfl | hy; exact hx; exact hl y hy⟩

theorem placeEffect_sub (e : Effect) (p : Place) (h : SubK e) : SubK (placeEffect e p) := by
  unfold placeEffect
  split
  · exact h
  · split
    · exact fun x hx => addKind_sub x hx
    · exact fun x hx => addKind_sub x hx

theorem placeEffect_clean (e : Effect) (p : Place) (h : SubK e) (hc : Clean (placeEffect e p)) :
    Clean e ∧ (p.kind ∈ kindOrder → p.shown = true) := by
  unfold placeEffect at hc
  split at hc
  · rename_i hs; exact ⟨hc, fun _ => hs⟩
  · have key : addKind e.fails p.kind = [] → Clean e ∧ (p.kind ∈ kindOrder → p.shown = true) := by
      intro h0
      refine ⟨⟨nil_of_sub h fun x hx hxe => ?_, ?_⟩, fun hk => ?_⟩
      · have := addKind_keep (k := p.kind) hxe hx; rw [h0] at this; cases this
      · split at hc <;> exact hc.2
      · have := addKind_has (ks := e.fails) hk; rw [h0] at this; cases this
    split at hc
    · exact key hc.1
    · exact key hc.1

theorem callEffect_sub {ring : List Nat} {effs : List Effect}
    (e : Effect) (g : Nat) (h : SubK e) : SubK (callEffect ring effs e g) := by
  unfold callEffect
  split
  · exact h
  · split
    · exact fun x hx => foldl_addKind_sub _ _ h x hx
    · exact h

theorem callEffect_clean {ring : List Nat} {effs : List Effect} (heffs : ∀ (g : Nat) (o : Effect), effs[g]? = some o → SubK o)
    (e : Effect) (g : Nat) (h : SubK e) (hc : Clean (callEffect ring effs e g)) :
    Clean e ∧ (ring.contains g = false ∧ ∀ o, effs[g]? = some o → Clean o) := by
  unfold callEffect at hc
  split at hc
  · simp [Clean] at hc
  · rename_i hr
    split at hc
    · rename_i o ho
      obtain ⟨h1, h2⟩ := hc
      simp only [Bool.or_eq_false_iff] at h2
      simp only at h1
      have he : e.fails = [] := nil_of_sub h fun x hx hxe => by
        have := foldl_addKind_keep o.fails e.fails x hxe hx; rw [h1] at this; cases this
      have hoo : o.fails = [] := nil_of_sub (heffs g o ho) fun x hx hxo => by
        have := foldl_addKind_has o.fails e.fails x hxo hx; rw [h1] at this; cases this
      obtain ⟨⟨hd1, hd2⟩, ht1, ht2⟩ := h2
      refine ⟨⟨he, hd1, ht1⟩, by simpa using hr, fun o' ho' => ?_⟩
      rw [ho] at ho'; cases ho'
      exact ⟨hoo, hd2, ht2⟩
    · rename_i hn
      refine ⟨hc, by simpa using hr, fun o ho => ?_⟩
      rw [ho] at hn; cases hn

theorem thruEffect_sub (e : Effect) (k : List Kind × Bool) (h : SubK e) : SubK (thruEffect e k) :=
  fun x hx => foldl_addKind_sub _ _ h x hx

theorem thruEffect_clean (e : Effect) (k : List Kind × Bool) (h : SubK e) (hc : Clean (thruEffect e k)) :
    Clean e ∧ True := by
  obtain ⟨h1, h2, h3⟩ := hc
  simp only [thruEffect, Bool.or_eq_false_iff] at h1 h2 h3
  refine ⟨⟨nil_of_sub h fun x hx hxe => ?_, h2.1, h3⟩, trivial⟩
  have := foldl_addKind_keep k.1 e.fails x hxe hx; rw [h1] at this; cases this

/-- **A ring whose effect is clean**: every place of its functions of one of the five kinds is
    shown, every function they call is outside the ring, with a clean effect, and none of them takes
    a function or calls one the analysis does not see. -/
theorem ringEffect_clean {facts : List FnFacts} {effs : List Effect} {ring : List Nat} {round : Bool}
    (heffs : ∀ (g : Nat) (o : Effect), effs[g]? = some o → SubK o) :
    SubK (ringEffect facts effs ring round) ∧
    (Clean (ringEffect facts effs ring round) → round = false ∧ ∀ f ∈ ring,
      (∀ p ∈ (facts[f]?.getD (default : FnFacts)).places, p.kind ∈ kindOrder → p.shown = true) ∧
      (∀ g ∈ (facts[f]?.getD (default : FnFacts)).calls, ring.contains g = false ∧ ∀ o, effs[g]? = some o → Clean o) ∧
      (facts[f]?.getD (default : FnFacts)).unseen = false) := by
  have hfn : ∀ e f, SubK e → SubK (fnEffect facts ring effs e f) ∧ (Clean (fnEffect facts ring effs e f) → Clean e ∧
      ((∀ p ∈ (facts[f]?.getD (default : FnFacts)).places, p.kind ∈ kindOrder → p.shown = true) ∧
      (∀ g ∈ (facts[f]?.getD (default : FnFacts)).calls, ring.contains g = false ∧ ∀ o, effs[g]? = some o → Clean o) ∧
      (facts[f]?.getD (default : FnFacts)).unseen = false)) := by
    intro e f h
    unfold fnEffect
    obtain ⟨p1, p2⟩ := foldl_clean (cond := fun p : Place => p.kind ∈ kindOrder → p.shown = true)
      placeEffect_sub placeEffect_clean (facts[f]?.getD (default : FnFacts)).places e h
    obtain ⟨c1, c2⟩ := foldl_clean (cond := fun g => ring.contains g = false ∧ ∀ o, effs[g]? = some o → Clean o)
      callEffect_sub (callEffect_clean heffs) (facts[f]?.getD (default : FnFacts)).calls _ p1
    obtain ⟨t1, t2⟩ := foldl_clean (cond := fun _ => True) thruEffect_sub thruEffect_clean
      (facts[f]?.getD (default : FnFacts)).thru _ c1
    refine ⟨t1, fun hc => ?_⟩
    obtain ⟨hc1, hc2, hc3⟩ := hc
    simp only [Bool.or_eq_false_iff] at hc3
    obtain ⟨hct, _⟩ := t2 ⟨hc1, hc2, hc3.1⟩
    obtain ⟨hcc, hcalls⟩ := c2 hct
    obtain ⟨hcp, hplaces⟩ := p2 hcc
    exact ⟨hcp, hplaces, hcalls, hc3.2⟩
  unfold ringEffect
  obtain ⟨r1, r2⟩ := foldl_clean (s := fnEffect facts ring effs) (fun e f h => (hfn e f h).1)
    (fun e f h hc => (hfn e f h).2 hc) ring ⟨[], [], round, false⟩ (fun _ h => by cases h)
  refine ⟨r1, fun hc => ?_⟩
  obtain ⟨⟨_, hr, _⟩, hall⟩ := r2 hc
  exact ⟨hr, hall⟩

theorem foldl_set_get {α : Type} (e : α) : ∀ (ring : List Nat) (l : List α) (g : Nat),
    (ring.foldl (fun l f => l.set f e) l)[g]? = if g ∈ ring ∧ g < l.length then some e else l[g]?
  | [], l, g => by simp
  | f :: ring, l, g => by
    simp only [List.foldl_cons]
    rw [foldl_set_get e ring (l.set f e) g]
    by_cases hfg : f = g
    · subst hfg
      by_cases hlt : f < l.length
      · simp [hlt]
      · simp [hlt]
    · by_cases hg : g ∈ ring <;> simp [hg, Ne.symm hfg, List.getElem?_set_ne hfg]

theorem foldl_set_length {α : Type} (e : α) : ∀ (ring : List Nat) (l : List α),
    (ring.foldl (fun l f => l.set f e) l).length = l.length
  | [], _ => rfl
  | _ :: ring, l => by simp only [List.foldl_cons]; rw [foldl_set_length e ring _]; simp

/-- What the effects so far say: a function with a total effect does not fail; every kind is one of
    the five. -/
def EffOK (C : Ctx) (effs : List Effect) : Prop :=
  effs.length = C.M.fns.length ∧ (∀ (g : Nat) (o : Effect), effs[g]? = some o → SubK o) ∧
    ∀ (g : Nat) (o : Effect), effs[g]? = some o → Clean o → NoFail C g

theorem effects_ok {C : Ctx} (facts : List FnFacts) :
    ∀ (rings : List (List Nat × Bool)) (effs : List Effect),
    (∀ x ∈ rings, ∀ f ∈ x.1, f < C.M.fns.length → FnSound C f (facts[f]?.getD (default : FnFacts))) →
    EffOK C effs → EffOK C (rings.foldl (effectsStep facts) effs)
  | [], _, _, h => h
  | x :: rings, effs, hs, h => by
    simp only [List.foldl_cons]
    apply effects_ok facts rings _ (fun y hy => hs y (List.mem_cons_of_mem _ hy))
    obtain ⟨hlen, hsub, hno⟩ := h
    obtain ⟨e1, e2⟩ := ringEffect_clean (facts := facts) (ring := x.1) (round := x.2) hsub
    unfold effectsStep
    refine ⟨by rw [foldl_set_length]; exact hlen, fun g o ho => ?_, fun g o ho hc => ?_⟩
    · rw [foldl_set_get] at ho
      split at ho
      · cases ho; exact e1
      · exact hsub g o ho
    · rw [foldl_set_get] at ho
      split at ho
      · -- a function of this ring
        rename_i hg
        cases ho
        obtain ⟨_, hall⟩ := e2 hc
        obtain ⟨hplaces, hcalls, hunseen⟩ := hall g hg.1
        have hgl : g < C.M.fns.length := by rw [← hlen]; exact hg.2
        have hsg := hs x (List.mem_cons_self ..) g hg.1 hgl
        refine fn_noFail hsg hunseen (fun p hp => hplaces p hp ?_) (fun g' hg' => ?_)
        · have hfn := List.getElem?_eq_getElem hgl
          exact ((hsg _ hfn).2 ((hsg _ hfn).1 hunseen)).2.2 p hp
        · obtain ⟨_, hgo⟩ := hcalls g' hg'
          cases hg'' : effs[g']? with
          | some o' => exact hno g' o' hg'' (hgo o' hg'')
          | none =>
            intro fn hfn
            have : C.M.fns.length ≤ g' := by
              rw [← hlen]; exact List.getElem?_eq_none_iff.mp hg''
            rw [List.getElem?_eq_none this] at hfn; cases hfn
      · exact hno g o ho hc

theorem foldOK_start {C : Ctx} {n : Nat} : FoldOK C (List.replicate n default, List.replicate n none, []) := by
  refine ⟨by simp, fun g a h => ?_⟩
  simp only at h
  rw [List.getElem?_replicate] at h
  split at h <;> cases h

theorem effOK_start {C : Ctx} : EffOK C (List.replicate C.M.fns.length ⟨[], [], true, false⟩) := by
  refine ⟨by simp, fun g o h => ?_, fun g o h hc => ?_⟩
  · rw [List.getElem?_replicate] at h
    split at h
    · cases h; intro k hk; cases hk
    · cases h
  · rw [List.getElem?_replicate] at h
    split at h
    · cases h; exact absurd hc.2.1 (by simp)
    · cases h

/-- **The functions of a module are sound** (DESIGN 15.2.2): the answer the analysis gives each
    function analysed that takes no function holds of every call of it, for an analysis that let no
    running value go. -/
theorem module_sound {C : Ctx} (hO : OracleOK C)
    (hw : ∀ (g : Nat) fn, C.M.fns[g]? = some fn → fn.takesFn = false →
      (((analyzeModule C.M).1)[g]?.getD (default : FnFacts)).widened = false) :
    ∀ x ∈ C.M.rings, ∀ f ∈ x.1, f < C.M.fns.length → FnSound C f (((analyzeModule C.M).1)[f]?.getD default) := by
  intro x hx f hf hlt
  have hflat := fold_flatten (M := C.M) C.M.rings (List.replicate C.M.fns.length default, List.replicate C.M.fns.length none, [])
  have hfin : (analyzeModule C.M).1 = (C.M.rings.foldl (fun acc x => x.1.foldl (analyzeStep C.M x.1) acc)
      (List.replicate C.M.fns.length default, List.replicate C.M.fns.length none, [])).1 := rfl
  rw [hfin, hflat] at hw ⊢
  have hok := fold_ok hO _ _ foldOK_start hw
  have han := fold_analyzed (M := C.M) _ (List.replicate C.M.fns.length default, List.replicate C.M.fns.length none, [])
    x.1 f ((List.mem_flatMap (f := fun (x : List Nat × Bool) => x.1.map fun f => (x.1, f))).mpr
      ⟨x, hx, List.mem_map_of_mem (f := fun f => (x.1, f)) hf⟩) (by simpa using hlt)
  generalize (C.M.rings.flatMap fun x => x.1.map fun f => (x.1, f)).foldl (flatStep C.M)
    (List.replicate C.M.fns.length default, List.replicate C.M.fns.length none, []) = fin at hok han ⊢
  cases hr : fin.2.1[f]? with
  | none => rw [hr] at han; cases han
  | some ro =>
    cases ro with
    | none => rw [hr] at han; cases han
    | some a =>
      obtain ⟨ff, hff, _, hs⟩ := hok.2 f a hr
      rw [hff]; exact hs

/-- **A function the module's analysis infers total does not fail** for any values its parameters
    take (DESIGN 4.5, 15.2.3), for an analysis that let no running value go, where the effect of no
    function it is given goes on to it (kaname's DESIGN 27.5): a function the host calls. -/
theorem total_sound {C : Ctx} (hO : OracleOK C)
    (hw : ∀ (g : Nat) fn, C.M.fns[g]? = some fn → fn.takesFn = false →
      (((analyzeModule C.M).1)[g]?.getD (default : FnFacts)).widened = false) :
    ∀ f e, (analyzeModule C.M).2.1[f]? = some e → e.total = true → e.through = false → NoFail C f := by
  intro f e hf ht hth
  have hok := effects_ok (C := C) (analyzeModule C.M).1 C.M.rings (List.replicate C.M.fns.length ⟨[], [], true, false⟩)
    (module_sound hO hw) effOK_start
  exact hok.2.2 f e hf (total_clean ht hth)

end KanameCore
