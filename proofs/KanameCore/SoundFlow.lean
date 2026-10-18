/-
  The forms that carry the evaluation on (kaname's DESIGN 15.2.2, 15.2.3): a block, a choice, a
  `match`, `and` and `or`, and a name given a value.
-/
import KanameCore.SoundStep

namespace KanameCore

-- ── What the analysis goes on with grows ────────────────────────────────────

theorem gEv {o₀ o : Out} {A : ACtx} {m : Nat} {e : Expr} (h : Grows o₀ o) : Grows o₀ (ev A m e o).out :=
  h.trans ((sat m).ev _ _ _).1
theorem gEvI {o₀ o : Out} {A : ACtx} {m : Nat} {e : Expr} (h : Grows o₀ o) : Grows o₀ (evI A m e o).out :=
  h.trans ((sat m).evI _ _ _).1
theorem gBranch {o₀ o : Out} {A : ACtx} {m : Nat} {b : Expr} {c : Option Conv} (h : Grows o₀ o) :
    Grows o₀ (branch A m b c o).2 := h.trans ((sat m).branch _ _ _ _)
theorem gArms {o₀ o : Out} {A : ACtx} {m : Nat} {sy : Option Sym} {sv : Abs}
    {arms : List (List Pat × Expr × Option Conv)} {out : Abs} (h : Grows o₀ o) :
    Grows o₀ (matchArms A m sy sv arms out o).2 := h.trans ((sat m).arms _ _ _ _ _ _)
theorem gArgs {o₀ o : Out} {A : ACtx} {m : Nat} {args : List (Nat × Expr × Option Conv)} (h : Grows o₀ o) :
    Grows o₀ (evArgs A m args o).2.1 := h.trans ((sat m).args _ _ _).1
theorem gArgs5 {o₀ o : Out} {A : ACtx} {m : Nat} {args : List (String × Expr × Option Conv × Nat × Bool)}
    (h : Grows o₀ o) : Grows o₀ (evArgs5 A m args o).2.1 := h.trans ((sat m).args5 _ _ _).1
theorem gInits {o₀ o : Out} {A : ACtx} {m : Nat} {inits : List (Nat × Expr × Option Conv × Nat)} (h : Grows o₀ o) :
    Grows o₀ (evInits A m inits o).2.1 := h.trans ((sat m).inits _ _ _).1
theorem gList {o₀ o : Out} {A : ACtx} {m : Nat} {args : List Expr} (h : Grows o₀ o) :
    Grows o₀ (evList A m args o).2.1 := h.trans ((sat m).list _ _ _).1
theorem gElems {o₀ o : Out} {A : ACtx} {m : Nat} {elems : List (Expr × Option Conv)} {elem : Abs} {k : Nat}
    (h : Grows o₀ o) : Grows o₀ (listAbs A m elems elem k o).out := h.trans ((sat m).elems _ _ _ _ _).1
theorem gRounds {o₀ o : Out} {A : ACtx} {m : Nat} {acc x : String} {body : Expr} {c : Option Conv} {elem : Abs}
    {k : Nat} {a : Abs} (h : Grows o₀ o) : Grows o₀ (reduceRounds A m acc x body c elem k a o).2 :=
  h.trans ((sat m).rounds _ _ _ _ _ _ _ _ _)
theorem gLam2 {o₀ o : Out} {A : ACtx} {m : Nat} {acc x : String} {a elem : Abs} {body : Expr} {c : Option Conv}
    (h : Grows o₀ o) : Grows o₀ (lam2 A m acc x a elem body c o).2 := h.trans ((sat m).lam2 _ _ _ _ _ _ _ _)

/-- Closes `Grows o X` where `X` is what the analysis goes on with, step by step back to `o`. -/
macro "gcl" : tactic => `(tactic| (repeat (first
  | exact Grows.refl _
  | with_reducible apply grows_record
  | with_reducible apply grows_fuel
  | with_reducible apply grows_unknown
  | with_reducible apply grows_heldA
  | with_reducible apply grows_convA
  | with_reducible apply grows_calls
  | with_reducible apply grows_widen
  | with_reducible apply callAbs_grows
  | with_reducible apply ruleAbs_grows
  | with_reducible apply dateAbs_grows
  | with_reducible apply transferAbs_ok
  | with_reducible apply recordAbs_grows
  | with_reducible apply grows_place
  | (show _ ∈ kindOrder; decide)
  | refine gEv ?_ | refine gEvI ?_ | refine gBranch ?_ | refine gArms ?_ | refine gArgs ?_ | refine gArgs5 ?_
  | refine gInits ?_ | refine gList ?_ | refine gElems ?_ | refine gRounds ?_ | refine gLam2 ?_)))

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

theorem fail_type {α : Type} {k : Kind} {line l : Nat} (h : (failK .type line : R α) = .error (.failed k l)) :
    k.ofEffect = false := by
  rw [failK_err h]; rfl

-- ── A block ──────────────────────────────────────────────────────────────────

/-- **A block that answers for an expression**: its answer read in the expression's unit. -/
theorem branch_sound (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {b : Expr} {c : Option Conv} {o : Out}
    {dep : Nat} (hG : Good C res A) (hS : Sat A ρ) (hw : (branch A m b c o).2.widened = false) :
    (∀ v, (eval C n dep ρ b >>= coerce c) = .ok v → Mem (branch A m b c o).1 v) ∧
    (∀ k l, (eval C n dep ρ b >>= coerce c) = .error (.failed k l) → k.ofEffect = true →
      Wit C (branch A m b c o).2) := by
  cases m with
  | zero => simp only [branch]; exact ⟨fun v _ => Mem.any v, fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    simp only [branch] at hw ⊢
    obtain ⟨h1, h2⟩ := IH.expr m A ρ b o dep hG hS (grows_not_widened (grows_convA (Grows.refl _)) hw)
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · obtain ⟨w, hw', hv⟩ := bind_ok hv
      exact convA_mem (h1 w hw').1 hv
    · rcases bind_err hv with hv | ⟨w, hw', hv⟩
      · exact Wit.grow (h2 k l hv hk) (grows_convA (Grows.refl _))
      · exact (coerce_wit (h1 w hw').1 hv).2

theorem case_block (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {b : Expr}
    {c : Option Conv} (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.block id b c) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.block id b c)) (evI A m (.block id b c) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, AR.of] at hw ⊢
    obtain ⟨h1, h2⟩ := branch_sound (dep := dep) IH hG hS hw
    refine ⟨fun v hv => ⟨h1 v ?_, hS, fun _ _ => trivial⟩, fun k l hv hk => h2 k l ?_ hk⟩
    · simp only [eval] at hv; exact hv
    · simp only [eval] at hv; exact hv

-- ── A choice ─────────────────────────────────────────────────────────────────

theorem ite_ok {dep id : Nat} {ρ : Env} {c t f : Expr} {ct cf : Option Conv} {v : Val}
    (h : eval C (n + 1) dep ρ (.ite id c t f ct cf) = .ok v) :
    ∃ go, eval C n dep ρ c = .ok (.bool go) ∧
      (eval C n dep ρ (if go then t else f) >>= coerce (if go then ct else cf)) = .ok v := by
  simp only [eval] at h
  obtain ⟨cv, hcv, h⟩ := bind_ok h
  obtain ⟨go, hgo, h⟩ := bind_ok h
  cases cv <;> simp [asBool, failK] at hgo
  subst hgo
  exact ⟨_, hcv, h⟩

theorem ite_err {dep id : Nat} {ρ : Env} {c t f : Expr} {ct cf : Option Conv} {k : Kind} {l : Nat}
    (h : eval C (n + 1) dep ρ (.ite id c t f ct cf) = .error (.failed k l)) (hk : k.ofEffect = true) :
    eval C n dep ρ c = .error (.failed k l) ∨ ∃ go, eval C n dep ρ c = .ok (.bool go) ∧
      (eval C n dep ρ (if go then t else f) >>= coerce (if go then ct else cf)) = .error (.failed k l) := by
  simp only [eval] at h
  rcases bind_err h with h | ⟨cv, hcv, h⟩
  · exact Or.inl h
  · rcases bind_err h with h | ⟨go, hgo, h⟩
    · rw [type_noEffect (asBool_err h)] at hk; cases hk
    · cases cv <;> simp [asBool, failK] at hgo
      subst hgo
      exact Or.inr ⟨_, hcv, h⟩

theorem case_ite (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {c t f : Expr}
    {ct cf : Option Conv} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.ite id c t f ct cf) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.ite id c t f ct cf)) (evI A m (.ite id c t f ct cf) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGT : Good C res ((ev A m c o).nar.apply true (ev A m c o).ctx) :=
      hG.same (same_apply _ _ _ ((sat m).ev A c o).2)
    have hGF : Good C res ((ev A m c o).nar.apply false (ev A m c o).ctx) :=
      hG.same (same_apply _ _ _ ((sat m).ev A c o).2)
    simp only [evI] at hw ⊢
    split at hw
    · -- the condition is never reached
      rename_i hnev
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ c o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · obtain ⟨go, hc, _⟩ := ite_ok hv
        have := (h1 _ hc).1; rw [hnev] at this; exact absurd this Mem.not_never
      · rcases ite_err hv hk with hc | ⟨go, hc, _⟩
        · exact h2 k l hc hk
        · have := (h1 _ hc).1; rw [hnev] at this; exact absurd this Mem.not_never
    · rename_i hne
      · simp only [AR.of] at hw ⊢
        cases hT : (truths (ev A m c o).abs).1 <;> cases hF : (truths (ev A m c o).abs).2 <;>
          simp only [hT, hF, Bool.false_eq_true, ite_true, ite_false] at hw ⊢ <;>
          have w0 : (ev A m c o).out.widened = false := grows_not_widened (by gcl) hw <;>
          obtain ⟨h1, h2⟩ := IH.expr m A ρ c o dep hG hS w0 <;>
          refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        -- it can neither hold nor fail: the evaluation does not get past it
        · obtain ⟨go, hc, _⟩ := ite_ok hv
          have hm := (h1 _ hc).1
          cases go
          · rw [truths_false hm] at hF; cases hF
          · rw [truths_true hm] at hT; cases hT
        · rcases ite_err hv hk with hc | ⟨go, hc, _⟩
          · exact h2 k l hc hk
          · have hm := (h1 _ hc).1
            cases go
            · rw [truths_false hm] at hF; cases hF
            · rw [truths_true hm] at hT; cases hT
        -- it can only fail
        · obtain ⟨go, hc, hb⟩ := ite_ok hv
          obtain ⟨hm, hs, hn⟩ := h1 _ hc
          cases go
          · simp only [Bool.false_eq_true, ite_false] at hb
            obtain ⟨b1, _⟩ := branch_sound (dep := dep) IH hGF (Sat.apply _ false hs (hn false rfl)) hw
            exact ⟨mem_join_right (a := Abs.never) (b1 v hb), hs, fun _ _ => trivial⟩
          · rw [truths_true hm] at hT; cases hT
        · rcases ite_err hv hk with hc | ⟨go, hc, hb⟩
          · exact Wit.grow (h2 k l hc hk) (by gcl)
          · obtain ⟨hm, hs, hn⟩ := h1 _ hc
            cases go
            · simp only [Bool.false_eq_true, ite_false] at hb
              exact (branch_sound (dep := dep) IH hGF (Sat.apply _ false hs (hn false rfl)) hw).2 k l hb hk
            · rw [truths_true hm] at hT; cases hT
        -- it can only hold
        · obtain ⟨go, hc, hb⟩ := ite_ok hv
          obtain ⟨hm, hs, hn⟩ := h1 _ hc
          cases go
          · rw [truths_false hm] at hF; cases hF
          · simp only [ite_true] at hb
            obtain ⟨b1, _⟩ := branch_sound (dep := dep) IH hGT (Sat.apply _ true hs (hn true rfl)) hw
            exact ⟨mem_join_right (a := Abs.never) (b1 v hb), hs, fun _ _ => trivial⟩
        · rcases ite_err hv hk with hc | ⟨go, hc, hb⟩
          · exact Wit.grow (h2 k l hc hk) (by gcl)
          · obtain ⟨hm, hs, hn⟩ := h1 _ hc
            cases go
            · rw [truths_false hm] at hF; cases hF
            · simp only [ite_true] at hb
              exact (branch_sound (dep := dep) IH hGT (Sat.apply _ true hs (hn true rfl)) hw).2 k l hb hk
        -- either
        · obtain ⟨go, hc, hb⟩ := ite_ok hv
          obtain ⟨hm, hs, hn⟩ := h1 _ hc
          cases go
          · simp only [Bool.false_eq_true, ite_false] at hb
            obtain ⟨b1, _⟩ := branch_sound (dep := dep) IH hGF (Sat.apply _ false hs (hn false rfl)) hw
            exact ⟨mem_join_right (a := Abs.never.join _) (b1 v hb), hs, fun _ _ => trivial⟩
          · simp only [ite_true] at hb
            obtain ⟨b1, _⟩ := branch_sound (dep := dep) IH hGT (Sat.apply _ true hs (hn true rfl))
              (grows_not_widened (gBranch (Grows.refl _)) hw)
            exact ⟨mem_join_left (mem_join_right (b1 v hb)), hs, fun _ _ => trivial⟩
        · rcases ite_err hv hk with hc | ⟨go, hc, hb⟩
          · exact Wit.grow (h2 k l hc hk) (by gcl)
          · obtain ⟨hm, hs, hn⟩ := h1 _ hc
            cases go
            · simp only [Bool.false_eq_true, ite_false] at hb
              exact (branch_sound (dep := dep) IH hGF (Sat.apply _ false hs (hn false rfl)) hw).2 k l hb hk
            · simp only [ite_true] at hb
              exact Wit.grow ((branch_sound (dep := dep) IH hGT (Sat.apply _ true hs (hn true rfl))
                (grows_not_widened (gBranch (Grows.refl _)) hw)).2 k l hb hk) (gBranch (Grows.refl _))

-- ── A match ──────────────────────────────────────────────────────────────────

theorem same_armFold {A₀ : ACtx} {sy : Option Sym} {sv : Abs} :
    ∀ (ps : List Pat) (acc : ACtx × Bool), Same A₀ acc.1 → Same A₀ (ps.foldl (armStep sy sv) acc).1
  | [], _, h => h
  | p :: ps, acc, h => by
    simp only [List.foldl_cons]
    apply same_armFold ps
    unfold armStep
    repeat' split
    all_goals first | exact h | exact same_narrowTo h | exact same_bindNew h

theorem same_armScope {A : ACtx} {sy : Option Sym} {sv : Abs} {ps : List Pat} : Same A (armScope A sy sv ps).1 := by
  have h := same_armFold (A₀ := A) (sy := sy) (sv := sv) ps (A, false) (Same.refl A)
  unfold armScope
  simp only
  repeat' split
  all_goals first | exact h | exact same_narrowTo h

/-- **The arms of a `match`**: the arm that takes the value is reached, its answer among what the
    arms answer; what the arms answer only grows. -/
theorem arms_sound (IH : SoundAt C res n) {ρ : Env} {dep : Nat} {sy : Option Sym} {sv : Abs} {v : Val}
    (hm : Mem sv v) (hsy : ∀ s, sy = some s → symVal ρ s = some v) :
    ∀ (arms : List (List Pat × Expr × Option Conv)) (m : Nat) (A : ACtx) (out : Abs) (o : Out),
    Good C res A → Sat A ρ → (matchArms A m sy sv arms out o).2.widened = false →
    (∀ x, Mem out x → Mem (matchArms A m sy sv arms out o).1 x) ∧
    (∀ body c bnd, pick v arms = some (body, c, bnd) →
      (∀ r, (eval C n dep (match bnd with | some x => (x, v) :: ρ | none => ρ) body >>= coerce c) = .ok r →
        Mem (matchArms A m sy sv arms out o).1 r) ∧
      (∀ k l, (eval C n dep (match bnd with | some x => (x, v) :: ρ | none => ρ) body >>= coerce c) =
        .error (.failed k l) → k.ofEffect = true → Wit C (matchArms A m sy sv arms out o).2))
  | [], m, A, out, o, _, _, _ => by
    cases m with
    | zero => simp only [matchArms]; exact ⟨fun x h => mem_join_left h, fun _ _ _ h => by simp [pick] at h⟩
    | succ m => simp only [matchArms]; exact ⟨fun x h => h, fun _ _ _ h => by simp [pick] at h⟩
  | (ps, body, c) :: rest, m, A, out, o, hG, hS, hw => by
    cases m with
    | zero =>
      simp only [matchArms]
      exact ⟨fun x h => mem_join_left h, fun _ _ _ _ => ⟨fun r _ => mem_join_right (Mem.any r), fun _ _ _ _ => wit_fuel⟩⟩
    | succ m =>
      have hGA : Good C res (armScope A sy sv ps).1 := hG.same same_armScope
      simp only [matchArms] at hw ⊢
      cases hr : (armScope A sy sv ps).2 <;> simp only [hr, Bool.false_eq_true, ite_true, ite_false] at hw ⊢
      · -- nothing reaches the arm
        obtain ⟨p1, p2⟩ := arms_sound IH hm hsy rest m A out o hG hS hw
        refine ⟨p1, fun body' c' bnd hp => ?_⟩
        simp only [pick] at hp
        split at hp
        · cases hp
        · rename_i hok
          split at hp
          · rename_i b hb
            have := (armScope_sound hS (by simpa using hok) hb hm hsy).1
            rw [hr] at this; cases this
          · exact p2 body' c' bnd hp
      · -- the arm is reached
        obtain ⟨p1, p2⟩ := arms_sound IH hm hsy rest m A _ _ hG hS hw
        refine ⟨fun x hx => p1 x (mem_join_left hx), fun body' c' bnd hp => ?_⟩
        simp only [pick] at hp
        split at hp
        · cases hp
        · rename_i hok
          split at hp
          · rename_i b hb
            simp only [Option.some.injEq, Prod.mk.injEq] at hp
            obtain ⟨rfl, rfl, rfl⟩ := hp
            have hsc := (armScope_sound hS (by simpa using hok) hb hm hsy).2
            have hwb : (branch (armScope A sy sv ps).1 m body c o).2.widened = false :=
              grows_not_widened (gArms (Grows.refl _)) hw
            refine ⟨fun r hr' => p1 r (mem_join_right ?_), fun k l he hk => Wit.grow ?_ (gArms (Grows.refl _))⟩
            · cases b <;> exact (branch_sound (dep := dep) IH hGA hsc hwb).1 r hr'
            · cases b <;> exact (branch_sound (dep := dep) IH hGA hsc hwb).2 k l he hk
          · exact p2 body' c' bnd hp

theorem case_mtch (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {s : Expr}
    {arms : List (List Pat × Expr × Option Conv)} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.mtch id line s arms) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.mtch id line s arms)) (evI A m (.mtch id line s arms) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGs : Good C res (ev A m s o).ctx := hG.same ((sat m).ev A s o).2
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hnev
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ s o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨sv, hsv, _⟩ := bind_ok hv
        have := (h1 sv hsv).1; rw [hnev] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨sv, hsv, _⟩
        · exact h2 k l hv hk
        · have := (h1 sv hsv).1; rw [hnev] at this; exact absurd this Mem.not_never
    · rename_i hne
      · simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ s o dep hG hS (grows_not_widened (gArms (Grows.refl _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨sv, hsv, hv⟩ := bind_ok hv
          obtain ⟨hm, hs, _⟩ := h1 sv hsv
          have hsy : ∀ s', sym s = some s' → symVal ρ s' = some sv := fun s' h => sym_eval h hsv
          obtain ⟨_, p2⟩ := arms_sound (dep := dep) IH hm hsy arms m _ Abs.never _ hGs hs hw
          split at hv
          · rename_i body c bnd hp
            refine ⟨?_, hs, fun _ _ => trivial⟩
            cases bnd <;> exact (p2 body c _ hp).1 v hv
          · exact absurd hv failK_ne_ok
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨sv, hsv, hv⟩
          · exact Wit.grow (h2 k l hv hk) (gArms (Grows.refl _))
          · obtain ⟨hm, hs, _⟩ := h1 sv hsv
            have hsy : ∀ s', sym s = some s' → symVal ρ s' = some sv := fun s' h => sym_eval h hsv
            obtain ⟨_, p2⟩ := arms_sound (dep := dep) IH hm hsy arms m _ Abs.never _ hGs hs hw
            split at hv
            · rename_i body c bnd hp
              cases bnd <;> exact (p2 body c _ hp).2 k l hv hk
            · rw [fail_type hv] at hk; cases hk

-- ── A name given a value ─────────────────────────────────────────────────────

theorem case_letIn (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep : Nat} {x : String}
    {v : Expr} {c : Option Conv} {body : Expr} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (ev A m (.letIn x v c body) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.letIn x v c body)) (ev A m (.letIn x v c body) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGv : Good C res (ev A m v o).ctx := hG.same ((sat m).ev A v o).2
    simp only [ev] at hw ⊢
    split at hw
    · rename_i hnev
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ v o dep hG hS hw
      refine ⟨fun w hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨a, ha, _⟩ := bind_ok hv
        have := (h1 a ha).1; rw [hnev] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨a, ha, _⟩
        · exact h2 k l hv hk
        · have := (h1 a ha).1; rw [hnev] at this; exact absurd this Mem.not_never
    · rename_i hne
      · simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ v o dep hG hS (grows_not_widened (gEv (grows_convA (Grows.refl _))) hw)
        refine ⟨fun w hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨a, ha, hv⟩ := bind_ok hv
          obtain ⟨a', ha', hv⟩ := bind_ok hv
          obtain ⟨hm, hs, _⟩ := h1 a ha
          have hS' := hs.bindNew (x := x) (convA_mem (o := (ev A m v o).out) hm ha')
          obtain ⟨hb1, _⟩ := IH.expr m ((ev A m v o).ctx.bindNew x _) _ body _ dep hGv hS' hw
          exact ⟨(hb1 w hv).1, hs, fun _ _ => trivial⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨a, ha, hv⟩
          · exact Wit.grow (h2 k l hv hk) (gEv (grows_convA (Grows.refl _)))
          · obtain ⟨hm, hs, _⟩ := h1 a ha
            rcases bind_err hv with hv | ⟨a', ha', hv⟩
            · exact Wit.grow (coerce_wit hm hv).2 (gEv (Grows.refl _))
            · have hS' := hs.bindNew (x := x) (convA_mem (o := (ev A m v o).out) hm ha')
              exact (IH.expr m ((ev A m v o).ctx.bindNew x _) _ body _ dep hGv hS' hw).2 k l hv hk

-- ── `and` and `or` ───────────────────────────────────────────────────────────

theorem bool_of_asBool {v : Val} {line : Nat} {b : Bool} (h : asBool v line = .ok b) : v = .bool b := by
  cases v <;> simp [asBool, failK] at h
  rw [h]

theorem case_and (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {a b : Expr}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.and id line a b) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.and id line a b)) (evI A m (.and id line a b) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    cases m with
    | zero => simp only [andOr]; exact concl_fuel hS
    | succ m =>
      have hGb : Good C res ((ev A m a o).nar.apply true (ev A m a o).ctx) :=
        hG.same (same_apply _ _ _ ((sat m).ev A a o).2)
      simp only [andOr] at hw ⊢
      cases hT : (truths (ev A m a o).abs).1 <;> simp only [hT, ite_true, ite_false, Bool.false_eq_true] at hw ⊢
      · -- the left cannot hold: the right is not evaluated
        obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS hw
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨lv, hlv, hv1⟩ := bind_ok hv
          obtain ⟨lb, hlb, hv2⟩ := bind_ok hv1
          obtain rfl := bool_of_asBool hlb
          obtain ⟨hm, hs, _⟩ := h1 _ hlv
          cases lb
          · simp at hv2; subst hv2
            exact ⟨Mem_bools (by simp) (fun _ => by simp [truths_false hm]), hs, fun t ht => by cases ht; intro h; cases h⟩
          · rw [truths_true hm] at hT; cases hT
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨lv, hlv, hv1⟩
          · exact h2 k l hv hk
          · rcases bind_err hv1 with hv2 | ⟨lb, hlb, hv2⟩
            · rw [type_noEffect (asBool_err hv2)] at hk; cases hk
            · obtain rfl := bool_of_asBool hlb
              obtain ⟨hm, _, _⟩ := h1 _ hlv
              cases lb
              · simp at hv2
              · rw [truths_true hm] at hT; cases hT
      · obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened (gEv (Grows.refl _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨lv, hlv, hv1⟩ := bind_ok hv
          obtain ⟨lb, hlb, hv2⟩ := bind_ok hv1
          obtain rfl := bool_of_asBool hlb
          obtain ⟨hm, hs, hn⟩ := h1 _ hlv
          cases lb
          · simp at hv2; subst hv2
            exact ⟨Mem_bools (by simp) (fun _ => by simp [truths_false hm]), hs, fun t ht => by cases ht; intro h; cases h⟩
          · simp only [Bool.not_true, Bool.false_eq_true, ite_false] at hv2
            obtain ⟨rv, hrv, hv3⟩ := bind_ok hv2
            obtain ⟨rb, hrb, hv4⟩ := bind_ok hv3
            cases hv4
            obtain rfl := bool_of_asBool hrb
            obtain ⟨hm2, _, hn2⟩ := (IH.expr m _ ρ b _ dep hGb (Sat.apply _ true hs (hn true rfl)) hw).1 _ hrv
            cases rb
            · exact ⟨Mem_bools (by simp) (fun _ => by simp [truths_false hm2]), hs,
                fun t ht => by cases ht; intro h; cases h⟩
            · exact ⟨Mem_bools (fun _ => by simp [truths_true hm2]) (by simp), hs,
                fun t ht => by cases ht; intro _; exact ⟨hn true rfl, hn2 true rfl⟩⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨lv, hlv, hv1⟩
          · exact Wit.grow (h2 k l hv hk) (gEv (Grows.refl _))
          · rcases bind_err hv1 with hv2 | ⟨lb, hlb, hv2⟩
            · rw [type_noEffect (asBool_err hv2)] at hk; cases hk
            · obtain rfl := bool_of_asBool hlb
              obtain ⟨hm, hs, hn⟩ := h1 _ hlv
              cases lb
              · simp at hv2
              · simp only [Bool.not_true, Bool.false_eq_true, ite_false] at hv2
                rcases bind_err hv2 with hv | ⟨rv, hrv, hv3⟩
                · exact (IH.expr m _ ρ b _ dep hGb (Sat.apply _ true hs (hn true rfl)) hw).2 k l hv hk
                · rcases bind_err hv3 with hv4 | ⟨rb, _, hv4⟩
                  · rw [type_noEffect (asBool_err hv4)] at hk; cases hk
                  · cases hv4

theorem case_or (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {a b : Expr}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.or id line a b) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.or id line a b)) (evI A m (.or id line a b) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    cases m with
    | zero => simp only [andOr]; exact concl_fuel hS
    | succ m =>
      have hGb : Good C res ((ev A m a o).nar.apply false (ev A m a o).ctx) :=
        hG.same (same_apply _ _ _ ((sat m).ev A a o).2)
      simp only [andOr] at hw ⊢
      cases hF : (truths (ev A m a o).abs).2 <;> simp only [hF, ite_true, ite_false, Bool.false_eq_true] at hw ⊢
      · -- the left cannot fail: the right is not evaluated
        obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS hw
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨lv, hlv, hv1⟩ := bind_ok hv
          obtain ⟨lb, hlb, hv2⟩ := bind_ok hv1
          obtain rfl := bool_of_asBool hlb
          obtain ⟨hm, hs, _⟩ := h1 _ hlv
          cases lb
          · rw [truths_false hm] at hF; cases hF
          · simp at hv2; subst hv2
            exact ⟨Mem_bools (fun _ => by simp [truths_true hm]) (by simp), hs, fun t ht => by cases ht; intro h; cases h⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨lv, hlv, hv1⟩
          · exact h2 k l hv hk
          · rcases bind_err hv1 with hv2 | ⟨lb, hlb, hv2⟩
            · rw [type_noEffect (asBool_err hv2)] at hk; cases hk
            · obtain rfl := bool_of_asBool hlb
              obtain ⟨hm, _, _⟩ := h1 _ hlv
              cases lb
              · rw [truths_false hm] at hF; cases hF
              · simp at hv2
      · obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened (gEv (Grows.refl _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨lv, hlv, hv1⟩ := bind_ok hv
          obtain ⟨lb, hlb, hv2⟩ := bind_ok hv1
          obtain rfl := bool_of_asBool hlb
          obtain ⟨hm, hs, hn⟩ := h1 _ hlv
          cases lb
          · simp only [Bool.false_eq_true, ite_false] at hv2
            obtain ⟨rv, hrv, hv3⟩ := bind_ok hv2
            obtain ⟨rb, hrb, hv4⟩ := bind_ok hv3
            cases hv4
            obtain rfl := bool_of_asBool hrb
            obtain ⟨hm2, _, hn2⟩ := (IH.expr m _ ρ b _ dep hGb (Sat.apply _ false hs (hn false rfl)) hw).1 _ hrv
            cases rb
            · exact ⟨Mem_bools (by simp) (fun _ => by simp [truths_false hm2]), hs,
                fun t ht => by cases ht; intro _; exact ⟨hn false rfl, hn2 false rfl⟩⟩
            · exact ⟨Mem_bools (fun _ => by simp [truths_true hm2]) (by simp), hs,
                fun t ht => by cases ht; intro h; cases h⟩
          · simp at hv2; subst hv2
            exact ⟨Mem_bools (fun _ => by simp [truths_true hm]) (by simp), hs, fun t ht => by cases ht; intro h; cases h⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨lv, hlv, hv1⟩
          · exact Wit.grow (h2 k l hv hk) (gEv (Grows.refl _))
          · rcases bind_err hv1 with hv2 | ⟨lb, hlb, hv2⟩
            · rw [type_noEffect (asBool_err hv2)] at hk; cases hk
            · obtain rfl := bool_of_asBool hlb
              obtain ⟨hm, hs, hn⟩ := h1 _ hlv
              cases lb
              · simp only [Bool.false_eq_true, ite_false] at hv2
                rcases bind_err hv2 with hv | ⟨rv, hrv, hv3⟩
                · exact (IH.expr m _ ρ b _ dep hGb (Sat.apply _ false hs (hn false rfl)) hw).2 k l hv hk
                · rcases bind_err hv3 with hv4 | ⟨rb, _, hv4⟩
                  · rw [type_noEffect (asBool_err hv4)] at hk; cases hk
                  · cases hv4
              · simp at hv2

end

end KanameCore
