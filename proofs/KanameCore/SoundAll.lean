/-
  **The analysis of ranges is sound** (kaname's DESIGN 15.2.2): every evaluation, at any number of
  steps, keeps what the analysis of the same expression says (`SoundAt`), by induction on the steps.
-/
import KanameCore.SoundArith

namespace KanameCore

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

-- ── What the evaluation of a list of arguments shows of its own ─────────────

theorem evalArgs_syms {dep : Nat} {ρ : Env} : ∀ (n : Nat) (args : List (Nat × Expr × Option Conv)) (vs : List Val),
    evalArgs C n dep ρ args = .ok vs → ArgSyms ρ (args.map (·.2.1)) vs
  | 0, _, _, h => by simp [evalArgs] at h
  | _ + 1, [], vs, h => by simp [evalArgs] at h; subst h; trivial
  | n + 1, (_, x, _) :: rest, vs, h => by
    simp only [evalArgs] at h
    obtain ⟨v, hv, h1⟩ := bind_ok h
    obtain ⟨vs', hvs, h2⟩ := bind_ok h1
    cases h2
    exact ⟨fun s hs => sym_eval hs hv, evalArgs_syms n rest vs' hvs⟩

theorem evalInits_syms {dep : Nat} {ρ : Env} :
    ∀ (n : Nat) (inits : List (Nat × Expr × Option Conv × Nat)) (vs : List Val),
    evalInits C n dep ρ inits = .ok vs → ArgSyms ρ (inits.map (·.2.1)) vs
  | 0, _, _, h => by simp [evalInits] at h
  | _ + 1, [], vs, h => by simp [evalInits] at h; subst h; trivial
  | n + 1, (_, x, _) :: rest, vs, h => by
    simp only [evalInits] at h
    obtain ⟨v, hv, h1⟩ := bind_ok h
    obtain ⟨vs', hvs, h2⟩ := bind_ok h1
    cases h2
    exact ⟨fun s hs => sym_eval hs hv, evalInits_syms n rest vs' hvs⟩

theorem evalArgs4_length {dep : Nat} {ρ : Env} :
    ∀ (n : Nat) (args : List (String × Expr × Option Conv × Nat × Bool)) (vs : List Val),
    evalArgs4 C n dep ρ args = .ok vs → vs.length = args.length
  | 0, _, _, h => by simp [evalArgs4] at h
  | _ + 1, [], vs, h => by simp [evalArgs4] at h; subst h; rfl
  | n + 1, (_, _, _) :: rest, vs, h => by
    simp only [evalArgs4] at h
    obtain ⟨v, _, h1⟩ := bind_ok h
    obtain ⟨vs', hvs, h2⟩ := bind_ok h1
    cases h2
    simp [evalArgs4_length n rest vs' hvs]

theorem evalList_length {dep : Nat} {ρ : Env} : ∀ (n : Nat) (args : List Expr) (vs : List Val),
    evalList C n dep ρ args = .ok vs → vs.length = args.length
  | 0, _, _, h => by simp [evalList] at h
  | _ + 1, [], vs, h => by simp [evalList] at h; subst h; rfl
  | n + 1, _ :: rest, vs, h => by
    simp only [evalList] at h
    obtain ⟨v, _, h1⟩ := bind_ok h
    obtain ⟨vs', hvs, h2⟩ := bind_ok h1
    cases h2
    simp [evalList_length n rest vs' hvs]

theorem allMem_any {α : Type} : ∀ (args : List α) (vs : List Val), vs.length = args.length →
    AllMem (args.map fun _ => .any) vs
  | [], [], _ => trivial
  | _ :: args, v :: vs, h => ⟨Mem.any v, allMem_any args vs (by simpa using h)⟩
  | [], _ :: _, h => by simp at h
  | _ :: _, [], h => by simp at h

-- ── One more step ────────────────────────────────────────────────────────────

theorem concl_record {ρ : Env} {res' : R Val} {r : AR} {id : Nat} (h : Concl C ρ res' r) :
    Concl C ρ res' { r with out := r.out.record id r.abs } :=
  ⟨h.1, fun k l hv hk => Wit.grow (h.2 k l hv hk) (grows_record (Grows.refl _))⟩

/-- Every form but `let`, one more step. -/
theorem evI_succ (IH : SoundAt C res n) (hR : ResultsOK C res) (hO : OracleOK C) (m : Nat) (A : ACtx) (ρ : Env)
    (e : Expr) (o : Out) (dep : Nat) (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m e o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ e) (evI A m e o) := by
  cases e with
  | num => exact case_num hS
  | date => exact case_date hS
  | str => exact case_str hS
  | bool => exact case_bool hS
  | none => exact case_none hS
  | enum => exact case_enum hS
  | var => exact case_var hS
  | field => exact case_field IH hG hS hw
  | neg => exact case_neg IH hG hS hw
  | not => exact case_not IH hG hS hw
  | inUnit => exact case_inUnit IH hG hS hw
  | arith => exact case_arith IH hG hS hw
  | ddiff => exact case_ddiff IH hG hS hw
  | cmp => exact case_cmp IH hG hS hw
  | and => exact case_and IH hG hS hw
  | or => exact case_or IH hG hS hw
  | ite => exact case_ite IH hG hS hw
  | mtch => exact case_mtch IH hG hS hw
  | letIn x v c body =>
    cases m with
    | zero => exact concl_fuel hS
    | succ m => simp only [evI] at hw ⊢; exact case_letIn IH hG hS hw
  | block => exact case_block IH hG hS hw
  | call => exact case_call IH hR hG hS hw
  | rule => exact case_rule IH hO hG hS hw
  | dateCall => exact case_dateCall IH hO hG hS hw
  | transfer => exact case_transfer IH hG hS hw
  | record => exact case_record IH hG hS hw
  | list => exact case_list IH hG hS hw
  | walk => exact case_walk IH hG hS hw
  | reduce => exact case_reduce IH hG hS hw
  | minmax => exact case_minmax IH hG hS hw
  | modOp => exact case_modOp IH hG hS hw
  | isqrt => exact case_isqrt IH hG hS hw
  | allocate => exact case_allocate IH hG hS hw
  | round => exact case_round IH hG hS hw
  | count1 => exact case_count1 IH hG hS hw
  | startsWith => exact case_startsWith IH hG hS hw
  | concat => exact case_concat IH hG hS hw
  | text => exact case_text IH hG hS hw
  | bad => exact case_bad hS

theorem ev_succ (IH : SoundAt C res n) (hR : ResultsOK C res) (hO : OracleOK C) (m : Nat) (A : ACtx) (ρ : Env)
    (e : Expr) (o : Out) (dep : Nat) (hG : Good C res A) (hS : Sat A ρ) (hw : (ev A m e o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ e) (ev A m e o) := by
  cases e
  case letIn x v c body => exact case_letIn IH hG hS hw
  all_goals
    cases m with
    | zero => exact concl_fuel hS
    | succ m =>
      simp only [ev, Expr.id?] at hw ⊢
      exact concl_record (evI_succ IH hR hO m A ρ _ o dep hG hS (grows_not_widened (grows_record (Grows.refl _)) hw))

theorem args_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (args : List (Nat × Expr × Option Conv))
    (o : Out) (dep : Nat) (hG : Good C res A) (hS : Sat A ρ) (hw : (evArgs A m args o).2.1.widened = false) :
    (∀ vs, evalArgs C (n + 1) dep ρ args = .ok vs → ∃ avs, (evArgs A m args o).1 = some avs ∧ AllMem avs vs ∧
        ArgSyms ρ (args.map (·.2.1)) vs ∧ Sat (evArgs A m args o).2.2 ρ) ∧
    (∀ k l, evalArgs C (n + 1) dep ρ args = .error (.failed k l) → k.ofEffect = true →
      Wit C (evArgs A m args o).2.1) := by
  cases m with
  | zero =>
    simp only [evArgs]
    refine ⟨fun vs h => ⟨_, rfl, ?_, evalArgs_syms _ _ _ h, hS⟩, fun _ _ _ _ => wit_fuel⟩
    exact allMem_any _ _ (by have := ArgSyms.length (evalArgs_syms _ _ _ h); simpa using this.symm)
  | succ m =>
    cases args with
    | nil =>
      simp only [evArgs]
      refine ⟨fun vs h => ?_, fun k l h => ?_⟩
      · simp [evalArgs] at h; subst h; exact ⟨[], rfl, trivial, trivial, hS⟩
      · simp [evalArgs] at h
    | cons a rest =>
      obtain ⟨k0, x, c⟩ := a
      simp only [evArgs] at hw ⊢
      split at hw
      · rename_i hn
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalArgs] at h
          obtain ⟨v, hv, _⟩ := bind_ok h
          have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
        · simp only [evalArgs] at h
          rcases bind_err h with h | ⟨v, hv, _⟩
          · exact h2 k l h hk
          · have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
      · have hGx : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (gArgs (Grows.refl _)) hw)
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalArgs] at h
          obtain ⟨v, hv, h3⟩ := bind_ok h
          obtain ⟨vs', hvs, h4⟩ := bind_ok h3
          cases h4
          obtain ⟨hm, hs, _⟩ := h1 v hv
          obtain ⟨avs, havs, hall, hsyms, hsat⟩ := (IH.args m _ ρ rest _ dep hGx hs hw).1 vs' hvs
          exact ⟨(ev A m x o).abs :: avs, by rw [havs]; rfl, ⟨hm, hall⟩, ⟨fun s hs' => sym_eval hs' hv, hsyms⟩, hsat⟩
        · simp only [evalArgs] at h
          rcases bind_err h with h | ⟨v, hv, h3⟩
          · exact Wit.grow (h2 k l h hk) (gArgs (Grows.refl _))
          · rcases bind_err h3 with h4 | ⟨vs', _, h4⟩
            · exact (IH.args m _ ρ rest _ dep hGx (h1 v hv).2.1 hw).2 k l h4 hk
            · cases h4

theorem args5_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env)
    (args : List (String × Expr × Option Conv × Nat × Bool)) (o : Out) (dep : Nat) (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evArgs5 A m args o).2.1.widened = false) :
    (∀ vs, evalArgs4 C (n + 1) dep ρ args = .ok vs → ∃ avs, (evArgs5 A m args o).1 = some avs ∧ AllMem avs vs ∧
        Sat (evArgs5 A m args o).2.2 ρ) ∧
    (∀ k l, evalArgs4 C (n + 1) dep ρ args = .error (.failed k l) → k.ofEffect = true →
      Wit C (evArgs5 A m args o).2.1) := by
  cases m with
  | zero =>
    simp only [evArgs5]
    refine ⟨fun vs h => ⟨_, rfl, ?_, hS⟩, fun _ _ _ _ => wit_fuel⟩
    exact allMem_any _ _ (evalArgs4_length _ _ _ h)
  | succ m =>
    cases args with
    | nil =>
      simp only [evArgs5]
      refine ⟨fun vs h => ?_, fun k l h => ?_⟩
      · simp [evalArgs4] at h; subst h; exact ⟨[], rfl, trivial, hS⟩
      · simp [evalArgs4] at h
    | cons a rest =>
      obtain ⟨k0, x, c⟩ := a
      simp only [evArgs5] at hw ⊢
      split at hw
      · rename_i hn
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalArgs4] at h
          obtain ⟨v, hv, _⟩ := bind_ok h
          have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
        · simp only [evalArgs4] at h
          rcases bind_err h with h | ⟨v, hv, _⟩
          · exact h2 k l h hk
          · have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
      · have hGx : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (gArgs5 (Grows.refl _)) hw)
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalArgs4] at h
          obtain ⟨v, hv, h3⟩ := bind_ok h
          obtain ⟨vs', hvs, h4⟩ := bind_ok h3
          cases h4
          obtain ⟨hm, hs, _⟩ := h1 v hv
          obtain ⟨avs, havs, hall, hsat⟩ := (IH.args5 m _ ρ rest _ dep hGx hs hw).1 vs' hvs
          exact ⟨(ev A m x o).abs :: avs, by rw [havs]; rfl, ⟨hm, hall⟩, hsat⟩
        · simp only [evalArgs4] at h
          rcases bind_err h with h | ⟨v, hv, h3⟩
          · exact Wit.grow (h2 k l h hk) (gArgs5 (Grows.refl _))
          · rcases bind_err h3 with h4 | ⟨vs', _, h4⟩
            · exact (IH.args5 m _ ρ rest _ dep hGx (h1 v hv).2.1 hw).2 k l h4 hk
            · cases h4

theorem inits_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (inits : List (Nat × Expr × Option Conv × Nat))
    (o : Out) (dep : Nat) (hG : Good C res A) (hS : Sat A ρ) (hw : (evInits A m inits o).2.1.widened = false) :
    (∀ vs, evalInits C (n + 1) dep ρ inits = .ok vs → ∃ avs, (evInits A m inits o).1 = some avs ∧ AllMem avs vs ∧
        ArgSyms ρ (inits.map (·.2.1)) vs ∧ Sat (evInits A m inits o).2.2 ρ) ∧
    (∀ k l, evalInits C (n + 1) dep ρ inits = .error (.failed k l) → k.ofEffect = true →
      Wit C (evInits A m inits o).2.1) := by
  cases m with
  | zero =>
    simp only [evInits]
    refine ⟨fun vs h => ⟨_, rfl, ?_, evalInits_syms _ _ _ h, hS⟩, fun _ _ _ _ => wit_fuel⟩
    exact allMem_any _ _ (by have := ArgSyms.length (evalInits_syms _ _ _ h); simpa using this.symm)
  | succ m =>
    cases inits with
    | nil =>
      simp only [evInits]
      refine ⟨fun vs h => ?_, fun k l h => ?_⟩
      · simp [evalInits] at h; subst h; exact ⟨[], rfl, trivial, trivial, hS⟩
      · simp [evalInits] at h
    | cons a rest =>
      obtain ⟨k0, x, c⟩ := a
      simp only [evInits] at hw ⊢
      split at hw
      · rename_i hn
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalInits] at h
          obtain ⟨v, hv, _⟩ := bind_ok h
          have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
        · simp only [evalInits] at h
          rcases bind_err h with h | ⟨v, hv, _⟩
          · exact h2 k l h hk
          · have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
      · have hGx : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (gInits (Grows.refl _)) hw)
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalInits] at h
          obtain ⟨v, hv, h3⟩ := bind_ok h
          obtain ⟨vs', hvs, h4⟩ := bind_ok h3
          cases h4
          obtain ⟨hm, hs, _⟩ := h1 v hv
          obtain ⟨avs, havs, hall, hsyms, hsat⟩ := (IH.inits m _ ρ rest _ dep hGx hs hw).1 vs' hvs
          exact ⟨(ev A m x o).abs :: avs, by rw [havs]; rfl, ⟨hm, hall⟩, ⟨fun s hs' => sym_eval hs' hv, hsyms⟩, hsat⟩
        · simp only [evalInits] at h
          rcases bind_err h with h | ⟨v, hv, h3⟩
          · exact Wit.grow (h2 k l h hk) (gInits (Grows.refl _))
          · rcases bind_err h3 with h4 | ⟨vs', _, h4⟩
            · exact (IH.inits m _ ρ rest _ dep hGx (h1 v hv).2.1 hw).2 k l h4 hk
            · cases h4

theorem list_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (args : List Expr) (o : Out) (dep : Nat)
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evList A m args o).2.1.widened = false) :
    (∀ vs, evalList C (n + 1) dep ρ args = .ok vs → ∃ avs, (evList A m args o).1 = some avs ∧ AllMem avs vs ∧
        Sat (evList A m args o).2.2 ρ) ∧
    (∀ k l, evalList C (n + 1) dep ρ args = .error (.failed k l) → k.ofEffect = true →
      Wit C (evList A m args o).2.1) := by
  cases m with
  | zero =>
    simp only [evList]
    refine ⟨fun vs h => ⟨_, rfl, ?_, hS⟩, fun _ _ _ _ => wit_fuel⟩
    exact allMem_any _ _ (evalList_length _ _ _ h)
  | succ m =>
    cases args with
    | nil =>
      simp only [evList]
      refine ⟨fun vs h => ?_, fun k l h => ?_⟩
      · simp [evalList] at h; subst h; exact ⟨[], rfl, trivial, hS⟩
      · simp [evalList] at h
    | cons x rest =>
      simp only [evList] at hw ⊢
      split at hw
      · rename_i hn
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalList] at h
          obtain ⟨v, hv, _⟩ := bind_ok h
          have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
        · simp only [evalList] at h
          rcases bind_err h with h | ⟨v, hv, _⟩
          · exact h2 k l h hk
          · have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
      · have hGx : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (gList (Grows.refl _)) hw)
        refine ⟨fun vs h => ?_, fun k l h hk => ?_⟩
        · simp only [evalList] at h
          obtain ⟨v, hv, h3⟩ := bind_ok h
          obtain ⟨vs', hvs, h4⟩ := bind_ok h3
          cases h4
          obtain ⟨hm, hs, _⟩ := h1 v hv
          obtain ⟨avs, havs, hall, hsat⟩ := (IH.list m _ ρ rest _ dep hGx hs hw).1 vs' hvs
          exact ⟨(ev A m x o).abs :: avs, by rw [havs]; rfl, ⟨hm, hall⟩, hsat⟩
        · simp only [evalList] at h
          rcases bind_err h with h | ⟨v, hv, h3⟩
          · exact Wit.grow (h2 k l h hk) (gList (Grows.refl _))
          · rcases bind_err h3 with h4 | ⟨vs', _, h4⟩
            · exact (IH.list m _ ρ rest _ dep hGx (h1 v hv).2.1 hw).2 k l h4 hk
            · cases h4

theorem elems_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (elems : List (Expr × Option Conv))
    (elem : Abs) (k : Nat) (o : Out) (dep : Nat) (pre : List Val) (hG : Good C res A) (hS : Sat A ρ)
    (hw : (listAbs A m elems elem k o).out.widened = false) (hpre : ∀ x ∈ pre, Mem elem x) (hk : pre.length = k) :
    (∀ vs, evalElems C (n + 1) dep ρ elems = .ok vs → Mem (listAbs A m elems elem k o).abs (.list (pre ++ vs)) ∧
        Sat (listAbs A m elems elem k o).ctx ρ) ∧
    (∀ k' l, evalElems C (n + 1) dep ρ elems = .error (.failed k' l) → k'.ofEffect = true →
      Wit C (listAbs A m elems elem k o).out) := by
  cases m with
  | zero =>
    simp only [listAbs]
    exact ⟨fun vs _ => ⟨Mem.any _, hS⟩, fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    cases elems with
    | nil =>
      simp only [listAbs, AR.of]
      refine ⟨fun vs h => ?_, fun k' l h => ?_⟩
      · simp [evalElems] at h; subst h
        refine ⟨Mem.list ?_ (by simpa using hpre), hS⟩
        simp only [List.append_nil, hk]
        exact In_ints (Int.le_refl _) (Int.le_refl _)
      · simp [evalElems] at h
    | cons xc rest =>
      obtain ⟨x, c⟩ := xc
      simp only [listAbs] at hw ⊢
      split at hw
      · rename_i hn
        simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
        refine ⟨fun vs h => ?_, fun k' l h hk' => ?_⟩
        · simp only [evalElems] at h
          obtain ⟨v, hv, _⟩ := bind_ok h
          have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
        · simp only [evalElems] at h
          rcases bind_err h with h | ⟨v, hv, _⟩
          · exact h2 k' l h hk'
          · have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
      · have hGx : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS
          (grows_not_widened (gElems (grows_convA (Grows.refl _))) hw)
        refine ⟨fun vs h => ?_, fun k' l h hk' => ?_⟩
        · simp only [evalElems] at h
          obtain ⟨v, hv, h3⟩ := bind_ok h
          obtain ⟨v', hv', h4⟩ := bind_ok h3
          obtain ⟨vs', hvs, h5⟩ := bind_ok h4
          cases h5
          obtain ⟨hm, hs, _⟩ := h1 v hv
          have hm' := convA_mem (o := (ev A m x o).out) hm hv'
          have hpre' : ∀ y ∈ pre ++ [v'], Mem (elem.join (convA (ev A m x o).out c (ev A m x o).abs).1) y := by
            intro y hy
            rcases List.mem_append.mp hy with hy | hy
            · exact mem_join_left (hpre y hy)
            · simp at hy; subst hy; exact mem_join_right hm'
          obtain ⟨e1, e2⟩ := (IH.elems m _ ρ rest _ (k + 1) _ dep (pre ++ [v']) hGx hs hw hpre'
            (by simp [hk])).1 vs' hvs
          refine ⟨?_, e2⟩
          have : pre ++ v' :: vs' = (pre ++ [v']) ++ vs' := by simp
          rw [this]; exact e1
        · simp only [evalElems] at h
          rcases bind_err h with h | ⟨v, hv, h3⟩
          · exact Wit.grow (h2 k' l h hk') (gElems (grows_convA (Grows.refl _)))
          · obtain ⟨hm, hs, _⟩ := h1 v hv
            rcases bind_err h3 with h4 | ⟨v', hv', h4⟩
            · exact Wit.grow (coerce_wit hm h4).2 (gElems (Grows.refl _))
            · rcases bind_err h4 with h5 | ⟨vs', _, h5⟩
              · have hm' := convA_mem (o := (ev A m x o).out) hm hv'
                have hpre' : ∀ y ∈ pre ++ [v'], Mem (elem.join (convA (ev A m x o).out c (ev A m x o).abs).1) y := by
                  intro y hy
                  rcases List.mem_append.mp hy with hy | hy
                  · exact mem_join_left (hpre y hy)
                  · simp at hy; subst hy; exact mem_join_right hm'
                exact (IH.elems m _ ρ rest _ (k + 1) _ dep (pre ++ [v']) hGx hs hw hpre' (by simp [hk])).2 k' l h5 hk'
              · cases h5

theorem each_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (x : String) (body : Expr) (c : Option Conv)
    (w : Walk) (xs : List Val) (elem : Abs) (o : Out) (dep : Nat) (hG : Good C res A) (hS : Sat A ρ)
    (hel : ∀ el ∈ xs, Mem elem el) (hw : (ev (A.bindNew x elem) m body o).out.widened = false) :
    (∀ as, evalEach C (n + 1) dep ρ x body c w xs = .ok as →
      as.length ≤ xs.length ∧ ((∀ a ∈ as, stopAt w a = false) → as.length = xs.length) ∧ (xs ≠ [] → as ≠ []) ∧
      ∀ el a, (el, a) ∈ xs.zip as → AnsOK ρ x (ev (A.bindNew x elem) m body o) c el a) ∧
    (∀ k l, evalEach C (n + 1) dep ρ x body c w xs = .error (.failed k l) → k.ofEffect = true →
      Wit C (convA (ev (A.bindNew x elem) m body o).out c (ev (A.bindNew x elem) m body o).abs).2) := by
  cases xs with
  | nil =>
    refine ⟨fun as h => ?_, fun k l h => ?_⟩
    · simp [evalEach] at h; subst h; simp
    · simp [evalEach] at h
  | cons el rest =>
    have he : Mem elem el := hel el (List.mem_cons_self ..)
    have hrest : ∀ e ∈ rest, Mem elem e := fun e h => hel e (List.mem_cons_of_mem _ h)
    have hG' : Good C res (A.bindNew x elem) := hG
    obtain ⟨b1, b2⟩ := IH.expr m (A.bindNew x elem) ((x, el) :: ρ) body o dep hG' (hS.bindNew he) hw
    refine ⟨fun as h => ?_, fun k l h hk => ?_⟩
    · simp only [evalEach] at h
      obtain ⟨raw, hraw, h1⟩ := bind_ok h
      obtain ⟨a, ha, h2⟩ := bind_ok h1
      obtain ⟨hm, hs, hn⟩ := b1 raw hraw
      have hans : AnsOK ρ x (ev (A.bindNew x elem) m body o) c el a := ⟨raw, hm, hs, hn, ha⟩
      split at h2
      · rename_i hstop
        cases h2
        refine ⟨by simp, fun hall => ?_, fun _ => by simp, fun el' a' h' => ?_⟩
        · have := hall a (List.mem_singleton_self a); rw [hstop] at this; cases this
        · simp at h'; obtain ⟨rfl, rfl⟩ := h'; exact hans
      · rename_i hstop
        obtain ⟨as', has', h3⟩ := bind_ok h2
        cases h3
        obtain ⟨l1, l2, _, l4⟩ := (IH.each m A ρ x body c w rest elem o dep hG hS hrest hw).1 as' has'
        refine ⟨by simp; exact l1, fun hall => ?_, fun _ => by simp, fun el' a' h' => ?_⟩
        · simp only [List.length_cons]
          rw [l2 (fun b hb => hall b (List.mem_cons_of_mem _ hb))]
        · simp only [List.zip_cons_cons, List.mem_cons, Prod.mk.injEq] at h'
          rcases h' with ⟨rfl, rfl⟩ | h'
          · exact hans
          · exact l4 el' a' h'
    · simp only [evalEach] at h
      rcases bind_err h with h | ⟨raw, hraw, h1⟩
      · exact Wit.grow (b2 k l h hk) (grows_convA (Grows.refl _))
      · rcases bind_err h1 with h2 | ⟨a, _, h2⟩
        · exact (coerce_wit (b1 raw hraw).1 h2).2
        · split at h2
          · cases h2
          · rcases bind_err h2 with h3 | ⟨as', _, h3⟩
            · exact (IH.each m A ρ x body c w rest elem o dep hG hS hrest hw).2 k l h3 hk
            · cases h3

-- ── Every step ───────────────────────────────────────────────────────────────

theorem sound_succ (IH : SoundAt C res n) (hR : ResultsOK C res) (hO : OracleOK C) : SoundAt C res (n + 1) where
  expr := ev_succ IH hR hO
  args := args_succ IH
  args5 := args5_succ IH
  inits := inits_succ IH
  list := list_succ IH
  elems := elems_succ IH
  each := each_succ IH
  fix := fix_succ IH
  rounds := rounds_succ IH

/-- **What the analysis says of every evaluation**, at any number of steps. -/
theorem sound_all (hR : ResultsOK C res) (hO : OracleOK C) : ∀ n, SoundAt C res n
  | 0 => sound_zero
  | n + 1 => sound_succ (sound_all hR hO n) hR hO

end

/-- **The analysis of ranges is sound** (DESIGN 15.2.2): in an environment among those the scope of
    the analysis says, an answer of the evaluation is among the values the analysis says, the scope
    still holds, and a failure of one of the five kinds is one the analysis kept a place for, or one
    of a function it calls. -/
theorem ranges_sound {C : Ctx} {res : List (Option Abs)} (hR : ResultsOK C res) (hO : OracleOK C) {n m dep : Nat}
    {A : ACtx} {ρ : Env} {e : Expr} {o : Out} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (ev A m e o).out.widened = false) :
    Concl C ρ (eval C n dep ρ e) (ev A m e o) :=
  (sound_all hR hO n).expr m A ρ e o dep hG hS hw

end KanameCore
