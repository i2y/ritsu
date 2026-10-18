/-
  One step of the evaluation, as the analysis says it (kaname's DESIGN 15.2.2, 15.2.3): for each
  form of expression, where its parts keep what their analysis says (`SoundAt` at one fewer step),
  so does the expression.
-/
import KanameCore.SoundWalk

namespace KanameCore

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

-- ── Literals and names ───────────────────────────────────────────────────────

theorem concl_of {ρ : Env} {res' : R Val} {a : Abs} {o : Out} {A : ACtx} (hS : Sat A ρ)
    (hv : ∀ v, res' = .ok v → Mem a v) (hf : ∀ k l, res' = .error (.failed k l) → k.ofEffect = true → Wit C o) :
    Concl C ρ res' (AR.of a o A) :=
  ⟨fun v h => ⟨hv v h, hS, fun _ _ => trivial⟩, hf⟩

theorem case_num {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {q : Rat} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.num id line q)) (evI A m (.num id line q) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, eval]
    refine concl_of hS (fun v hv => ?_) (fun k l hv hk => ?_)
    · obtain ⟨r, hr, rfl⟩ := map_ok hv
      obtain ⟨rfl, _⟩ := held_ok (liftA_ok hr)
      exact Mem.num (In_exact _)
    · have := map_err hv
      rcases liftA_err this with ⟨h1, _⟩ | ⟨h1, _⟩
      · have hf := (held_err h1).2
        simp only [hf, Bool.false_eq_true, ite_false]
        exact wit_place (Mem.num (In_exact q)) ⟨q, rfl, hf⟩
      · simp [held] at h1; split at h1 <;> simp at h1

theorem case_date {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {d : Int} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.date id d)) (evI A m (.date id d) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, eval]
    exact concl_of hS (fun v hv => by cases hv; exact Mem.date (In_ints (Int.le_refl d) (Int.le_refl d)))
      (fun k l hv => by cases hv)

theorem case_str {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {t : String} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.str id t)) (evI A m (.str id t) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, eval]
    exact concl_of hS (fun v hv => by cases hv; exact Mem.str) (fun k l hv => by cases hv)

theorem case_bool {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {b : Bool} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.bool id b)) (evI A m (.bool id b) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, eval]
    exact concl_of hS (fun v hv => by cases hv; exact Mem_boolAbs b) (fun k l hv => by cases hv)

theorem case_none {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.none id)) (evI A m (.none id) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, eval]
    exact concl_of hS (fun v hv => by cases hv; exact Mem_none') (fun k l hv => by cases hv)

theorem case_enum {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {e : String} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.enum id e)) (evI A m (.enum id e) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, eval]
    exact concl_of hS (fun v hv => by cases hv; exact Mem.enum (by simp)) (fun k l hv => by cases hv)

theorem case_var {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {x : String} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.var id line x)) (evI A m (.var id line x) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI]
    refine ⟨fun v hv => ⟨?_, hS, ?_⟩, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      split at hv
      · rename_i w hw
        cases hv
        rcases hl : A.lookup x with _ | a
        · exact Mem.any _
        · obtain ⟨w', hw', hm⟩ := hS.env x a hl
          rw [hw] at hw'; cases hw'
          exact hm
      · simp [failK] at hv
    · intro t ht
      subst ht
      simp only [NarHolds]
      exact sym_eval (e := .var id line x) rfl hv
    · simp only [eval] at hv
      split at hv
      · cases hv
      · have := failK_err hv; subst this; simp [Kind.ofEffect] at hk


theorem type_noEffect {k : Kind} {l line : Nat} (h : Stop.failed k l = Stop.failed .type line) :
    k.ofEffect = false := by
  simp only [Stop.failed.injEq] at h; rw [h.1]; rfl

theorem wit_unknown {o : Out} {e : Nat} : Wit C (unknown o e) :=
  wit_place (what := .pending) (c := .none) (Mem.any _) trivial

theorem case_field (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {base : Expr} {f : String} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.field id line base f) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.field id line base f)) (evI A m (.field id line base f) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    obtain ⟨h1, h2⟩ := IH.expr m A ρ base o dep hG hS hw
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · have hv' := hv
      simp only [eval] at hv
      obtain ⟨b, hb, hv⟩ := bind_ok hv
      split at hv
      · rename_i w hw'
        cases hv
        obtain ⟨hm, hs, _⟩ := h1 b hb
        refine ⟨Mem_field hm hw', hs, ?_⟩
        intro t ht
        subst ht
        split
        · rename_i s hs'
          exact sym_eval hs' hv'
        · trivial
      · simp [failK] at hv
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨b, hb, hv⟩
      · exact h2 k l hv hk
      · split at hv
        · cases hv
        · have := failK_err hv; subst this; simp [Kind.ofEffect] at hk

theorem case_neg (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {x : Expr} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.neg id line x) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.neg id line x)) (evI A m (.neg id line x) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    have hgr : Grows (ev A m x o).out
        (match (ev A m x o).abs with
          | .num i => AR.of (heldA (ev A m x o).out id (.num i.negated)).1 (heldA (ev A m x o).out id (.num i.negated)).2 (ev A m x o).ctx
          | .never => AR.of .never (ev A m x o).out (ev A m x o).ctx
          | _ => AR.of .any (unknown (ev A m x o).out id) (ev A m x o).ctx).out := by
      split
      · exact grows_heldA (Grows.refl _)
      · exact Grows.refl _
      · exact grows_unknown (Grows.refl _)
    obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened hgr hw)
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨w, hwv, hv⟩ := bind_ok hv
      obtain ⟨hm, hs, _⟩ := h1 w hwv
      split <;> simp only [AR.of]
      · rename_i i hi
        rw [hi] at hm
        cases hm with
        | @num _ r hr =>
          simp only at hv
          obtain ⟨r', hr', rfl⟩ := map_ok hv
          have := liftA_ok hr'
          simp only [negN] at this
          obtain ⟨rfl, hf⟩ := held_ok this
          exact ⟨heldA_mem (Mem.num (In_negated hr)) hf, hs, fun _ h => by cases h⟩
      · rename_i hi; rw [hi] at hm; exact absurd hm Mem.not_never
      · exact ⟨Mem.any v, hs, fun _ _ => trivial⟩
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨w, hwv, hv⟩
      · exact Wit.grow (h2 k l hv hk) hgr
      · obtain ⟨hm, _, _⟩ := h1 w hwv
        cases w with
        | num r =>
          simp only at hv
          have := map_err hv
          rcases liftA_err this with ⟨h3, _⟩ | ⟨h3, _⟩
          · simp only [negN] at h3
            have hf := (held_err h3).2
            split <;> simp only [AR.of]
            · rename_i i hi
              rw [hi] at hm
              cases hm with
              | num hr => exact heldA_wit (Mem.num (In_negated hr)) hf
            · rename_i hi; rw [hi] at hm; exact absurd hm Mem.not_never
            · exact wit_unknown
          · simp [negN, held] at h3; split at h3 <;> simp at h3
        | _ => simp at hv

theorem case_not (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {x : Expr} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.not id line x) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.not id line x)) (evI A m (.not id line x) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨w, hwv, hv⟩ := bind_ok hv
      obtain ⟨b, hb, hv⟩ := bind_ok hv
      cases hv
      cases w <;> simp [asBool, failK] at hb
      subst hb
      obtain ⟨hm, hs, hn⟩ := h1 _ hwv
      refine ⟨?_, hs, ?_⟩
      · split
        · rename_i s hsa
          rw [hsa] at hm
          exact Mem_not hm
        · exact Mem.bool (by split <;> simp)
      · intro t ht
        cases ht
        simp only [NarHolds, Bool.not_not]
        exact hn _ rfl
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨w, _, hv⟩
      · exact h2 k l hv hk
      · rcases bind_err hv with hv | ⟨b, _, hv⟩
        · rw [type_noEffect (asBool_err hv)] at hk; simp at hk
        · cases hv

theorem case_inUnit (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {x : Expr} {c : Option Conv} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.inUnit id line x c) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.inUnit id line x c)) (evI A m (.inUnit id line x c) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    have hgr : Grows (convA (ev A m x o).out c (ev A m x o).abs).2
        (match (convA (ev A m x o).out c (ev A m x o).abs).1 with
          | .num _ | .any | .opt _ _ => AR.of (heldA (convA (ev A m x o).out c (ev A m x o).abs).2 id
              (convA (ev A m x o).out c (ev A m x o).abs).1).1 (heldA (convA (ev A m x o).out c (ev A m x o).abs).2 id
              (convA (ev A m x o).out c (ev A m x o).abs).1).2 (ev A m x o).ctx
          | v => AR.of v (convA (ev A m x o).out c (ev A m x o).abs).2 (ev A m x o).ctx).out := by
      split <;> first | exact grows_heldA (Grows.refl _) | exact Grows.refl _
    have hgr0 : Grows (ev A m x o).out (convA (ev A m x o).out c (ev A m x o).abs).2 := grows_convA (Grows.refl _)
    obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (hgr0.trans hgr) hw)
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨w, hwv, hv⟩ := bind_ok hv
      obtain ⟨w', hcw, hv⟩ := bind_ok hv
      obtain ⟨hm, hs, _⟩ := h1 w hwv
      have hm' := convA_mem (o := (ev A m x o).out) hm hcw
      -- the value held to int64
      have hfit : ∀ r, w' = .num r → fits r = true ∧ v = .num r := by
        intro r hr; subst hr
        simp only [heldV] at hv
        obtain ⟨r', hr', rfl⟩ := map_ok hv
        obtain ⟨rfl, hf⟩ := held_ok (liftA_ok hr')
        exact ⟨hf, rfl⟩
      have hvw : v = w' := by
        cases w' with
        | num r => exact (hfit r rfl).2
        | _ => simpa [heldV] using hv.symm
      subst hvw
      split <;> simp only [AR.of]
      · rename_i i hi
        cases v with
        | num r => exact ⟨heldA_mem hm' (hfit _ rfl).1, hs, fun _ _ => trivial⟩
        | _ => rw [hi] at hm'; cases hm'
      · rename_i hi
        rw [hi]; simp only [heldA, heldAbs]; exact ⟨Mem.any _, hs, fun _ _ => trivial⟩
      · rename_i nn y hi
        rw [hi] at hm' ⊢; simp only [heldA, heldAbs]; exact ⟨hm', hs, fun _ _ => trivial⟩
      · exact ⟨hm', hs, fun _ _ => trivial⟩
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨w, hwv, hv⟩
      · exact Wit.grow (h2 k l hv hk) (hgr0.trans hgr)
      · obtain ⟨hm, _, _⟩ := h1 w hwv
        rcases bind_err hv with hv | ⟨w', hcw, hv⟩
        · obtain ⟨_, hw'⟩ := coerce_wit (o := (ev A m x o).out) hm hv
          exact Wit.grow hw' hgr
        · -- the value read leaves int64
          have hm' := convA_mem (o := (ev A m x o).out) hm hcw
          cases w' with
          | num r =>
            simp only [heldV] at hv
            have := map_err hv
            rcases liftA_err this with ⟨h3, _⟩ | ⟨h3, _⟩
            · have hf := (held_err h3).2
              split <;> simp only [AR.of]
              · exact heldA_wit hm' hf
              · exact heldA_wit hm' hf
              · exact heldA_wit hm' hf
              · rename_i v' h1' h2' h3'
                exfalso
                generalize (convA (ev A m x o).out c (ev A m x o).abs).1 = a at hm' h1' h2' h3'
                cases hm' with
                | num _ => exact h1' _ rfl
                | any => exact h2' rfl
                | optSome _ _ => exact h3' _ _ rfl
            · simp [held] at h3; split at h3 <;> simp at h3
          | _ => simp [heldV] at hv

end

end KanameCore
