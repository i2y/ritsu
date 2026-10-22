/-
  Calls and the builtins that do no arithmetic (kaname's DESIGN 15.2.2, 15.2.3): a function of the
  module, a rule, a date, a transfer and a record built, a list written out, `count`,
  `starts_with` (`concat` and `text` are `SoundText.lean`'s).
-/
import KanameCore.SoundReread

namespace KanameCore

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

theorem Mem_allBools (b : Bool) : Mem Abs.bools (.bool b) := Mem.bool (by cases b <;> simp)

-- ── Calls ────────────────────────────────────────────────────────────────────

theorem case_call (IH : SoundAt C res n) (IHs : ∀ k, k < n → SoundAt C res k) (hR : ResultsOK C res) {m : Nat}
    {A : ACtx} {ρ : Env} {o : Out} {dep id line f : Nat} {args : List (Nat × Expr × Option Conv)} (hG : Good C res A)
    (hS : Sat A ρ) (hw : (evI A m (.call id line f args) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.call id line f args)) (evI A m (.call id line f args) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGa : Good C res (evArgs A m args o).2.2 := hG.same ((sat m).args A args o).2
    simp only [evI] at hw ⊢
    split at hw
    · -- an argument never reached
      rename_i hnone
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.args m A ρ args o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, _⟩ := bind_ok hv
        obtain ⟨avs, havs, _⟩ := a1 vals hvals
        rw [hnone] at havs; cases havs
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, _⟩
        · exact a2 k l hv hk
        · obtain ⟨avs, havs, _⟩ := a1 vals hvals
          rw [hnone] at havs; cases havs
    · rename_i avs hsome
      split at hw
      · -- a generic function: its body read again with the values the call gives (DESIGN 26.5)
        rename_i fn hre
        simp only [AR.of] at hw ⊢
        have hgrow : Grows (evArgs A m args o).2.1 (rereadEnd fn ((args.zip avs).foldl (callStep fn) ((evArgs A m args o).2.1, (evArgs A m args o).2.2)).1
            (ev (rereadCtx ((args.zip avs).foldl (callStep fn) ((evArgs A m args o).2.1, (evArgs A m args o).2.2)).2 fn f args avs) m fn.body
              ((args.zip avs).foldl (callStep fn) ((evArgs A m args o).2.1, (evArgs A m args o).2.2)).1)).2 :=
          rereadEnd_grows ((callFold_grows (Grows.refl _)).trans ((sat m).ev _ _ _).1)
        obtain ⟨a1, a2⟩ := IH.args m A ρ args o dep hG hS (grows_not_widened hgrow hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
          obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
          rw [hsome] at havs; cases havs
          split at hv1
          · exact absurd hv1 failK_ne_ok
          · obtain ⟨c1, _⟩ := reread_case (line := line) (dep := dep) IHs hre hGa hs hall hsyms hw
            exact ⟨(c1 v hv1).1, (c1 v hv1).2, fun _ _ => trivial⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
          · exact Wit.grow (a2 k l hv hk) hgrow
          · obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
            rw [hsome] at havs; cases havs
            split at hv1
            · rw [fail_type hv1] at hk; cases hk
            · exact (reread_case (line := line) (dep := dep) IHs hre hGa hs hall hsyms hw).2 k l hv1 hk
      · simp only [AR.of] at hw ⊢
        obtain ⟨a1, a2⟩ := IH.args m A ρ args o dep hG hS
          (grows_not_widened ((callAbs_grows (Grows.refl _)).trans ((sat m).once _ _ _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
          obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
          rw [hsome] at havs; cases havs
          split at hv1
          · exact absurd hv1 failK_ne_ok
          · rename_i fn0 hfn0
            obtain ⟨c1, _⟩ := call_case (o := (evArgs A m args o).2.1) (line := line) (n := n) (dep := dep) hR hfn0
              hGa hs hall hsyms
            exact ⟨(c1 v hv1).1, (c1 v hv1).2, fun _ _ => trivial⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
          · exact Wit.grow (a2 k l hv hk) ((callAbs_grows (Grows.refl _)).trans ((sat m).once _ _ _))
          · obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
            rw [hsome] at havs; cases havs
            split at hv1
            · rw [fail_type hv1] at hk; cases hk
            · rename_i fn0 hfn0
              exact Wit.grow ((call_case (o := (evArgs A m args o).2.1) (line := line) (n := n) (dep := dep) hR hfn0
                hGa hs hall hsyms).2 k l hv1 hk) ((sat m).once _ _ _)

theorem case_rule (IH : SoundAt C res n) (hO : OracleOK C) {m : Nat} {A : ACtx} {ρ : Env} {o : Out}
    {dep id line r : Nat} {args : List (Nat × Expr × Option Conv)} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.rule id line r args) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.rule id line r args)) (evI A m (.rule id line r args) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGa : Good C res (evArgs A m args o).2.2 := hG.same ((sat m).args A args o).2
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hnone
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.args m A ρ args o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, _⟩ := bind_ok hv
        obtain ⟨avs, havs, _⟩ := a1 vals hvals
        rw [hnone] at havs; cases havs
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, _⟩
        · exact a2 k l hv hk
        · obtain ⟨avs, havs, _⟩ := a1 vals hvals
          rw [hnone] at havs; cases havs
    · rename_i avs hsome
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.args m A ρ args o dep hG hS (grows_not_widened (ruleAbs_grows (Grows.refl _)) hw)
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
        obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
        rw [hsome] at havs; cases havs
        split at hv1
        · exact absurd hv1 failK_ne_ok
        · rename_i rd0 hrd0
          obtain ⟨c1, _⟩ := rule_case (o := (evArgs A m args o).2.1) (id := id) (line := line) hO hrd0 hGa hs hall hsyms
          exact ⟨(c1 v hv1).1, (c1 v hv1).2, fun _ _ => trivial⟩
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
        · exact Wit.grow (a2 k l hv hk) (ruleAbs_grows (Grows.refl _))
        · obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
          rw [hsome] at havs; cases havs
          split at hv1
          · rw [fail_type hv1] at hk; cases hk
          · rename_i rd0 hrd0
            exact (rule_case (o := (evArgs A m args o).2.1) (id := id) (line := line) hO hrd0 hGa hs hall hsyms).2
              k l hv1 hk

theorem case_dateCall (IH : SoundAt C res n) (hO : OracleOK C) {m : Nat} {A : ACtx} {ρ : Env} {o : Out}
    {dep id line file : Nat} {date : String} {args : List (Nat × Expr × Option Conv)} (hG : Good C res A)
    (hS : Sat A ρ) (hw : (evI A m (.dateCall id line file date args) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.dateCall id line file date args)) (evI A m (.dateCall id line file date args) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGa : Good C res (evArgs A m args o).2.2 := hG.same ((sat m).args A args o).2
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hnone
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.args m A ρ args o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, _⟩ := bind_ok hv
        obtain ⟨avs, havs, _⟩ := a1 vals hvals
        rw [hnone] at havs; cases havs
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, _⟩
        · exact a2 k l hv hk
        · obtain ⟨avs, havs, _⟩ := a1 vals hvals
          rw [hnone] at havs; cases havs
    · rename_i avs hsome
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.args m A ρ args o dep hG hS (grows_not_widened (dateAbs_grows (Grows.refl _)) hw)
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
        obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
        rw [hsome] at havs; cases havs
        split at hv1
        · exact absurd hv1 failK_ne_ok
        · rename_i fd0 hfd0
          obtain ⟨c1, _⟩ := date_case (o := (evArgs A m args o).2.1) (line := line) (date := date) hO hfd0 hGa hs hall
            hsyms
          exact ⟨(c1 v hv1).1, (c1 v hv1).2, fun _ _ => trivial⟩
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
        · exact Wit.grow (a2 k l hv hk) (dateAbs_grows (Grows.refl _))
        · obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
          rw [hsome] at havs; cases havs
          split at hv1
          · rw [fail_type hv1] at hk; cases hk
          · rename_i fd0 hfd0
            exact (date_case (o := (evArgs A m args o).2.1) (line := line) (date := date) hO hfd0 hGa hs hall
              hsyms).2 k l hv1 hk

theorem case_transfer (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {tid : String}
    {params : List String} {args : List (String × Expr × Option Conv × Nat × Bool)} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.transfer id tid params args) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.transfer id tid params args)) (evI A m (.transfer id tid params args) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hnone
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.args5 m A ρ args o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, _⟩ := bind_ok hv
        obtain ⟨avs, havs, _⟩ := a1 vals hvals
        rw [hnone] at havs; cases havs
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, _⟩
        · exact a2 k l hv hk
        · obtain ⟨avs, havs, _⟩ := a1 vals hvals
          rw [hnone] at havs; cases havs
    · rename_i avs hsome
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.args5 m A ρ args o dep hG hS (grows_not_widened (transferAbs_ok (Grows.refl _)) hw)
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
        obtain ⟨avs', havs, hall, hs⟩ := a1 vals hvals
        rw [hsome] at havs; cases havs
        exact ⟨(transfer_case (C := C) (o := (evArgs5 A m args o).2.1) (tid := tid) (params := params) hall).1 v hv1,
          hs, fun _ _ => trivial⟩
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
        · exact Wit.grow (a2 k l hv hk) (transferAbs_ok (Grows.refl _))
        · obtain ⟨avs', havs, hall, _⟩ := a1 vals hvals
          rw [hsome] at havs; cases havs
          exact (transfer_case (C := C) (o := (evArgs5 A m args o).2.1) (tid := tid) (params := params) hall).2
            k l hv1 hk

theorem case_record (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {module : Bool} {fields : List (String × Slot × Bool)} {inits : List (Nat × Expr × Option Conv × Nat)}
    {tag : Option String} {base : Option Expr}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.record id line module fields inits tag base) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.record id line module fields inits tag base))
      (evI A m (.record id line module fields inits tag base) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    cases base with
    | none =>
      simp only [evI] at hw ⊢
      split at hw
      · rename_i hnone
        simp only [AR.of] at hw ⊢
        obtain ⟨a1, a2⟩ := IH.inits m A ρ inits o dep hG hS hw
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨src, hsrc, hv⟩ := bind_ok hv
          obtain ⟨vals, hvals, _⟩ := bind_ok hv
          obtain ⟨avs, havs, _⟩ := a1 vals hvals
          rw [hnone] at havs; cases havs
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨src, hsrc, hv⟩
          · simp at hv
          rcases bind_err hv with hv | ⟨vals, hvals, _⟩
          · exact a2 k l hv hk
          · obtain ⟨avs, havs, _⟩ := a1 vals hvals
            rw [hnone] at havs; cases havs
      · rename_i avs hsome
        simp only [AR.of] at hw ⊢
        obtain ⟨a1, a2⟩ := IH.inits m A ρ inits o dep hG hS
          (grows_not_widened (recordAbs_grows (tag := tag) (src := none) (Grows.refl _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨src, hsrc, hv⟩ := bind_ok hv
          cases hsrc
          obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
          obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
          rw [hsome] at havs; cases havs
          obtain ⟨c1, _⟩ := record_case (C := C) (o := (evInits A m inits o).2.1) (line := line) (module := module)
            (fields := fields) (tag := tag) (src := none) (srcV := none) hs hall hsyms trivial
          exact ⟨(c1 v hv1).1, (c1 v hv1).2, fun _ _ => trivial⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨src, hsrc, hv⟩
          · simp at hv
          cases hsrc
          rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
          · exact Wit.grow (a2 k l hv hk) (recordAbs_grows (tag := tag) (src := none) (Grows.refl _))
          · obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
            rw [hsome] at havs; cases havs
            exact (record_case (C := C) (o := (evInits A m inits o).2.1) (line := line) (module := module)
              (fields := fields) (tag := tag) (src := none) (srcV := none) hs hall hsyms trivial).2 k l hv1 hk
    | some b =>
      have hGb : Good C res (ev A m b o).ctx := hG.same ((sat m).ev A b o).2
      simp only [evI] at hw ⊢
      split at hw
      · -- the value it is made from is never there
        rename_i hnev
        simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ b o dep hG hS hw
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨src, hsrc, _⟩ := bind_ok hv
          obtain ⟨w, hw', _⟩ := bind_ok (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .ok src from hsrc)
          have := (h1 w hw').1; rw [hnev] at this; exact absurd this Mem.not_never
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨src, hsrc, _⟩
          · rcases bind_err (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .error _ from hv) with hv | ⟨w, hw', h'⟩
            · exact h2 k l hv hk
            · simp at h'
          · obtain ⟨w, hw', _⟩ := bind_ok (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .ok src from hsrc)
            have := (h1 w hw').1; rw [hnev] at this; exact absurd this Mem.not_never
      · rename_i hne
        split at hw
        · rename_i hnone
          simp only [AR.of] at hw ⊢
          obtain ⟨h1, h2⟩ := IH.expr m A ρ b o dep hG hS (grows_not_widened (gInits (Grows.refl _)) hw)
          refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
          · simp only [eval] at hv
            obtain ⟨src, hsrc, hv⟩ := bind_ok hv
            obtain ⟨w, hw', hsw⟩ := bind_ok (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .ok src from hsrc)
            obtain ⟨_, hsb, _⟩ := h1 w hw'
            obtain ⟨a1, _⟩ := IH.inits m _ ρ inits _ dep hGb hsb hw
            obtain ⟨vals, hvals, _⟩ := bind_ok hv
            obtain ⟨avs, havs, _⟩ := a1 vals hvals
            rw [hnone] at havs; cases havs
          · simp only [eval] at hv
            rcases bind_err hv with hv | ⟨src, hsrc, hv⟩
            · rcases bind_err (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .error _ from hv) with hv | ⟨w, hw', h'⟩
              · exact Wit.grow (h2 k l hv hk) (gInits (Grows.refl _))
              · simp at h'
            · obtain ⟨w, hw', hsw⟩ := bind_ok (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .ok src from hsrc)
              obtain ⟨_, hsb, _⟩ := h1 w hw'
              obtain ⟨a1, a2⟩ := IH.inits m _ ρ inits _ dep hGb hsb hw
              rcases bind_err hv with hv | ⟨vals, hvals, _⟩
              · exact a2 k l hv hk
              · obtain ⟨avs, havs, _⟩ := a1 vals hvals
                rw [hnone] at havs; cases havs
        · rename_i avs hsome
          simp only [AR.of] at hw ⊢
          have hwI : (evInits (ev A m b o).ctx m inits (ev A m b o).out).2.1.widened = false :=
            grows_not_widened (recordAbs_grows (tag := tag) (src := some ((ev A m b o).abs, exprId b)) (Grows.refl _)) hw
          obtain ⟨h1, h2⟩ := IH.expr m A ρ b o dep hG hS (grows_not_widened (gInits (Grows.refl _)) hwI)
          refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
          · simp only [eval] at hv
            obtain ⟨src, hsrc, hv⟩ := bind_ok hv
            obtain ⟨w, hw', hsw⟩ := bind_ok (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .ok src from hsrc)
            cases hsw
            obtain ⟨hmw, hsb, _⟩ := h1 w hw'
            obtain ⟨a1, _⟩ := IH.inits m _ ρ inits _ dep hGb hsb hwI
            obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
            obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
            rw [hsome] at havs; cases havs
            obtain ⟨c1, _⟩ := record_case (C := C) (o := (evInits _ m inits _).2.1) (line := line) (module := module)
              (fields := fields) (tag := tag) (src := some ((ev A m b o).abs, exprId b)) (srcV := some w) hs hall hsyms hmw
            exact ⟨(c1 v hv1).1, (c1 v hv1).2, fun _ _ => trivial⟩
          · simp only [eval] at hv
            rcases bind_err hv with hv | ⟨src, hsrc, hv⟩
            · rcases bind_err (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .error _ from hv) with hv | ⟨w, hw', h'⟩
              · exact Wit.grow (h2 k l hv hk) (recordAbs_grows (tag := tag) (src := some ((ev A m b o).abs, exprId b)) (gInits (Grows.refl _)))
              · simp at h'
            · obtain ⟨w, hw', hsw⟩ := bind_ok (show (eval C n dep ρ b >>= fun w => Except.ok (some w)) = .ok src from hsrc)
              cases hsw
              obtain ⟨hmw, hsb, _⟩ := h1 w hw'
              obtain ⟨a1, a2⟩ := IH.inits m _ ρ inits _ dep hGb hsb hwI
              rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
              · exact Wit.grow (a2 k l hv hk) (recordAbs_grows (tag := tag) (src := some ((ev A m b o).abs, exprId b)) (Grows.refl _))
              · obtain ⟨avs', havs, hall, hsyms, hs⟩ := a1 vals hvals
                rw [hsome] at havs; cases havs
                exact (record_case (C := C) (o := (evInits _ m inits _).2.1) (line := line) (module := module)
                  (fields := fields) (tag := tag) (src := some ((ev A m b o).abs, exprId b)) (srcV := some w)
                  hs hall hsyms hmw).2 k l hv1 hk

-- ── Lists ────────────────────────────────────────────────────────────────────

theorem case_list (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat}
    {elems : List (Expr × Option Conv)} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.list id elems) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.list id elems)) (evI A m (.list id elems) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    obtain ⟨e1, e2⟩ := IH.elems m A ρ elems .never 0 o dep [] hG hS hw (fun _ h => by cases h) rfl
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨vs, hvs, hv1⟩ := bind_ok hv
      cases hv1
      obtain ⟨hm, hs⟩ := e1 vs hvs
      exact ⟨hm, hs, fun t ht => by cases ht⟩
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨vs, _, hv1⟩
      · exact e2 k l hv hk
      · cases hv1

theorem case_walk (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {w : Walk}
    {list : Expr} {x : String} {body : Expr} {c : Option Conv} {ord : Order} {step : Option Rat}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.walk id line w list x body c ord step) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.walk id line w list x body c ord step))
      (evI A m (.walk id line w list x body c ord step) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    exact walk_case IH hG hS hw

theorem case_count1 (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {l : Expr}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.count1 id l) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.count1 id l)) (evI A m (.count1 id l) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, AR.of] at hw ⊢
    obtain ⟨h1, h2⟩ := IH.expr m A ρ l o dep hG hS hw
    refine ⟨fun v hv => ?_, fun k l' hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨w, hw', hv1⟩ := bind_ok hv
      cases hv1
      obtain ⟨hm, hs, _⟩ := h1 w hw'
      refine ⟨?_, hs, fun t ht => by cases ht⟩
      split
      · rename_i hn; rw [hn] at hm; exact absurd hm Mem.not_never
      · rename_i c e hl
        rw [hl] at hm
        cases hm with
        | list hc _ => exact Mem.num hc
      · rename_i c k e hl
        rw [hl] at hm
        cases hm with
        | dict hc _ _ _ => exact Mem.num hc
      · rename_i c e hl
        rw [hl] at hm
        cases hm with
        | members hc _ _ => exact Mem.num hc
      · rename_i c hl
        rw [hl] at hm
        cases hm with
        | str hc => exact Mem.num hc
      · apply Mem.num
        cases w with
        | list xs => exact In_mk' (by simp [EndLe]; exact_mod_cast Nat.zero_le _) trivial (by simp [DenLe])
        | dict ps => exact In_mk' (by simp [EndLe]; exact_mod_cast Nat.zero_le _) trivial (by simp [DenLe])
        | set xs => exact In_mk' (by simp [EndLe]; exact_mod_cast Nat.zero_le _) trivial (by simp [DenLe])
        | str s => exact In_mk' (by simp [EndLe]; exact_mod_cast Nat.zero_le _) trivial (by simp [DenLe])
        | _ => exact In_mk' (by simp [EndLe]) trivial (by simp [DenLe])
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨w, _, hv1⟩
      · exact h2 k l' hv hk
      · cases hv1

-- ── Strings ──────────────────────────────────────────────────────────────────

theorem case_startsWith (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat}
    {s p : Expr} (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.startsWith id s p) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.startsWith id s p)) (evI A m (.startsWith id s p) o) := by
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
      split at hw
      · rename_i hnev
        simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ s o dep hG hS (grows_not_widened (gEv (Grows.refl _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨sv, hsv, hv1⟩ := bind_ok hv
          obtain ⟨pv, hpv, _⟩ := bind_ok hv1
          obtain ⟨_, hs, _⟩ := h1 sv hsv
          have := ((IH.expr m _ ρ p _ dep hGs hs hw).1 pv hpv).1; rw [hnev] at this; exact absurd this Mem.not_never
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨sv, hsv, hv1⟩
          · exact Wit.grow (h2 k l hv hk) (gEv (Grows.refl _))
          · obtain ⟨_, hs, _⟩ := h1 sv hsv
            rcases bind_err hv1 with hv2 | ⟨pv, hpv, _⟩
            · exact (IH.expr m _ ρ p _ dep hGs hs hw).2 k l hv2 hk
            · have := ((IH.expr m _ ρ p _ dep hGs hs hw).1 pv hpv).1; rw [hnev] at this
              exact absurd this Mem.not_never
      · simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ s o dep hG hS (grows_not_widened (gEv (Grows.refl _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨sv, hsv, hv1⟩ := bind_ok hv
          obtain ⟨pv, hpv, hv2⟩ := bind_ok hv1
          cases hv2
          obtain ⟨_, hs, _⟩ := h1 sv hsv
          obtain ⟨_, hs', _⟩ := (IH.expr m _ ρ p _ dep hGs hs hw).1 pv hpv
          exact ⟨Mem_allBools _, hs', fun _ _ => trivial⟩
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨sv, hsv, hv1⟩
          · exact Wit.grow (h2 k l hv hk) (gEv (Grows.refl _))
          · obtain ⟨_, hs, _⟩ := h1 sv hsv
            rcases bind_err hv1 with hv2 | ⟨pv, hpv, hv2⟩
            · exact (IH.expr m _ ρ p _ dep hGs hs hw).2 k l hv2 hk
            · cases hv2

theorem case_bad {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {args : List Expr} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.bad id line args)) (evI A m (.bad id line args) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv; exact absurd hv failK_ne_ok
    · simp only [eval] at hv; rw [fail_type hv] at hk; cases hk

end

end KanameCore
