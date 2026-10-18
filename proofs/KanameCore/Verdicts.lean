/-
  The verdicts of the checks across the languages, and what a call of a rule keeps between the
  inputs it gives (kaname's DESIGN 14.3, 14.5, 15.2).

  `analyzeWith` takes ritsu-cross's verdicts at the places they decide, after the analysis, as
  `ranges.rs` does. A verdict changes no place but one that waits for it (`decideAcross_other`), and
  with no verdicts the analysis is `analyzeModule`, which the theorems of `Total.lean` are of
  (`analyzeWith_nil`). A function the effects inferred from the verdicts call total does not fail,
  where what each verdict that holds says is so: the facts with the verdicts taken are sound
  (`total_with_sound`; what ritsu-cross shows of each check is RitsuCross's to prove).

  A call of a rule keeps the relations the conditions in force and the records of `running_totals`
  say of the inputs it gives (`callRels`, what kaname hands ritsu-cross for X2): each is between two
  names given to two inputs, and holds between their values in every environment the scope of the
  call holds (`callRels_sound`). `Cross.lean` puts it together with RitsuCross's relation stage.
-/
import KanameCore.Total

namespace KanameCore

-- ── The verdicts ─────────────────────────────────────────────────────────────

theorem decideAcross_nil (p : Place) : decideAcross [] p = p := by
  unfold decideAcross
  split <;> simp

/-- **A verdict changes no place but one that waits for it.** -/
theorem decideAcross_other {vs : List (Nat × String × Bool)} {p : Place} (h : p.pending = false) :
    decideAcross vs p = p := by
  unfold decideAcross
  split
  · rename_i hw; simp [Place.pending, hw] at h
  · rfl

/-- A place that waits, with the verdict that names it: decided as the verdict says. -/
theorem decideAcross_named {vs : List (Nat × String × Bool)} {p : Place} (h : p.pending = true)
    {v : Nat × String × Bool} (hv : vs.find? (fun v => v.1 == p.expr && v.2.1 == p.key) = some v) :
    decideAcross vs p = { p with what := .across v.2.2 } := by
  unfold decideAcross
  split
  · rw [hv]
  · rename_i hw; unfold Place.pending at h; split at h <;> simp_all

theorem map_decideAcross_nil : ∀ (l : List Place), l.map (decideAcross []) = l
  | [] => rfl
  | p :: l => by simp only [List.map_cons, decideAcross_nil, map_decideAcross_nil l]

theorem decided_nil (ff : FnFacts) : ff.decided [] = ff := by
  unfold FnFacts.decided
  rw [map_decideAcross_nil]

theorem map_decided_nil : ∀ (l : List FnFacts), l.map (FnFacts.decided []) = l
  | [] => rfl
  | ff :: l => by simp only [List.map_cons, decided_nil, map_decided_nil l]

/-- What `analyzeWith` is made of: the facts of `analyzeModule` with the verdicts taken, the effects
    inferred from them, the values of the expressions as they were. -/
theorem analyzeWith_parts (M : Module) :
    (analyzeWith M).1 = (analyzeModule M).1.map (FnFacts.decided M.verdicts) ∧
    (analyzeWith M).2.1 = M.rings.foldl (effectsStep (analyzeWith M).1) (List.replicate M.fns.length ⟨[], [], true⟩) ∧
    (analyzeWith M).2.2 = (analyzeModule M).2.2 := by
  unfold analyzeWith
  generalize analyzeModule M = t
  rcases t with ⟨facts, effs, vals⟩
  exact ⟨rfl, rfl, rfl⟩

theorem analyzeModule_effects (M : Module) :
    (analyzeModule M).2.1 = M.rings.foldl (effectsStep (analyzeModule M).1) (List.replicate M.fns.length ⟨[], [], true⟩) :=
  rfl

/-- **With no verdicts, the analysis is `analyzeModule`**, which the theorems of `Total.lean` are
    of. -/
theorem analyzeWith_nil {M : Module} (h : M.verdicts = []) : analyzeWith M = analyzeModule M := by
  obtain ⟨h1, h2, h3⟩ := analyzeWith_parts M
  have hf : (analyzeWith M).1 = (analyzeModule M).1 := by rw [h1, h, map_decided_nil]
  have he : (analyzeWith M).2.1 = (analyzeModule M).2.1 := by rw [h2, hf, analyzeModule_effects]
  exact Prod.ext hf (Prod.ext he h3)

/-- **A function the effects inferred with the verdicts call total does not fail**, for any values
    its parameters take, where the facts with the verdicts taken are sound: what each verdict that
    holds says of its place is so (for an analysis that let no running value go, as `module_sound`
    is). With no verdicts that is `module_sound` itself (`analyzeWith_nil`). -/
theorem total_with_sound {C : Ctx}
    (hV : ∀ x ∈ C.M.rings, ∀ f ∈ x.1, f < C.M.fns.length → FnSound C f (((analyzeWith C.M).1)[f]?.getD default)) :
    ∀ f e, (analyzeWith C.M).2.1[f]? = some e → e.total = true → NoFail C f := by
  intro f e hf ht
  have hok := effects_ok (C := C) (analyzeWith C.M).1 C.M.rings (List.replicate C.M.fns.length ⟨[], [], true⟩) hV effOK_start
  rw [(analyzeWith_parts C.M).2.1] at hf
  exact hok.2.2 f e hf (total_clean.mp ht)

-- ── What a call of a rule keeps between its inputs ──────────────────────────

theorem mem_insertByName {x y : String × Expr} : ∀ {l : List (String × Expr)}, y ∈ insertByName x l → y = x ∨ y ∈ l
  | [], h => by simp only [insertByName, List.mem_singleton] at h; exact Or.inl h
  | z :: zs, h => by
    unfold insertByName at h
    split at h
    · simp only [List.mem_cons] at h
      rcases h with h | h | h
      · exact Or.inl h
      · exact Or.inr (by simp [h])
      · exact Or.inr (by simp [h])
    · simp only [List.mem_cons] at h
      rcases h with h | h
      · exact Or.inr (by simp [h])
      · rcases mem_insertByName h with h | h
        · exact Or.inl h
        · exact Or.inr (by simp [h])

/-- Every input `givenByName` names is given by the call, to the input of that name. -/
theorem givenByName_mem {rd : RuleD} {args : List (Nat × Expr × Option Conv)} {n : String} {x : Expr}
    (h : (n, x) ∈ givenByName rd args) : ∃ k c, (k, x, c) ∈ args ∧ (rd.inputs[k]?).map (·.name) = some n := by
  let P : List (String × Expr) → Prop := fun acc => ∀ y ∈ acc, ∃ k c, (k, y.2, c) ∈ args ∧ (rd.inputs[k]?).map (·.name) = some y.1
  have step : ∀ (l : List (Nat × Expr × Option Conv)) (acc : List (String × Expr)), (∀ a ∈ l, a ∈ args) → P acc →
      P (l.foldl (fun acc (a : Nat × Expr × Option Conv) => match rd.inputs[a.1]? with
        | some inp => insertByName (inp.name, a.2.1) acc
        | none => acc) acc) := by
    intro l
    induction l with
    | nil => intro acc _ hp; exact hp
    | cons a l ih =>
      intro acc hl hp
      simp only [List.foldl_cons]
      apply ih _ (fun b hb => hl b (by simp [hb]))
      split
      · rename_i inp hinp
        intro y hy
        rcases mem_insertByName hy with rfl | hy
        · exact ⟨a.1, a.2.2, by simpa using hl a (by simp), by simp [hinp]⟩
        · exact hp y hy
      · exact hp
  exact step args [] (fun a ha => ha) (fun y hy => by cases hy) (n, x) h

/-- A relation `relPairs` keeps is between two of the inputs, as `relation` says of their
    expressions. -/
theorem relPairs_mem {A : ACtx} : ∀ {l : List (String × Expr)} {na nb : String} {r : Rel},
    (na, r, nb) ∈ relPairs A l → ∃ xa xb, (na, xa) ∈ l ∧ (nb, xb) ∈ l ∧ relation A xa xb = some r
  | [], _, _, _, h => by simp [relPairs] at h
  | (n, x) :: rest, na, nb, r, h => by
    simp only [relPairs, List.mem_append, List.mem_filterMap] at h
    rcases h with ⟨⟨m, y⟩, hy, hm⟩ | h
    · simp only at hm
      split at hm
      · rename_i r' hr
        simp only [Option.some.injEq, Prod.mk.injEq] at hm
        obtain ⟨rfl, rfl, rfl⟩ := hm
        exact ⟨x, y, by simp, by simp [hy], hr⟩
      · rcases hr2 : relation A y x with _ | r'
        · rw [hr2] at hm; simp at hm
        · rw [hr2] at hm
          simp only [Option.map_some, Option.some.injEq, Prod.mk.injEq] at hm
          obtain ⟨rfl, rfl, rfl⟩ := hm
          exact ⟨y, x, by simp [hy], by simp, hr2⟩
    · obtain ⟨xa, xb, h1, h2, h3⟩ := relPairs_mem h
      exact ⟨xa, xb, by simp [h1], by simp [h2], h3⟩

theorem symOK_of {ρ : Env} {e : Expr} {s : Sym} {x : Val} (hs : sym e = some s) (hv : symVal ρ s = some x) :
    SymOK ρ e x where
  sym s' hs' := by rw [hs] at hs'; cases hs'; exact hv
  count l hl := by cases e <;> simp [sym, counted] at hs hl

theorem relation_syms {A : ACtx} {a b : Expr} {r : Rel} (h : relation A a b = some r) :
    ∃ sa sb, sym a = some sa ∧ sym b = some sb := by
  unfold relation at h
  split at h
  · rename_i sa sb ha hb; exact ⟨sa, sb, ha, hb⟩
  · simp at h

/-- **What a call of a rule keeps between its inputs holds between the values given** (what X2
    reads first, kaname's DESIGN 14.3): each relation is between two names given to the two inputs,
    and holds between their values in every environment the scope of the call holds. -/
theorem callRels_sound {A : ACtx} {ρ : Env} (h : Sat A ρ) {rd : RuleD} {args : List (Nat × Expr × Option Conv)}
    {na nb : String} {r : Rel} (hm : (na, r, nb) ∈ callRels A rd args) :
    ∃ xa xb sa sb, (∃ k c, (k, xa, c) ∈ args ∧ (rd.inputs[k]?).map (·.name) = some na) ∧
      (∃ k c, (k, xb, c) ∈ args ∧ (rd.inputs[k]?).map (·.name) = some nb) ∧
      sym xa = some sa ∧ sym xb = some sb ∧
      ∀ x y, symVal ρ sa = some x → symVal ρ sb = some y → RelSem r x y := by
  obtain ⟨xa, xb, ha, hb, hr⟩ := relPairs_mem hm
  obtain ⟨sa, sb, hsa, hsb⟩ := relation_syms hr
  exact ⟨xa, xb, sa, sb, givenByName_mem ha, givenByName_mem hb, hsa, hsb,
    fun x y hx hy => relation_sound h (symOK_of hsa hx) (symOK_of hsb hy) hr⟩

end KanameCore
