/-
  Arithmetic (kaname's DESIGN 5.2, 5.3, 5.6, 15.2.2, 15.2.3): `+`, `-`, `*`, `/` and the days
  between two dates, comparisons, `min` and `max`, `mod`, `isqrt`, `allocate` and the roundings.
-/
import KanameCore.SoundReduce

namespace KanameCore

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

-- ── `+`, `-`, `*`, `/` ───────────────────────────────────────────────────────

theorem arith_ok {dep id line : Nat} {ρ : Env} {op : ArOp} {a b : Expr} {ca cb : Option Conv} {step : Option Rat}
    {v : Val} (h : eval C (n + 1) dep ρ (.arith id line op a b ca cb step) = .ok v) :
    ∃ p q r, eval C n dep ρ a = .ok (.num p) ∧ eval C n dep ρ b = .ok (.num q) ∧ arithR line op ca cb p q = .ok r ∧
      (∀ s, step = some s → onStep s r = true) ∧ fits r = true ∧ v = .num r := by
  simp only [eval] at h
  obtain ⟨x, hx, h1⟩ := bind_ok h
  obtain ⟨y, hy, h2⟩ := bind_ok h1
  cases x <;> cases y <;> simp only [failK, reduceCtorEq] at h2
  obtain ⟨r, hr, h3⟩ := bind_ok h2
  obtain ⟨u, hu, h4⟩ := bind_ok h3
  obtain ⟨r', hr', h5⟩ := bind_ok h4
  cases h5
  obtain ⟨rfl, hf⟩ := held_ok (liftA_ok hr')
  exact ⟨_, _, _, hx, hy, hr, stepped_num hu, hf, rfl⟩

theorem arith_err {dep id line : Nat} {ρ : Env} {op : ArOp} {a b : Expr} {ca cb : Option Conv} {step : Option Rat}
    {k : Kind} {l : Nat} (h : eval C (n + 1) dep ρ (.arith id line op a b ca cb step) = .error (.failed k l))
    (hk : k.ofEffect = true) :
    eval C n dep ρ a = .error (.failed k l) ∨
    (∃ x, eval C n dep ρ a = .ok x ∧ eval C n dep ρ b = .error (.failed k l)) ∨
    (∃ p q, eval C n dep ρ a = .ok (.num p) ∧ eval C n dep ρ b = .ok (.num q) ∧
      arithR line op ca cb p q = .error (.failed k l)) ∨
    (∃ p q r, eval C n dep ρ a = .ok (.num p) ∧ eval C n dep ρ b = .ok (.num q) ∧ arithR line op ca cb p q = .ok r ∧
      (∀ s, step = some s → onStep s r = true) ∧ fits r = false) := by
  simp only [eval] at h
  rcases bind_err h with h | ⟨x, hx, h1⟩
  · exact Or.inl h
  · rcases bind_err h1 with h1 | ⟨y, hy, h2⟩
    · exact Or.inr (Or.inl ⟨x, hx, h1⟩)
    · cases x <;> cases y <;> simp only at h2 <;> try (rw [fail_type h2] at hk; cases hk)
      rcases bind_err h2 with h3 | ⟨r, hr, h3⟩
      · exact Or.inr (Or.inr (Or.inl ⟨_, _, hx, hy, h3⟩))
      · rcases bind_err h3 with h4 | ⟨u, hu, h4⟩
        · rw [type_noEffect (stepped_err h4)] at hk; cases hk
        · rcases bind_err h4 with h5 | ⟨r', _, h5⟩
          · rcases liftA_err h5 with ⟨h6, _⟩ | ⟨h6, _⟩
            · exact Or.inr (Or.inr (Or.inr ⟨_, _, _, hx, hy, hr, stepped_num hu, (held_err h6).2⟩))
            · simp [held] at h6; split at h6 <;> simp at h6
          · cases h5

theorem arithR_add {line : Nat} {ca cb : Option Conv} {p q r : Rat} (h : arithR line .add ca cb p q = .ok r) :
    ∃ p' q', convN ca p = .ok p' ∧ convN cb q = .ok q' ∧ r = p' + q' := by
  simp only [arithR] at h
  obtain ⟨p', hp', h1⟩ := bind_ok h
  obtain ⟨q', hq', h2⟩ := bind_ok h1
  cases h2; exact ⟨p', q', hp', hq', rfl⟩

theorem arithR_sub {line : Nat} {ca cb : Option Conv} {p q r : Rat} (h : arithR line .sub ca cb p q = .ok r) :
    ∃ p' q', convN ca p = .ok p' ∧ convN cb q = .ok q' ∧ r = p' - q' := by
  simp only [arithR] at h
  obtain ⟨p', hp', h1⟩ := bind_ok h
  obtain ⟨q', hq', h2⟩ := bind_ok h1
  cases h2; exact ⟨p', q', hp', hq', rfl⟩

theorem arithR_div {line : Nat} {ca cb : Option Conv} {p q r : Rat} (h : arithR line .div ca cb p q = .ok r) :
    ∃ p' q', convN ca p = .ok p' ∧ convN cb q = .ok q' ∧ q' ≠ 0 ∧ r = p' / q' := by
  simp only [arithR] at h
  obtain ⟨p', hp', h1⟩ := bind_ok h
  obtain ⟨q', hq', h2⟩ := bind_ok h1
  split at h2
  · exact absurd h2 failK_ne_ok
  · rename_i hq0; cases h2; exact ⟨p', q', hp', hq', hq0, rfl⟩

/-- Where a sum, a difference or a quotient fails: reading a side in its unit, or a division by 0. -/
theorem arithR_err {line : Nat} {op : ArOp} {ca cb : Option Conv} {p q : Rat} {k : Kind} {l : Nat}
    (h : arithR line op ca cb p q = .error (.failed k l)) :
    convN ca p = .error (.failed k l) ∨ (∃ p', convN ca p = .ok p' ∧ convN cb q = .error (.failed k l)) ∨
      (op = .div ∧ ∃ p', convN ca p = .ok p' ∧ convN cb q = .ok 0) := by
  cases op with
  | mul => simp [arithR] at h
  | add | sub =>
    simp only [arithR] at h
    rcases bind_err h with h1 | ⟨p', hp', h1⟩
    · exact Or.inl h1
    · rcases bind_err h1 with h2 | ⟨q', _, h2⟩
      · exact Or.inr (Or.inl ⟨p', hp', h2⟩)
      · cases h2
  | div =>
    simp only [arithR] at h
    rcases bind_err h with h1 | ⟨p', hp', h1⟩
    · exact Or.inl h1
    · rcases bind_err h1 with h2 | ⟨q', hq', h2⟩
      · exact Or.inr (Or.inl ⟨p', hp', h2⟩)
      · split at h2
        · rename_i hq0; subst hq0; exact Or.inr (Or.inr ⟨rfl, p', hp', hq'⟩)
        · cases h2

theorem case_arith (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {op : ArOp}
    {a b : Expr} {ca cb : Option Conv} {step : Option Rat} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.arith id line op a b ca cb step) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.arith id line op a b ca cb step)) (evI A m (.arith id line op a b ca cb step) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hgr : Grows (ev (ev A m a o).ctx m b (ev A m a o).out).out
        (evI A (m + 1) (.arith id line op a b ca cb step) o).out := by
      simp only [evI]; (repeat' split) <;> (try simp only [AR.of]) <;> gcl
    have hGb : Good C res (ev A m a o).ctx := hG.same ((sat m).ev A a o).2
    have wb := grows_not_widened hgr hw
    have hga : Grows (ev A m a o).out (ev (ev A m a o).ctx m b (ev A m a o).out).out := gEv (Grows.refl _)
    obtain ⟨ha1, ha2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened hga wb)
    have HB : ∀ x, eval C n dep ρ a = .ok x →
        Concl C ρ (eval C n dep ρ b) (ev (ev A m a o).ctx m b (ev A m a o).out) :=
      fun x hx => IH.expr m _ ρ b _ dep hGb (ha1 x hx).2.1 wb
    have Eab : ∀ k l, k.ofEffect = true → (eval C n dep ρ a = .error (.failed k l) ∨
        ∃ x, eval C n dep ρ a = .ok x ∧ eval C n dep ρ b = .error (.failed k l)) →
        Wit C (ev (ev A m a o).ctx m b (ev A m a o).out).out := by
      intro k l hk h
      rcases h with h | ⟨x, hx, h⟩
      · exact Wit.grow (ha2 k l h hk) hga
      · exact (HB x hx).2 k l h hk
    simp only [evI]
    generalize ev (ev A m a o).ctx m b (ev A m a o).out = rb at HB Eab ⊢
    generalize ev A m a o = ra at ha1 ha2 HB Eab ⊢
    split
    · -- the left never reached
      rename_i hn
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · obtain ⟨p, _, _, hp, _⟩ := arith_ok hv
        have := (ha1 _ hp).1; rw [hn] at this; exact absurd this Mem.not_never
      · rcases arith_err hv hk with h | h | ⟨p, _, hp, _⟩ | ⟨p, _, _, hp, _⟩
        · exact Eab k l hk (Or.inl h)
        · exact Eab k l hk (Or.inr h)
        · have := (ha1 _ hp).1; rw [hn] at this; exact absurd this Mem.not_never
        · have := (ha1 _ hp).1; rw [hn] at this; exact absurd this Mem.not_never
    · -- the right never reached
      rename_i hn _
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · obtain ⟨p, q, _, hp, hq, _⟩ := arith_ok hv
        have := ((HB _ hp).1 _ hq).1; rw [hn] at this; exact absurd this Mem.not_never
      · rcases arith_err hv hk with h | h | ⟨p, q, hp, hq, _⟩ | ⟨p, q, _, hp, hq, _⟩
        · exact Eab k l hk (Or.inl h)
        · exact Eab k l hk (Or.inr h)
        · have := ((HB _ hp).1 _ hq).1; rw [hn] at this; exact absurd this Mem.not_never
        · have := ((HB _ hp).1 _ hq).1; rw [hn] at this; exact absurd this Mem.not_never
    · -- two dates: the evaluation fails on its types
      rename_i x y hx _
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · obtain ⟨p, _, _, hp, _⟩ := arith_ok hv
        have := (ha1 _ hp).1; rw [hx] at this; cases this
      · rcases arith_err hv hk with h | h | ⟨p, _, hp, _⟩ | ⟨p, _, _, hp, _⟩
        · exact Eab k l hk (Or.inl h)
        · exact Eab k l hk (Or.inr h)
        · have := (ha1 _ hp).1; rw [hx] at this; cases this
        · have := (ha1 _ hp).1; rw [hx] at this; cases this
    · -- two numbers
      rename_i i j hi hj
      cases op
      · -- +
        simp only [beq_self_eq_true, ite_true]
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · obtain ⟨p, q, r, hp, hq, hr, hst, hfit, rfl⟩ := arith_ok hv
          obtain ⟨hma, _, _⟩ := ha1 _ hp
          obtain ⟨hmb, hsb, _⟩ := (HB _ hp).1 _ hq
          obtain ⟨p', q', hp', hq', rfl⟩ := arithR_add hr
          have hpa := convN_mem (o := rb.out) hma hp'
          have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
          split <;> simp only [AR.of]
          · rename_i x y hx hy
            exact ⟨heldA_mem (Mem.num (In_withDen (In_plus (Mem_iv_num hpa hx) (Mem_iv_num hpb hy))
              (denLe_stepDen hst))) hfit, hsb, fun _ h => by cases h⟩
          · exact ⟨Mem.any _, hsb, fun _ _ => trivial⟩
        · rcases arith_err hv hk with h | h | ⟨p, q, hp, hq, hr⟩ | ⟨p, q, r, hp, hq, hr, hst, hfit⟩
          · exact Wit.grow (Eab k l hk (Or.inl h)) (by split <;> simp only [AR.of] <;> gcl)
          · exact Wit.grow (Eab k l hk (Or.inr h)) (by split <;> simp only [AR.of] <;> gcl)
          · obtain ⟨hma, _, _⟩ := ha1 _ hp
            obtain ⟨hmb, _, _⟩ := (HB _ hp).1 _ hq
            rcases arithR_err hr with h1 | ⟨p', hp', h1⟩ | ⟨h1, _⟩
            · exact Wit.grow (convN_wit (o := rb.out) hma h1).2 (by split <;> simp only [AR.of] <;> gcl)
            · exact Wit.grow (convN_wit (o := (convA rb.out ca ra.abs).2) hmb h1).2
                (by split <;> simp only [AR.of] <;> gcl)
            · cases h1
          · obtain ⟨hma, _, _⟩ := ha1 _ hp
            obtain ⟨hmb, _, _⟩ := (HB _ hp).1 _ hq
            obtain ⟨p', q', hp', hq', rfl⟩ := arithR_add hr
            have hpa := convN_mem (o := rb.out) hma hp'
            have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
            split <;> simp only [AR.of]
            · rename_i x y hx hy
              exact heldA_wit (Mem.num (In_withDen (In_plus (Mem_iv_num hpa hx) (Mem_iv_num hpb hy))
                (denLe_stepDen hst))) hfit
            · exact wit_unknown
      · -- -
        simp only [show (ArOp.sub == ArOp.add) = false from rfl, Bool.false_eq_true, ite_false]
        -- the difference, with what the scope keeps of the two sides' order
        have hsub : ∀ (p q p' q' : Rat) (x y : Iv), eval C n dep ρ a = .ok (.num p) → eval C n dep ρ b = .ok (.num q) →
            Sat rb.ctx ρ → convN ca p = .ok p' → convN cb q = .ok q' → x.In p' → y.In q' →
            (∀ s, step = some s → onStep s (p' - q') = true) →
            (if (ca.isNone && cb.isNone) = true then subRel rb.ctx a b step (x.minus y) else x.minus y).In (p' - q') := by
          intro p q p' q' x y hp hq hsb hp' hq' hx hy hst
          split
          · rename_i hnn
            simp only [Bool.and_eq_true, Option.isNone_iff_eq_none] at hnn
            obtain ⟨rfl, rfl⟩ := hnn
            cases hp'; cases hq'
            exact subRel_sound hsb (symOK_of_eval hp) (symOK_of_eval hq) (In_minus hx hy)
              (fun s hs _ => hst s hs)
          · exact In_minus hx hy
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · obtain ⟨p, q, r, hp, hq, hr, hst, hfit, rfl⟩ := arith_ok hv
          obtain ⟨hma, _, _⟩ := ha1 _ hp
          obtain ⟨hmb, hsb, _⟩ := (HB _ hp).1 _ hq
          obtain ⟨p', q', hp', hq', rfl⟩ := arithR_sub hr
          have hpa := convN_mem (o := rb.out) hma hp'
          have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
          split <;> simp only [AR.of]
          · rename_i x y hx hy
            exact ⟨heldA_mem (Mem.num (In_withDen
              (hsub p q p' q' x y hp hq hsb hp' hq' (Mem_iv_num hpa hx) (Mem_iv_num hpb hy) hst)
              (denLe_stepDen hst))) hfit, hsb, fun _ h => by cases h⟩
          · exact ⟨Mem.any _, hsb, fun _ _ => trivial⟩
        · rcases arith_err hv hk with h | h | ⟨p, q, hp, hq, hr⟩ | ⟨p, q, r, hp, hq, hr, hst, hfit⟩
          · exact Wit.grow (Eab k l hk (Or.inl h)) (by split <;> simp only [AR.of] <;> gcl)
          · exact Wit.grow (Eab k l hk (Or.inr h)) (by split <;> simp only [AR.of] <;> gcl)
          · obtain ⟨hma, _, _⟩ := ha1 _ hp
            obtain ⟨hmb, _, _⟩ := (HB _ hp).1 _ hq
            rcases arithR_err hr with h1 | ⟨p', hp', h1⟩ | ⟨h1, _⟩
            · exact Wit.grow (convN_wit (o := rb.out) hma h1).2 (by split <;> simp only [AR.of] <;> gcl)
            · exact Wit.grow (convN_wit (o := (convA rb.out ca ra.abs).2) hmb h1).2
                (by split <;> simp only [AR.of] <;> gcl)
            · cases h1
          · obtain ⟨hma, hsa, _⟩ := ha1 _ hp
            obtain ⟨hmb, hsb, _⟩ := (HB _ hp).1 _ hq
            obtain ⟨p', q', hp', hq', rfl⟩ := arithR_sub hr
            have hpa := convN_mem (o := rb.out) hma hp'
            have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
            split <;> simp only [AR.of]
            · rename_i x y hx hy
              exact heldA_wit (Mem.num (In_withDen
                (hsub p q p' q' x y hp hq hsb hp' hq' (Mem_iv_num hpa hx) (Mem_iv_num hpb hy) hst)
                (denLe_stepDen hst))) hfit
            · exact wit_unknown
      · -- *
        simp only
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · obtain ⟨p, q, r, hp, hq, hr, hst, hfit, rfl⟩ := arith_ok hv
          obtain ⟨hma, _, _⟩ := ha1 _ hp
          obtain ⟨hmb, hsb, _⟩ := (HB _ hp).1 _ hq
          simp only [arithR] at hr; cases hr
          split <;> simp only [AR.of]
          · rename_i x y hx hy
            exact ⟨heldA_mem (Mem.num (In_withDen (In_times (Mem_iv_num hma hx) (Mem_iv_num hmb hy))
              (denLe_stepDen hst))) hfit, hsb, fun _ h => by cases h⟩
          · exact ⟨Mem.any _, hsb, fun _ _ => trivial⟩
        · rcases arith_err hv hk with h | h | ⟨p, q, hp, hq, hr⟩ | ⟨p, q, r, hp, hq, hr, hst, hfit⟩
          · exact Wit.grow (Eab k l hk (Or.inl h)) (by split <;> simp only [AR.of] <;> gcl)
          · exact Wit.grow (Eab k l hk (Or.inr h)) (by split <;> simp only [AR.of] <;> gcl)
          · simp [arithR] at hr
          · obtain ⟨hma, _, _⟩ := ha1 _ hp
            obtain ⟨hmb, _, _⟩ := (HB _ hp).1 _ hq
            simp only [arithR] at hr; cases hr
            split <;> simp only [AR.of]
            · rename_i x y hx hy
              exact heldA_wit (Mem.num (In_withDen (In_times (Mem_iv_num hma hx) (Mem_iv_num hmb hy))
                (denLe_stepDen hst))) hfit
            · exact wit_unknown
      · -- /
        simp only
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · obtain ⟨p, q, r, hp, hq, hr, hst, hfit, rfl⟩ := arith_ok hv
          obtain ⟨hma, _, _⟩ := ha1 _ hp
          obtain ⟨hmb, hsb, _⟩ := (HB _ hp).1 _ hq
          obtain ⟨p', q', hp', hq', hq0, rfl⟩ := arithR_div hr
          have hpa := convN_mem (o := rb.out) hma hp'
          have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
          split <;> simp only [AR.of]
          · rename_i x y hx hy
            split <;> simp only []
            · rename_i h0
              simp only [beq_iff_eq] at h0
              exact absurd (isExact_sound (Mem_iv_num hpb hy) h0) hq0
            · exact ⟨heldA_mem (Mem.num (In_withDen (In_over (Mem_iv_num hpa hx) (Mem_iv_num hpb hy) hq0)
                (denLe_stepDen hst))) hfit, hsb, fun _ h => by cases h⟩
          · exact ⟨Mem.any _, hsb, fun _ _ => trivial⟩
        · rcases arith_err hv hk with h | h | ⟨p, q, hp, hq, hr⟩ | ⟨p, q, r, hp, hq, hr, hst, hfit⟩
          · exact Wit.grow (Eab k l hk (Or.inl h)) (by (repeat' split) <;> simp only [AR.of] <;> gcl)
          · exact Wit.grow (Eab k l hk (Or.inr h)) (by (repeat' split) <;> simp only [AR.of] <;> gcl)
          · obtain ⟨hma, _, _⟩ := ha1 _ hp
            obtain ⟨hmb, _, _⟩ := (HB _ hp).1 _ hq
            rcases arithR_err hr with h1 | ⟨p', hp', h1⟩ | ⟨_, p', hp', hq'⟩
            · exact Wit.grow (convN_wit (o := rb.out) hma h1).2 (by (repeat' split) <;> simp only [AR.of] <;> gcl)
            · exact Wit.grow (convN_wit (o := (convA rb.out ca ra.abs).2) hmb h1).2
                (by (repeat' split) <;> simp only [AR.of] <;> gcl)
            · -- a division by 0: the place for 0 takes it
              have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
              split
              · split <;> simp only [AR.of]
                · exact wit_place (what := .zero) hpb rfl
                · exact Wit.grow (wit_place (what := .zero) hpb rfl) (grows_heldA (Grows.refl _))
              · exact wit_unknown
          · obtain ⟨hma, _, _⟩ := ha1 _ hp
            obtain ⟨hmb, _, _⟩ := (HB _ hp).1 _ hq
            obtain ⟨p', q', hp', hq', hq0, rfl⟩ := arithR_div hr
            have hpa := convN_mem (o := rb.out) hma hp'
            have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
            split <;> simp only [AR.of]
            · rename_i x y hx hy
              split <;> simp only []
              · rename_i h0
                simp only [beq_iff_eq] at h0
                exact absurd (isExact_sound (Mem_iv_num hpb hy) h0) hq0
              · exact heldA_wit (Mem.num (In_withDen (In_over (Mem_iv_num hpa hx) (Mem_iv_num hpb hy) hq0)
                  (denLe_stepDen hst))) hfit
            · exact wit_unknown
    · -- anything else: the place for what the analysis does not know
      refine ⟨fun v hv => ?_, fun k l hv hk => wit_unknown⟩
      obtain ⟨p, q, _, hp, hq, _⟩ := arith_ok hv
      exact ⟨Mem.any _, ((HB _ hp).1 _ hq).2.1, fun _ _ => trivial⟩


-- ── The days between two dates ───────────────────────────────────────────────

theorem case_ddiff (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {a b : Expr}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.ddiff id a b) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.ddiff id a b)) (evI A m (.ddiff id a b) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hgr : Grows (ev (ev A m a o).ctx m b (ev A m a o).out).out (evI A (m + 1) (.ddiff id a b) o).out := by
      simp only [evI]; (repeat' split) <;> (try simp only [AR.of]) <;> gcl
    have hGb : Good C res (ev A m a o).ctx := hG.same ((sat m).ev A a o).2
    have wb := grows_not_widened hgr hw
    have hga : Grows (ev A m a o).out (ev (ev A m a o).ctx m b (ev A m a o).out).out := gEv (Grows.refl _)
    obtain ⟨ha1, ha2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened hga wb)
    have HB : ∀ x, eval C n dep ρ a = .ok x →
        Concl C ρ (eval C n dep ρ b) (ev (ev A m a o).ctx m b (ev A m a o).out) :=
      fun x hx => IH.expr m _ ρ b _ dep hGb (ha1 x hx).2.1 wb
    simp only [evI]
    generalize ev (ev A m a o).ctx m b (ev A m a o).out = rb at HB hga ⊢
    generalize ev A m a o = ra at ha1 ha2 hga HB ⊢
    -- the evaluation: two dates, or a failure
    have hok : ∀ v, eval C (n + 1) dep ρ (.ddiff id a b) = .ok v → ∃ p q, eval C n dep ρ a = .ok (.date p) ∧
        eval C n dep ρ b = .ok (.date q) ∧ v = .num ((p : Rat) - (q : Rat)) := by
      intro v hv
      simp only [eval] at hv
      obtain ⟨x, hx, h1⟩ := bind_ok hv
      obtain ⟨y, hy, h2⟩ := bind_ok h1
      cases x <;> cases y <;> simp only [failK, reduceCtorEq] at h2
      cases h2; exact ⟨_, _, hx, hy, rfl⟩
    have herr : ∀ k l, eval C (n + 1) dep ρ (.ddiff id a b) = .error (.failed k l) → k.ofEffect = true →
        Wit C rb.out := by
      intro k l hv hk
      simp only [eval] at hv
      rcases bind_err hv with h | ⟨x, hx, h1⟩
      · exact Wit.grow (ha2 k l h hk) hga
      · rcases bind_err h1 with h2 | ⟨y, _, h2⟩
        · exact (HB x hx).2 k l h2 hk
        · cases x <;> cases y <;> simp only at h2 <;> first | (rw [fail_type h2] at hk; cases hk) | cases h2
    split
    · rename_i hn
      refine ⟨fun v hv => ?_, fun k l hv hk => herr k l hv hk⟩
      obtain ⟨p, _, hp, _⟩ := hok v hv
      have := (ha1 _ hp).1; rw [hn] at this; exact absurd this Mem.not_never
    · rename_i hn _
      refine ⟨fun v hv => ?_, fun k l hv hk => herr k l hv hk⟩
      obtain ⟨p, q, hp, hq, _⟩ := hok v hv
      have := ((HB _ hp).1 _ hq).1; rw [hn] at this; exact absurd this Mem.not_never
    · rename_i x y hx hy
      refine ⟨fun v hv => ?_, fun k l hv hk => herr k l hv hk⟩
      obtain ⟨p, q, hp, hq, rfl⟩ := hok v hv
      have hma := (ha1 _ hp).1
      obtain ⟨hmb, hsb, _⟩ := (HB _ hp).1 _ hq
      rw [hx] at hma; rw [hy] at hmb
      cases hma with
      | date h1 =>
        cases hmb with
        | date h2 =>
          refine ⟨Mem.num (In_withDen ?_ ?_), hsb, fun _ h => by cases h⟩
          · exact In_minus h1 h2
          · exact Nat.le_of_eq (by rw [← Rat.intCast_sub, Rat.den_intCast])
    · refine ⟨fun v hv => ?_, fun k l hv hk => herr k l hv hk⟩
      obtain ⟨p, q, hp, hq, _⟩ := hok v hv
      exact ⟨Mem.any _, ((HB _ hp).1 _ hq).2.1, fun _ _ => trivial⟩

-- ── Comparisons ──────────────────────────────────────────────────────────────

theorem atMost_sound {x y : Iv} {p q : Rat} (hx : x.In p) (hy : y.In q) (h : x.atMost y = true) : p ≤ q := by
  unfold Iv.atMost at h
  split at h
  · rename_i u l hu hl
    have h1 := hx.hi; have h2 := hy.lo
    rw [hu] at h1; rw [hl] at h2
    simp only [EndGe, EndLe] at h1 h2
    simp only [decide_eq_true_eq] at h
    exact Rat.le_trans h1 (Rat.le_trans h h2)
  · cases h

theorem sameAs_sound {x y : Iv} {p q : Rat} (hx : x.In p) (hy : y.In q) (h : x.sameAs y = true) : p = q := by
  unfold Iv.sameAs at h
  split at h
  · rename_i u w hu hw
    simp only [beq_iff_eq] at h
    rw [isExact_sound hx hu, isExact_sound hy hw, h]
  · cases h

/-- **A comparison of two intervals that is decided comes out as decided.** -/
theorem decide'_sound {x y : Iv} {p q : Rat} {op : CmpOp} {d t : Bool} (hx : x.In p) (hy : y.In q)
    (hd : decide' x op y = some d) (ht : OpRel (if t then op else op.negate) p q) : t = d := by
  have hlt := below_sound hx hy
  have hgt := below_sound hy hx
  have hle := atMost_sound hx hy
  have hge := atMost_sound hy hx
  have hsm := sameAs_sound hx hy
  unfold decide' at hd
  cases op <;> simp only at hd <;> (repeat' split at hd) <;> (try cases hd) <;> cases t <;>
    simp only [OpRel, CmpOp.negate, ite_true, ite_false, Bool.false_eq_true] at ht ⊢ <;>
    first | rfl | (exfalso; grind)

theorem cmpVals_bool {line : Nat} {op : CmpOp} {ca cb : Option Conv} {ord : Order} {x y v : Val}
    (h : cmpVals line op ca cb ord x y = .ok v) : ∃ t, v = .bool t := by
  unfold cmpVals at h
  split at h
  · obtain ⟨p', _, h1⟩ := bind_ok h
    obtain ⟨q', _, h2⟩ := bind_ok h1
    cases h2; exact ⟨_, rfl⟩
  · cases h; exact ⟨_, rfl⟩
  · cases h; exact ⟨_, rfl⟩
  · split at h <;> (cases h; exact ⟨_, rfl⟩)
  · exact absurd h failK_ne_ok
  · exact absurd h failK_ne_ok
  · split at h
    · obtain ⟨_, _, h1⟩ := bind_ok h
      obtain ⟨_, _, h2⟩ := bind_ok h1
      cases h2; exact ⟨_, rfl⟩
    · obtain ⟨_, _, h1⟩ := bind_ok h
      obtain ⟨_, _, h2⟩ := bind_ok h1
      cases h2; exact ⟨_, rfl⟩
    · cases h; exact ⟨_, rfl⟩

theorem convN_err_coerce {c : Option Conv} {p : Rat} {e : Stop} (h : convN c p = .error e) :
    coerce c (.num p) = .error e := by
  cases c with
  | none => simp [convN] at h
  | some c => simp only [coerce, coerceV, Functor.map, Except.map, h]

theorem cmpVals_err {line : Nat} {op : CmpOp} {ca cb : Option Conv} {ord : Order} {x y : Val} {k : Kind} {l : Nat}
    (h : cmpVals line op ca cb ord x y = .error (.failed k l)) (hk : k.ofEffect = true) :
    coerce ca x = .error (.failed k l) ∨ ∃ x', coerce ca x = .ok x' ∧ coerce cb y = .error (.failed k l) := by
  unfold cmpVals at h
  split at h
  · rcases bind_err h with h1 | ⟨p', hp', h1⟩
    · exact Or.inl (convN_err_coerce h1)
    · rcases bind_err h1 with h2 | ⟨q', _, h2⟩
      · exact Or.inr ⟨_, convN_coerce hp', convN_err_coerce h2⟩
      · cases h2
  · cases h
  · cases h
  · split at h <;> cases h
  · rw [fail_type h] at hk; cases hk
  · rw [fail_type h] at hk; cases hk
  · split at h
    · rcases bind_err h with h1 | ⟨x', hx', h1⟩
      · exact Or.inl h1
      · rcases bind_err h1 with h2 | ⟨_, _, h2⟩
        · exact Or.inr ⟨x', hx', h2⟩
        · cases h2
    · rcases bind_err h with h1 | ⟨x', hx', h1⟩
      · exact Or.inl h1
      · rcases bind_err h1 with h2 | ⟨_, _, h2⟩
        · exact Or.inr ⟨x', hx', h2⟩
        · cases h2
    · cases h

/-- **Two values of enums or bools compared for equality**, as the sets they are in decide. -/
theorem decideSet_sound {xs ys : List String} {v1 v2 : Val} {op : CmpOp} {t d : Bool} {line : Nat}
    {ca cb : Option Conv} {ord : Order} (h1 : Mem (.set (some xs)) v1) (h2 : Mem (.set (some ys)) v2)
    (hc : cmpVals line op ca cb ord v1 v2 = .ok (.bool t))
    (hd : (if (op == .eq || op == .ne) = true then
            if (xs.length == 1 && ys.length == 1) = true then some ((xs == ys) == (op == .eq))
            else if xs.all (fun v => !ys.contains v) = true then some (op == .ne) else none
          else none) = some d) : t = d := by
  obtain ⟨s1, hs1, hm1⟩ := Mem_set_str h1
  obtain ⟨s2, hs2, hm2⟩ := Mem_set_str h2
  have hin1 := hm1 xs rfl
  have hin2 := hm2 ys rfl
  obtain ⟨heq, hne⟩ := cmp_set hc (str_kind hs1) (str_kind hs2)
  have hmix : ∀ e b, ¬ (v1 = .enum e ∧ v2 = .bool b) ∧ ¬ (v1 = .bool b ∧ v2 = .enum e) := by
    intro e b
    constructor <;> rintro ⟨rfl, rfl⟩ <;> simp [cmpVals, failK] at hc
  -- two values of one kind are the same where their words are
  have same_of : v1 = v2 → s1 = s2 := by intro h; subst h; rw [hs1] at hs2; cases hs2; rfl
  have ne_of : v1 ≠ v2 → s1 ≠ s2 := fun h => strOf_ne hs1 hs2 h hmix
  split at hd
  · rename_i hop
    simp only [Bool.or_eq_true, beq_iff_eq] at hop
    split at hd
    · rename_i hlen
      simp only [Bool.and_eq_true, beq_iff_eq, List.length_eq_one_iff] at hlen
      obtain ⟨⟨a, rfl⟩, ⟨b, rfl⟩⟩ := hlen
      simp only [List.mem_singleton] at hin1 hin2
      subst hin1 hin2
      cases hd
      have beq_false : s1 ≠ s2 → ([s1] == [s2]) = false := fun h => by
        cases hb : ([s1] == [s2])
        · rfl
        · simp at hb; exact absurd hb h
      have beq_true : s1 = s2 → ([s1] == [s2]) = true := fun h => by subst h; simp
      rcases hop with rfl | rfl <;> cases t <;> simp only [CmpOp.negate, ite_true, ite_false, Bool.false_eq_true,
        forall_const, reduceCtorEq, false_implies] at heq hne
      · rw [beq_false (ne_of hne)]; rfl
      · rw [beq_true (same_of heq)]; rfl
      · rw [beq_true (same_of heq)]; rfl
      · rw [beq_false (ne_of hne)]; rfl
    · split at hd
      · rename_i _ hdis
        cases hd
        have hns : s1 ≠ s2 := by
          intro e; subst e
          have := List.all_eq_true.mp hdis s1 hin1
          simp [] at this
          exact this hin2
        rcases hop with rfl | rfl <;> cases t <;> simp only [CmpOp.negate, ite_true, ite_false, Bool.false_eq_true,
          forall_const, reduceCtorEq, false_implies] at heq hne
        · rfl
        · exact absurd (same_of heq) hns
        · exact absurd (same_of heq) hns
        · rfl
      · cases hd
  · cases hd

/-- A value of an enum that has fields is one with a value of an enum with no fields there: its name
    alone. -/
theorem Mem_variants_nofields {t : String} {v : Val} (h : Mem (.variants [(t, [])]) v) : v = .enum t := by
  cases h with
  | variantEnum hs =>
    rename_i x
    by_cases hx : t = x
    · subst hx; rfl
    · have : (t == x) = false := by simp [hx]
      simp [Abs.recOf, List.find?, this] at hs
  | @variant _ x r fs hr hne hsome _ =>
    exfalso
    cases fs with
    | nil => exact hne rfl
    | cons p rest =>
      obtain ⟨n, y⟩ := p
      have hfy : fieldV ((n, y) :: rest) n = some y := by simp [fieldV]
      have hs := hsome n y hfy
      by_cases hx : t = x
      · subst hx
        simp [Abs.recOf, List.find?] at hr
        subst hr
        simp [Abs.fieldOf] at hs
      · have : (t == x) = false := by simp [hx]
        simp [Abs.recOf, List.find?, this] at hr

theorem coerce_enum (cv : Option Conv) (x : String) : coerce cv (.enum x) = .ok (.enum x) := by
  cases cv <;> rfl

theorem coerce_variant (cv : Option Conv) (x : String) (fs : List (String × Val)) :
    coerce cv (.variant x fs) = .ok (.variant x fs) := by
  cases cv <;> rfl

/-- Two values of an enum compared for equality: whether they are the same, by `==`. -/
theorem cmp_tags {line : Nat} {op : CmpOp} {ca cb : Option Conv} {ord : Order} {x y : Val} {t : Bool}
    (hx : ∃ a, x.tag? = some a) (hy : ∃ b, y.tag? = some b) (hop : op = .eq ∨ op = .ne)
    (h : cmpVals line op ca cb ord x y = .ok (.bool t)) : t = (if op = .eq then (x == y) else !(x == y)) := by
  obtain ⟨a, ha⟩ := hx
  obtain ⟨b, hb⟩ := hy
  cases x <;> simp [Val.tag?] at ha <;> cases y <;> simp [Val.tag?] at hb <;>
    rcases hop with rfl | rfl <;>
    simp only [cmpVals, coerce_enum, coerce_variant, bind, Except.bind, Except.ok.injEq, Val.bool.injEq] at h <;>
    subst h <;> simp [BEq.beq, Val.beq, bne]

/-- **Two values of an enum that has fields compared for equality**, as the values they can be
    decide. -/
theorem decideVariants_sound {xs ys : List (String × List (String × Abs))} {v1 v2 : Val} {op : CmpOp} {t d : Bool}
    {line : Nat} {ca cb : Option Conv} {ord : Order} (h1 : Mem (.variants xs) v1) (h2 : Mem (.variants ys) v2)
    (hc : cmpVals line op ca cb ord v1 v2 = .ok (.bool t))
    (hd : (if (op == .eq || op == .ne) = true then
            if (xs.all fun k => !(ys.any (·.1 == k.1))) = true then some (op == .ne)
            else if (xs.length == 1 && xs.map (·.1) == ys.map (·.1) && (xs ++ ys).all (fun r => r.2.isEmpty)) = true then
              some (op == .eq)
            else none
          else none) = some d) : t = d := by
  obtain ⟨t1, ht1, hs1⟩ := Mem_variants_tag h1
  obtain ⟨t2, ht2, hs2⟩ := Mem_variants_tag h2
  split at hd
  · rename_i hop
    simp only [Bool.or_eq_true, beq_iff_eq] at hop
    have hct := cmp_tags ⟨_, ht1⟩ ⟨_, ht2⟩ hop hc
    split at hd
    · -- no value the same
      rename_i hdis
      cases hd
      have hne : t1 ≠ t2 := by
        intro e; subst e
        obtain ⟨r, hr⟩ := Option.isSome_iff_exists.mp hs1
        have := List.all_eq_true.mp hdis _ (recOf_mem hr)
        obtain ⟨r2, hr2⟩ := Option.isSome_iff_exists.mp hs2
        simp only [Bool.not_eq_true', List.any_eq_false, beq_iff_eq] at this
        exact this _ (recOf_mem hr2) rfl
      have hbeq : (v1 == v2) = false := by
        cases v1 <;> simp [Val.tag?] at ht1 <;> cases v2 <;> simp [Val.tag?] at ht2 <;>
          subst ht1 ht2 <;> simp [BEq.beq, Val.beq, hne]
      rcases hop with rfl | rfl <;> simp [hct, hbeq]
    · split at hd
      · -- one value, with no fields, on both sides
        rename_i _ hone
        cases hd
        simp only [Bool.and_eq_true, beq_iff_eq, List.length_eq_one_iff, List.all_eq_true] at hone
        obtain ⟨⟨⟨⟨k, r⟩, rfl⟩, hmap⟩, hemp⟩ := hone
        have hr : r = [] := by simpa using hemp (k, r) (by simp)
        subst hr
        obtain ⟨⟨k', r'⟩, rfl⟩ : ∃ q, ys = [q] := by
          cases ys with
          | nil => simp at hmap
          | cons q rest => cases rest with
            | nil => exact ⟨q, rfl⟩
            | cons _ _ => simp at hmap
        simp at hmap; subst hmap
        have hr' : r' = [] := by simpa using hemp (k, r') (by simp)
        subst hr'
        rw [Mem_variants_nofields h1, Mem_variants_nofields h2] at hct
        rcases hop with rfl | rfl <;> simp [hct, BEq.beq, Val.beq]
      · cases hd
  · cases hd

theorem case_cmp (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {op : CmpOp}
    {a b : Expr} {ca cb : Option Conv} {ord : Order} {sa sb : Option Rat} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.cmp id line op a b ca cb ord sa sb) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.cmp id line op a b ca cb ord sa sb))
      (evI A m (.cmp id line op a b ca cb ord sa sb) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    have hGb : Good C res (ev A m a o).ctx := hG.same ((sat m).ev A a o).2
    have wb : (ev (ev A m a o).ctx m b (ev A m a o).out).out.widened = false :=
      grows_not_widened (grows_convA (grows_convA (Grows.refl _))) hw
    have hga : Grows (ev A m a o).out (ev (ev A m a o).ctx m b (ev A m a o).out).out := gEv (Grows.refl _)
    obtain ⟨ha1, ha2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened hga wb)
    have HB : ∀ x, eval C n dep ρ a = .ok x →
        Concl C ρ (eval C n dep ρ b) (ev (ev A m a o).ctx m b (ev A m a o).out) :=
      fun x hx => IH.expr m _ ρ b _ dep hGb (ha1 x hx).2.1 wb
    generalize ev (ev A m a o).ctx m b (ev A m a o).out = rb at HB hga ⊢
    generalize ev A m a o = ra at ha1 ha2 hga HB ⊢
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨x, hx, h1⟩ := bind_ok hv
      obtain ⟨y, hy, h2⟩ := bind_ok h1
      obtain ⟨u1, hu1, h3⟩ := bind_ok h2
      obtain ⟨u2, hu2, hc⟩ := bind_ok h3
      obtain ⟨hma, _, _⟩ := ha1 x hx
      obtain ⟨hmb, hsb, _⟩ := (HB x hx).1 y hy
      obtain ⟨t, rfl⟩ := cmpVals_bool hc
      refine ⟨?_, hsb, fun t' ht' => ?_⟩
      · split
        · rename_i d hd
          -- what the comparison decides, it comes out as
          have hdt : t = d := by
            split at hd
            · rename_i i j hi hj
              obtain ⟨i0, hi0⟩ := scaleBy_num (convA_fst ▸ hi)
              obtain ⟨j0, hj0⟩ := scaleBy_num (convA_fst ▸ hj)
              have hma' := hma; have hmb' := hmb
              rw [hi0] at hma'; rw [hj0] at hmb'
              cases hma' with
              | num _ =>
                cases hmb' with
                | num _ =>
                  obtain ⟨p', q', hp', hq', hrel⟩ := cmp_num hc
                  have hpa := convN_mem (o := rb.out) hma hp'
                  have hpb := convN_mem (o := (convA rb.out ca ra.abs).2) hmb hq'
                  rw [hi] at hpa; rw [hj] at hpb
                  cases hpa with
                  | @num _ _ hx1 => cases hpb with
                    | @num _ _ hy1 => exact decide'_sound hx1 hy1 hd hrel
            · rename_i i j hi hj
              have hi0 := scaleBy_date (convA_fst ▸ hi)
              have hj0 := scaleBy_date (convA_fst ▸ hj)
              rw [hi0] at hma; rw [hj0] at hmb
              cases hma with
              | @date _ _ hx1 => cases hmb with
                | @date _ _ hy1 => exact decide'_sound hx1 hy1 hd (cmp_date hc)
            · rename_i xs ys hi hj
              have hi0 := scaleBy_set (convA_fst ▸ hi)
              have hj0 := scaleBy_set (convA_fst ▸ hj)
              rw [hi0] at hma; rw [hj0] at hmb
              exact decideSet_sound hma hmb hc hd
            · rename_i xs ys hi hj
              have hi0 := scaleBy_variants (convA_fst ▸ hi)
              have hj0 := scaleBy_variants (convA_fst ▸ hj)
              rw [hi0] at hma; rw [hj0] at hmb
              exact decideVariants_sound hma hmb hc hd
            · cases hd
          subst hdt
          exact Mem_bools (fun h => h) (fun h => by simp [h])
        · exact Mem_allBools t
      · cases ht'
        exact ⟨x, y, line, ord, symOK_of_eval hx, symOK_of_eval hy, hma, hmb, stepOK_of_stepped hu1,
          stepOK_of_stepped hu2, hc⟩
    · simp only [eval] at hv
      rcases bind_err hv with h | ⟨x, hx, h1⟩
      · exact Wit.grow (ha2 k l h hk) (grows_convA (grows_convA hga))
      · rcases bind_err h1 with h2 | ⟨y, hy, h2⟩
        · exact Wit.grow ((HB x hx).2 k l h2 hk) (grows_convA (grows_convA (Grows.refl _)))
        · rcases bind_err h2 with h3 | ⟨u1, _, h3⟩
          · rw [type_noEffect (stepped_err h3)] at hk; cases hk
          · rcases bind_err h3 with h4 | ⟨u2, _, h4⟩
            · rw [type_noEffect (stepped_err h4)] at hk; cases hk
            · obtain ⟨hma, _, _⟩ := ha1 x hx
              obtain ⟨hmb, _, _⟩ := (HB x hx).1 y hy
              rcases cmpVals_err h4 hk with h5 | ⟨x', _, h5⟩
              · exact Wit.grow (coerce_wit hma h5).2 (grows_convA (Grows.refl _))
              · exact (coerce_wit hmb h5).2


-- ── `min` and `max` ──────────────────────────────────────────────────────────

theorem endLe_trans {e : End} {p v : Rat} (h : EndLe e p) (hv : p ≤ v) : EndLe e v := by
  cases e with
  | none => trivial
  | some l => exact Rat.le_trans h hv

theorem endGe_trans {e : End} {p v : Rat} (h : EndGe e p) (hv : v ≤ p) : EndGe e v := by
  cases e with
  | none => trivial
  | some u => exact Rat.le_trans hv h

theorem In_ivD {a : Abs} {p : Rat} (h : Mem a (.num p)) : (a.iv.getD Iv.top).In p := by
  cases hi : a.iv with
  | none => exact ⟨trivial, trivial, trivial⟩
  | some i => exact Mem_iv_num h hi

theorem numOf_ok {v : Val} {line : Nat} {p : Rat} (h : numOf v line = .ok p) : v = .num p := by
  cases v <;> simp [numOf, failK] at h
  rw [h]

theorem ite_or {α : Type} {c : Prop} [Decidable c] {a b : α} : (if c then a else b) = a ∨ (if c then a else b) = b := by
  split
  · exact Or.inl rfl
  · exact Or.inr rfl

/-- **`min` and `max` of two values in two intervals** are in the interval of the ends. -/
theorem minMax_sound {x y : Iv} {p q : Rat} (isMin : Bool) (hx : x.In p) (hy : y.In q) :
    (minMaxIv isMin x y).In (if (if isMin then cmpRat p q != .gt else cmpRat p q != .lt) then p else q) := by
  generalize hv : (if (if isMin then cmpRat p q != .gt else cmpRat p q != .lt) then p else q) = v
  have hh : (x.hull y).In v := by
    have : v = p ∨ v = q := hv ▸ ite_or
    rcases this with rfl | rfl
    · exact In_hull_left hx
    · exact In_hull_right hy
  unfold minMaxIv
  cases isMin
  · have hge : p ≤ v ∧ q ≤ v := by
      rw [← hv]
      by_cases h1 : p < q
      · simp [cmpRat, h1]; exact Rat.le_of_lt h1
      · have h1' : q ≤ p := Rat.not_lt.mp h1
        by_cases h2 : q < p <;> simp [cmpRat, h1, h2, h1']
    exact In_mk' (maxLo_le (endLe_trans hx.lo hge.1) (endLe_trans hy.lo hge.2)) hh.hi hh.den
  · have hle : v ≤ p ∧ v ≤ q := by
      rw [← hv]
      by_cases h1 : p < q
      · simp [cmpRat, h1]; exact Rat.le_of_lt h1
      · have h1' : q ≤ p := Rat.not_lt.mp h1
        by_cases h2 : q < p
        · simp [cmpRat, h1, h2]; exact h1'
        · have : p = q := Rat.le_antisymm (Rat.not_lt.mp h2) h1'
          subst this; simp [cmpRat]
    exact In_mk' hh.lo (minHi_ge (endGe_trans hx.hi hle.1) (endGe_trans hy.hi hle.2)) hh.den

theorem mem_ite {a : Abs} {c : Prop} [Decidable c] {x y : Val} (hx : Mem a x) (hy : Mem a y) :
    Mem a (if c then x else y) := by
  split
  · exact hx
  · exact hy

theorem ite_num {c : Bool} {p q : Rat} : (if c = true then Val.num p else Val.num q) = .num (if c = true then p else q) := by
  split <;> rfl

theorem ite_date {c : Bool} {p q : Int} :
    (if c = true then Val.date p else Val.date q) = .date (if c = true then p else q) := by
  split <;> rfl

theorem case_minmax (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {isMin : Bool}
    {a b : Expr} {ca cb : Option Conv} {ord : Order} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.minmax id line isMin a b ca cb ord) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.minmax id line isMin a b ca cb ord)) (evI A m (.minmax id line isMin a b ca cb ord) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGb : Good C res (ev A m a o).ctx := hG.same ((sat m).ev A a o).2
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hn
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨x, hx, _⟩ := bind_ok hv
        have := (h1 x hx).1; rw [hn] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨x, hx, _⟩
        · exact h2 k l hv hk
        · have := (h1 x hx).1; rw [hn] at this; exact absurd this Mem.not_never
    · rename_i hne
      have wb : (ev (ev A m a o).ctx m b (ev A m a o).out).out.widened = false := by
        split at hw
        · exact hw
        · simp only [AR.of] at hw; exact grows_not_widened (grows_convA (grows_convA (Grows.refl _))) hw
      obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened (gEv (Grows.refl _)) wb)
      have HB : ∀ x, eval C n dep ρ a = .ok x →
          Concl C ρ (eval C n dep ρ b) (ev (ev A m a o).ctx m b (ev A m a o).out) :=
        fun x hx => IH.expr m _ ρ b _ dep hGb (h1 x hx).2.1 wb
      have hga : Grows (ev A m a o).out (ev (ev A m a o).ctx m b (ev A m a o).out).out := gEv (Grows.refl _)
      split at hw
      · rename_i hn
        simp only [AR.of] at hw ⊢
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨x, hx, h3⟩ := bind_ok hv
          obtain ⟨y, hy, _⟩ := bind_ok h3
          have := ((HB x hx).1 y hy).1; rw [hn] at this; exact absurd this Mem.not_never
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨x, hx, h3⟩
          · exact Wit.grow (h2 k l hv hk) hga
          · rcases bind_err h3 with h4 | ⟨y, hy, _⟩
            · exact (HB x hx).2 k l h4 hk
            · have := ((HB x hx).1 y hy).1; rw [hn] at this; exact absurd this Mem.not_never
      · simp only [AR.of] at hw ⊢
        generalize ev (ev A m a o).ctx m b (ev A m a o).out = rb at HB hga ⊢
        generalize ev A m a o = ra at h1 h2 hga HB ⊢
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨x, hx, h3⟩ := bind_ok hv
          obtain ⟨y, hy, h4⟩ := bind_ok h3
          obtain ⟨x', hx', h5⟩ := bind_ok h4
          obtain ⟨y', hy', h6⟩ := bind_ok h5
          cases h6
          obtain ⟨hma, _, _⟩ := h1 x hx
          obtain ⟨hmb, hsb, _⟩ := (HB x hx).1 y hy
          have hpa := convA_mem (o := rb.out) hma hx'
          have hpb := convA_mem (o := (convA rb.out ca ra.abs).2) hmb hy'
          refine ⟨?_, hsb, fun _ _ => trivial⟩
          split
          · rename_i i j hi hj
            rw [hi] at hpa; rw [hj] at hpb
            cases hpa with
            | @num _ p hp1 =>
              cases hpb with
              | @num _ q hq1 =>
                simp only [ite_num]
                exact Mem.num (minMax_sound isMin hp1 hq1)
          · rename_i i j hi hj
            rw [hi] at hpa; rw [hj] at hpb
            cases hpa with
            | @date _ p hp1 =>
              cases hpb with
              | @date _ q hq1 =>
                simp only [ite_date, cmpInt_cast]
                refine Mem.date ?_
                have e : ∀ c : Bool, (((if c = true then p else q : Int)) : Rat) =
                    if c = true then (p : Rat) else (q : Rat) := by
                  intro c; split <;> rfl
                rw [e]
                exact minMax_sound isMin hp1 hq1
          · exact mem_ite (mem_join_left hpa) (mem_join_right hpb)
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨x, hx, h3⟩
          · exact Wit.grow (h2 k l hv hk) (grows_convA (grows_convA hga))
          · rcases bind_err h3 with h4 | ⟨y, hy, h4⟩
            · exact Wit.grow ((HB x hx).2 k l h4 hk) (grows_convA (grows_convA (Grows.refl _)))
            · obtain ⟨hma, _, _⟩ := h1 x hx
              obtain ⟨hmb, _, _⟩ := (HB x hx).1 y hy
              rcases bind_err h4 with h5 | ⟨x', _, h5⟩
              · exact Wit.grow (coerce_wit (o := rb.out) hma h5).2 (grows_convA (Grows.refl _))
              · rcases bind_err h5 with h6 | ⟨y', _, h6⟩
                · exact (coerce_wit hmb h6).2
                · cases h6

-- ── `mod` ────────────────────────────────────────────────────────────────────

theorem modN_ints {x : Rat} {k : Int} (hk : 0 < k) : (Iv.ints 0 (k - 1)).In (modN x k) := by
  unfold modN
  apply In_ints
  · exact Int.emod_nonneg _ (by omega)
  · have := Int.emod_lt_of_pos x.floor hk; omega

theorem modN_small {x l h : Rat} {k : Int} (hl : l ≤ x) (hh : x ≤ h) (hl0 : 0 ≤ l) (hhk : h < (k : Rat)) :
    (Iv.ints 0 h.floor).In (modN x k) := by
  unfold modN
  have h0 : (0 : Int) ≤ x.floor := Rat.le_floor_iff.mpr (by simp; exact Rat.le_trans hl0 hl)
  have hfx : (x.floor : Rat) ≤ x := Rat.floor_le x
  have hlt : x.floor < k := by
    have h1 := Rat.le_trans hfx hh
    have : (x.floor : Rat) < (k : Rat) := by grind
    exact Rat.intCast_lt_intCast.mp this
  rw [Int.emod_eq_of_lt h0 hlt]
  apply In_ints h0
  exact Rat.le_floor_iff.mpr (Rat.le_trans hfx hh)

/-- The size of a number as the analysis writes it is its magnitude. -/
theorem magN_eq (v : Rat) : (if v.num < 0 then -v else v) = mag v := by
  unfold mag
  by_cases h : v < 0
  · have : v.num < 0 := (num_neg_iff v).mpr h
    simp [h, this]
  · have : ¬ v.num < 0 := fun h' => h ((num_neg_iff v).mp h')
    simp [h, this]

theorem rat_of_den_one {r : Rat} (h : r.den = 1) : r = (r.num : Rat) := by
  have := rat_eq_num_div_den r
  rw [h] at this
  calc r = (r.num : Rat) / ((1 : Nat) : Rat) := this
    _ = (r.num : Rat) := by rw [show ((1 : Nat) : Rat) = 1 from rfl]; grind

/-- The remainder by a whole number that is not 0, at most one less than the size of the ends the
    number is between (Euclid's: whatever the signs). -/
theorem modN_le_size {x l h : Rat} {z : Int} (hz : z ≠ 0) (hl : l ≤ (z : Rat)) (hh : (z : Rat) ≤ h) :
    ((x.floor % z : Int) : Rat) ≤ (if mag h < mag l then mag l else mag h) - 1 := by
  have hs1 : mag l ≤ (if mag h < mag l then mag l else mag h) := by split <;> grind
  have hs2 : mag h ≤ (if mag h < mag l then mag l else mag h) := by split <;> grind
  rcases Int.lt_or_gt_of_ne hz with hneg | hpos
  · have h1 : x.floor % (-z) = x.floor % z := Int.emod_neg _ _
    have h2 : x.floor % (-z) < -z := Int.emod_lt_of_pos _ (by omega)
    have h3 : x.floor % z ≤ -z - 1 := by omega
    have h4 : ((x.floor % z : Int) : Rat) ≤ ((-z - 1 : Int) : Rat) := Rat.intCast_le_intCast.mpr h3
    have hzr : (z : Rat) < 0 := by exact_mod_cast hneg
    have hl0 : l < 0 := by grind
    have hml : mag l = -l := by unfold mag; simp [hl0]
    have h5 : ((-z - 1 : Int) : Rat) = -(z : Rat) - 1 := by push_cast; rfl
    grind
  · have h2 : x.floor % z < z := Int.emod_lt_of_pos _ hpos
    have h3 : x.floor % z ≤ z - 1 := by omega
    have h4 : ((x.floor % z : Int) : Rat) ≤ ((z - 1 : Int) : Rat) := Rat.intCast_le_intCast.mpr h3
    have hzr : (0 : Rat) < (z : Rat) := by exact_mod_cast hpos
    have hh0 : 0 < h := by grind
    have hmh : mag h = h := by unfold mag; simp [show ¬ h < 0 by grind]
    have h5 : ((z - 1 : Int) : Rat) = (z : Rat) - 1 := by push_cast; rfl
    grind

theorem case_modOp (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {x k : Expr}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.modOp id line x k) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.modOp id line x k)) (evI A m (.modOp id line x k) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGk : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
    -- what the evaluation answers: the remainder by a whole number that is not 0
    have hval : ∀ v, eval C (n + 1) dep ρ (.modOp id line x k) = .ok v → ∃ xv kv p r,
        eval C n dep ρ x = .ok xv ∧ eval C n dep ρ k = .ok kv ∧ xv = .num p ∧ kv = .num r ∧ r ≠ 0 ∧
          r.den = 1 ∧ v = .num (modN p r.floor) := by
      intro v hv
      simp only [eval] at hv
      obtain ⟨xv, hxv, h3⟩ := bind_ok hv
      obtain ⟨kv, hkv, h4⟩ := bind_ok h3
      obtain ⟨p, hp, h5⟩ := bind_ok h4
      obtain ⟨r, hr, h6⟩ := bind_ok h5
      split at h6
      · exact absurd h6 failK_ne_ok
      · split at h6
        · exact absurd h6 failK_ne_ok
        · rename_i h0 hd
          cases h6
          exact ⟨xv, kv, p, r, hxv, hkv, numOf_ok hp, numOf_ok hr, h0, by simpa using hd, rfl⟩
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hn
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · obtain ⟨xv, _, _, _, hxv, _⟩ := hval v hv
        have := (h1 xv hxv).1; rw [hn] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨xv, hxv, _⟩
        · exact h2 k l hv hk
        · have := (h1 xv hxv).1; rw [hn] at this; exact absurd this Mem.not_never
    · rename_i hne
      -- what the analysis keeps grows from the divisor's in every branch
      have wk : (ev (ev A m x o).ctx m k (ev A m x o).out).out.widened = false := by
        split at hw
        · simp only [AR.of] at hw; exact hw
        · split at hw
          · simp only [AR.of] at hw; exact hw
          · split at hw <;> simp only [AR.of] at hw <;> exact grows_not_widened (grows_place (Grows.refl _)) hw
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (gEv (Grows.refl _)) wk)
      have HK : ∀ xv, eval C n dep ρ x = .ok xv →
          Concl C ρ (eval C n dep ρ k) (ev (ev A m x o).ctx m k (ev A m x o).out) :=
        fun xv hx => IH.expr m _ ρ k _ dep hGk (h1 xv hx).2.1 wk
      have hgx : Grows (ev A m x o).out (ev (ev A m x o).ctx m k (ev A m x o).out).out := gEv (Grows.refl _)
      -- the failures: the two sides', or a divisor that is 0
      have herr : ∀ k' l, eval C (n + 1) dep ρ (.modOp id line x k) = .error (.failed k' l) → k'.ofEffect = true →
          Wit C (ev (ev A m x o).ctx m k (ev A m x o).out).out ∨
            ∃ xv, eval C n dep ρ x = .ok xv ∧ eval C n dep ρ k = .ok (.num 0) := by
        intro k' l hv hk
        simp only [eval] at hv
        rcases bind_err hv with hv | ⟨xv, hxv, h3⟩
        · exact Or.inl (Wit.grow (h2 k' l hv hk) hgx)
        · rcases bind_err h3 with h4 | ⟨kv, hkv, h4⟩
          · exact Or.inl ((HK xv hxv).2 k' l h4 hk)
          · rcases bind_err h4 with h5 | ⟨p, _, h5⟩
            · rw [type_noEffect (numOf_err h5)] at hk; cases hk
            · rcases bind_err h5 with h6 | ⟨r, hr, h6⟩
              · rw [type_noEffect (numOf_err h6)] at hk; cases hk
              · split at h6
                · rename_i h0
                  subst h0
                  exact Or.inr ⟨xv, hxv, by rw [hkv, numOf_ok hr]⟩
                · split at h6
                  · rw [fail_type h6] at hk; cases hk
                  · cases h6
      split at hw
      · rename_i hn
        simp only [AR.of] at hw ⊢
        refine ⟨fun v hv => ?_, fun k' l hv hk => ?_⟩
        · obtain ⟨xv, kv, _, _, hxv, hkv, _⟩ := hval v hv
          have := ((HK xv hxv).1 kv hkv).1; rw [hn] at this; exact absurd this Mem.not_never
        · rcases herr k' l hv hk with hw' | ⟨xv, hxv, hkv⟩
          · exact hw'
          · have := ((HK xv hxv).1 _ hkv).1; rw [hn] at this; exact absurd this Mem.not_never
      · rename_i hnk
        split at hw
        · -- the divisor known to be one positive whole number
          rename_i M hM
          simp only [AR.of] at hw ⊢
          -- what the analysis knows of the divisor
          have hq : ∃ q, ((ev (ev A m x o).ctx m k (ev A m x o).out).abs.iv.getD Iv.top).isExact = some q ∧
              q.den = 1 ∧ 0 < q.num ∧ M = q.num := by
            split at hM
            · rename_i q hq
              split at hM
              · rename_i hqc
                cases hM
                simp only [Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at hqc
                exact ⟨q, hq, hqc.1, hqc.2, rfl⟩
              · cases hM
            · cases hM
          obtain ⟨q, hqe, hqd, hqp, rfl⟩ := hq
          refine ⟨fun v hv => ?_, fun k' l hv hk => ?_⟩
          · obtain ⟨xv, kv, p, r, hxv, hkv, rfl, rfl, hr0, hrd, rfl⟩ := hval v hv
            obtain ⟨hmx, _, _⟩ := h1 _ hxv
            obtain ⟨hmk, hsk, _⟩ := (HK _ hxv).1 _ hkv
            refine ⟨?_, hsk, fun _ h => by cases h⟩
            have hpx := In_ivD hmx
            have hrq := isExact_sound (In_ivD hmk) hqe
            subst hrq
            have hfl : r.floor = r.num := by rw [Rat.floor_def, hqd]; simp
            rw [hfl]
            apply Mem.num
            split
            · rename_i l h hl hh
              split
              · rename_i hcond
                simp only [Bool.and_eq_true, decide_eq_true_eq] at hcond
                have hl' := hpx.lo; have hh' := hpx.hi
                rw [hl] at hl'; rw [hh] at hh'
                exact modN_small hl' hh' ((Rat.num_nonneg).mp hcond.1) hcond.2
              · exact modN_ints hqp
            · exact modN_ints hqp
          · rcases herr k' l hv hk with hw' | ⟨xv, hxv, hkv⟩
            · exact hw'
            · obtain ⟨hmk, _, _⟩ := (HK _ hxv).1 _ hkv
              have h0 := isExact_sound (In_ivD hmk) hqe
              rw [← h0] at hqp; exact absurd hqp (by decide)
        · -- a divisor that is a value: the place `zero`, and the remainder from 0 to one less than
          -- its greatest size
          split
          · rename_i hz0
            rw [ite_eq_left hz0] at hw
            simp only [AR.of] at hw ⊢
            refine ⟨fun v hv => ?_, fun k' l hv hk => ?_⟩
            · obtain ⟨xv, kv, p, r, hxv, hkv, rfl, rfl, hr0, hrd, rfl⟩ := hval v hv
              obtain ⟨hmk, _, _⟩ := (HK _ hxv).1 _ hkv
              have := isExact_sound (In_ivD hmk) (by simpa using hz0)
              exact absurd this hr0
            · rcases herr k' l hv hk with hw' | ⟨xv, hxv, hkv⟩
              · exact Wit.grow hw' (grows_place (Grows.refl _))
              · obtain ⟨hmk, _, _⟩ := (HK _ hxv).1 _ hkv
                exact wit_place hmk rfl
          · rename_i hz0
            rw [ite_eq_right hz0] at hw
            simp only [AR.of] at hw ⊢
            refine ⟨fun v hv => ?_, fun k' l hv hk => ?_⟩
            · obtain ⟨xv, kv, p, r, hxv, hkv, rfl, rfl, hr0, hrd, rfl⟩ := hval v hv
              obtain ⟨hmk, hsk, _⟩ := (HK _ hxv).1 _ hkv
              refine ⟨?_, hsk, fun _ h => by cases h⟩
              have hrk := In_ivD hmk
              have hrn := rat_of_den_one hrd
              have hfl : r.floor = r.num := by rw [Rat.floor_def, hrd]; simp
              have hz : r.num ≠ 0 := fun h => hr0 (by rw [hrn, h]; rfl)
              apply Mem.num
              refine In_mk' ?_ ?_ ?_
              · show (0 : Rat) ≤ modN p r.floor
                unfold modN; rw [hfl]; exact_mod_cast Int.emod_nonneg _ hz
              · split
                · rename_i l h hl hh
                  show modN p r.floor ≤ (if (if h.num < 0 then -h else h) < (if l.num < 0 then -l else l) then
                    (if l.num < 0 then -l else l) else (if h.num < 0 then -h else h)) - 1
                  rw [magN_eq, magN_eq]
                  have hl' := hrk.lo; have hh' := hrk.hi
                  rw [hl] at hl'; rw [hh] at hh'
                  rw [hrn] at hl' hh'
                  unfold modN; rw [hfl]
                  exact modN_le_size hz hl' hh'
                · trivial
              · show (modN p r.floor).den ≤ 1
                simp [modN]
            · rcases herr k' l hv hk with hw' | ⟨xv, hxv, hkv⟩
              · exact Wit.grow hw' (grows_place (Grows.refl _))
              · obtain ⟨hmk, _, _⟩ := (HK _ hxv).1 _ hkv
                exact wit_place hmk rfl

-- ── `isqrt` ──────────────────────────────────────────────────────────────────

theorem sqrt_mono {a b : Nat} (h : a ≤ b) : a.sqrt ≤ b.sqrt := by
  have h1 := Nat.sqrt_le a
  have h2 := Nat.lt_succ_sqrt b
  apply Nat.le_of_not_lt
  intro hlt
  have h3 : (b.sqrt + 1) * (b.sqrt + 1) ≤ a.sqrt * a.sqrt := Nat.mul_le_mul hlt hlt
  exact absurd (Nat.lt_of_lt_of_le h2 (Nat.le_trans h3 (Nat.le_trans h1 h))) (Nat.lt_irrefl _)

theorem isqrtN_mono {x y r s : Rat} (hxy : x ≤ y) (hx : isqrtN x = some r) (hy : isqrtN y = some s) : r ≤ s := by
  unfold isqrtN at hx hy
  split at hx <;> split at hy <;> simp at hx hy
  subst hx hy
  have : x.floor ≤ y.floor := Rat.le_floor_iff.mpr (Rat.le_trans (Rat.floor_le x) hxy)
  have : x.floor.toNat ≤ y.floor.toNat := Int.toNat_le_toNat this
  exact_mod_cast sqrt_mono this

theorem isqrtN_some {x : Rat} (h : 0 ≤ x) : ∃ r, isqrtN x = some r := by
  unfold isqrtN
  have : ¬ x.num < 0 := by have := Rat.num_nonneg.mpr h; omega
  simp [this]

theorem case_isqrt (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {x : Expr}
    (hG : Good C res A) (hS : Sat A ρ) (hw : (evI A m (.isqrt id line x) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.isqrt id line x)) (evI A m (.isqrt id line x) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hn
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨xv, hxv, _⟩ := bind_ok hv
        have := (h1 xv hxv).1; rw [hn] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨xv, hxv, _⟩
        · exact h2 k l hv hk
        · have := (h1 xv hxv).1; rw [hn] at this; exact absurd this Mem.not_never
    · rename_i hne
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (grows_place (Grows.refl _)) hw)
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨xv, hxv, h3⟩ := bind_ok hv
        obtain ⟨p, hp, h4⟩ := bind_ok h3
        obtain rfl := numOf_ok hp
        obtain ⟨hm, hs, _⟩ := h1 _ hxv
        split at h4
        · rename_i r hr
          cases h4
          have hp0 : 0 ≤ p := by
            unfold isqrtN at hr; split at hr
            · cases hr
            · rename_i h; exact Rat.num_nonneg.mp (by omega)
          have hpx := In_ivD hm
          refine ⟨Mem.num (In_mk' ?_ ?_ ?_), ?_, fun _ h => by cases h⟩
          · -- the lower end
            simp only [EndLe]
            cases hl : ((ev A m x o).abs.iv.getD Iv.top).lo with
            | none => simp; unfold isqrtN at hr; split at hr <;> simp at hr; subst hr; exact_mod_cast Nat.zero_le _
            | some l =>
              simp only [Option.map_some, Option.bind_some]
              have hl0 : 0 ≤ (if l.num < 0 then (0 : Rat) else l) := by
                split
                · exact Rat.le_refl
                · rename_i h; exact Rat.num_nonneg.mp (by omega)
              have hlp : (if l.num < 0 then (0 : Rat) else l) ≤ p := by
                have := hpx.lo; rw [hl] at this
                split
                · exact hp0
                · exact this
              obtain ⟨s, hs⟩ := isqrtN_some hl0
              rw [hs]
              exact isqrtN_mono hlp hs hr
          · -- the upper end
            cases hh : ((ev A m x o).abs.iv.getD Iv.top).hi with
            | none => trivial
            | some h =>
              simp only [Option.bind_some]
              cases hs : isqrtN h with
              | none => trivial
              | some s =>
                have := hpx.hi; rw [hh] at this
                exact isqrtN_mono this hr hs
          · unfold isqrtN at hr; split at hr <;> simp at hr; subst hr; simp [DenLe]
          · -- the name is at least 0 from here on
            refine Sat.assume (x := x) (r := ⟨some 0, none⟩) (c := none) hs ?_
            intro _ s hsx w hw'
            have := sym_eval hsx hxv
            rw [this] at hw'; cases hw'
            intro q hq; cases hq
            exact ⟨hp0, trivial⟩
        · exact absurd h4 failK_ne_ok
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨xv, hxv, h3⟩
        · exact Wit.grow (h2 k l hv hk) (grows_place (Grows.refl _))
        · rcases bind_err h3 with h4 | ⟨p, hp, h4⟩
          · rw [type_noEffect (numOf_err h4)] at hk; cases hk
          · obtain rfl := numOf_ok hp
            obtain ⟨hm, _, _⟩ := h1 _ hxv
            split at h4
            · cases h4
            · rename_i hr
              have hneg : p.num < 0 := by
                unfold isqrtN at hr; split at hr
                · assumption
                · cases hr
              exact wit_place hm ⟨p, rfl, hneg⟩


-- ── `allocate` ───────────────────────────────────────────────────────────────

theorem allocateN_ok {t c s v : Rat} (h : allocateN t c s = .ok v) :
    s ≠ 0 ∧ fits (t * c) = true ∧ fits (t * c / s) = true ∧ fits (roundInt .down (t * c / s) 1) = true ∧
      v = roundInt .down (t * c / s) 1 := by
  simp only [allocateN, bind, Except.bind] at h
  rcases h1 : mulN t c with e | p <;> rw [h1] at h
  · cases h
  · simp only at h
    obtain ⟨rfl, hp⟩ := held_ok (r := t * c) h1
    rcases h2 : divN (t * c) s with e | q <;> rw [h2] at h
    · cases h
    · simp only at h
      unfold divN at h2
      split at h2
      · cases h2
      · rename_i hs
        obtain ⟨rfl, hq⟩ := held_ok h2
        obtain ⟨rfl, hr⟩ := held_ok (r := roundInt .down (t * c / s) 1) h
        exact ⟨hs, hp, hq, hr, rfl⟩

theorem allocateN_err {t c s : Rat} {a : Arith} (h : allocateN t c s = .error a) :
    fits (t * c) = false ∨ (s = 0 ∧ a = .zero) ∨ (s ≠ 0 ∧ fits (t * c / s) = false) ∨
      (s ≠ 0 ∧ fits (t * c) = true ∧ fits (t * c / s) = true ∧ fits (roundInt .down (t * c / s) 1) = false) := by
  simp only [allocateN, bind, Except.bind] at h
  rcases h1 : mulN t c with e | p <;> rw [h1] at h
  · exact Or.inl (held_err (r := t * c) h1).2
  · simp only at h
    obtain ⟨rfl, hp⟩ := held_ok (r := t * c) h1
    rcases h2 : divN (t * c) s with e | q <;> rw [h2] at h
    · simp only at h
      cases h
      unfold divN at h2
      split at h2
      · rename_i hs; cases h2; exact Or.inr (Or.inl ⟨hs, rfl⟩)
      · rename_i hs; exact Or.inr (Or.inr (Or.inl ⟨hs, (held_err h2).2⟩))
    · simp only at h
      unfold divN at h2
      split at h2
      · cases h2
      · rename_i hs
        obtain ⟨rfl, hq⟩ := held_ok h2
        exact Or.inr (Or.inr (Or.inr ⟨hs, hp, hq, (held_err (r := roundInt .down (t * c / s) 1) h).2⟩))

theorem alloc_ok {dep id line : Nat} {ρ : Env} {t c s : Expr} {cc cs out : Option Conv} {v : Val}
    (h : eval C (n + 1) dep ρ (.allocate id line t c s cc cs out) = .ok v) :
    ∃ tv cv sv c' s' r, eval C n dep ρ t = .ok (.num tv) ∧ eval C n dep ρ c = .ok (.num cv) ∧
      eval C n dep ρ s = .ok (.num sv) ∧ convN cc cv = .ok c' ∧ convN cs sv = .ok s' ∧
      allocateN tv c' s' = .ok r ∧ (∃ r', convN out r = .ok r' ∧ v = .num r') := by
  simp only [eval] at h
  obtain ⟨tv, ht, h1⟩ := bind_ok h
  obtain ⟨cv, hc, h2⟩ := bind_ok h1
  obtain ⟨sv, hs, h3⟩ := bind_ok h2
  obtain ⟨tv', ht', h4⟩ := bind_ok h3
  obtain ⟨cv', hc', h5⟩ := bind_ok h4
  obtain ⟨sv', hs', h6⟩ := bind_ok h5
  obtain ⟨c', hcc, h7⟩ := bind_ok h6
  obtain ⟨s', hcs, h8⟩ := bind_ok h7
  obtain ⟨r, hr, h9⟩ := bind_ok h8
  obtain ⟨r', hr', h10⟩ := bind_ok h9
  cases h10
  have e1 := numOf_ok ht'
  have e2 := numOf_ok hc'
  have e3 := numOf_ok hs'
  subst e1 e2 e3
  exact ⟨_, _, _, _, _, _, ht, hc, hs, hcc, hcs, liftA_ok hr, r', hr', rfl⟩

theorem alloc_err {dep id line : Nat} {ρ : Env} {t c s : Expr} {cc cs out : Option Conv} {k : Kind} {l : Nat}
    (h : eval C (n + 1) dep ρ (.allocate id line t c s cc cs out) = .error (.failed k l)) (hk : k.ofEffect = true) :
    eval C n dep ρ t = .error (.failed k l) ∨
    (∃ tv, eval C n dep ρ t = .ok tv ∧ eval C n dep ρ c = .error (.failed k l)) ∨
    (∃ tv cv, eval C n dep ρ t = .ok tv ∧ eval C n dep ρ c = .ok cv ∧ eval C n dep ρ s = .error (.failed k l)) ∨
    ∃ tv cv sv, eval C n dep ρ t = .ok (.num tv) ∧ eval C n dep ρ c = .ok (.num cv) ∧
      eval C n dep ρ s = .ok (.num sv) ∧
      (convN cc cv = .error (.failed k l) ∨
       (∃ c', convN cc cv = .ok c' ∧ convN cs sv = .error (.failed k l)) ∨
       (∃ c' s', convN cc cv = .ok c' ∧ convN cs sv = .ok s' ∧
         ((∃ a, allocateN tv c' s' = .error a) ∨
          (∃ r, allocateN tv c' s' = .ok r ∧ convN out r = .error (.failed k l))))) := by
  simp only [eval] at h
  rcases bind_err h with e1 | ⟨tv, ht, h1⟩
  · exact Or.inl e1
  rcases bind_err h1 with e2 | ⟨cv, hc, h2⟩
  · exact Or.inr (Or.inl ⟨tv, ht, e2⟩)
  rcases bind_err h2 with e3 | ⟨sv, hs, h3⟩
  · exact Or.inr (Or.inr (Or.inl ⟨tv, cv, ht, hc, e3⟩))
  rcases bind_err h3 with e4 | ⟨tv', ht', h4⟩
  · rw [type_noEffect (numOf_err e4)] at hk; cases hk
  rcases bind_err h4 with e5 | ⟨cv', hc', h5⟩
  · rw [type_noEffect (numOf_err e5)] at hk; cases hk
  rcases bind_err h5 with e6 | ⟨sv', hs', h6⟩
  · rw [type_noEffect (numOf_err e6)] at hk; cases hk
  have q1 := numOf_ok ht'
  have q2 := numOf_ok hc'
  have q3 := numOf_ok hs'
  subst q1 q2 q3
  refine Or.inr (Or.inr (Or.inr ⟨_, _, _, ht, hc, hs, ?_⟩))
  rcases bind_err h6 with e7 | ⟨c', hcc, h7⟩
  · exact Or.inl e7
  rcases bind_err h7 with e8 | ⟨s', hcs, h8⟩
  · exact Or.inr (Or.inl ⟨c', hcc, e8⟩)
  refine Or.inr (Or.inr ⟨c', s', hcc, hcs, ?_⟩)
  rcases bind_err h8 with e9 | ⟨r, hr, h9⟩
  · cases hr' : allocateN tv' c' s' with
    | error a => exact Or.inl ⟨a, rfl⟩
    | ok r => rw [hr'] at e9; simp [liftA] at e9
  rcases bind_err h9 with e10 | ⟨r', _, h10⟩
  · exact Or.inr ⟨r, liftA_ok hr, e10⟩
  · cases h10

theorem okNum_num {i : Iv} : okNum (.num i) = i.fits := rfl

theorem case_allocate (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {t c s : Expr} {cc cs out : Option Conv} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.allocate id line t c s cc cs out) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.allocate id line t c s cc cs out))
      (evI A m (.allocate id line t c s cc cs out) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGc : Good C res (ev A m t o).ctx := hG.same ((sat m).ev A t o).2
    have hGs : Good C res (ev (ev A m t o).ctx m c (ev A m t o).out).ctx := hGc.same ((sat m).ev _ c _).2
    simp only [evI] at hw ⊢
    split at hw
    · -- `t` never reached
      rename_i hn
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ t o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · obtain ⟨tv, _, _, _, _, _, ht, _⟩ := alloc_ok hv
        have := (h1 _ ht).1; rw [hn] at this; exact absurd this Mem.not_never
      · rcases alloc_err hv hk with h | ⟨tv, ht, _⟩ | ⟨tv, _, ht, _⟩ | ⟨tv, _, _, ht, _⟩
        · exact h2 k l h hk
        all_goals (have := (h1 _ ht).1; rw [hn] at this; exact absurd this Mem.not_never)
    · rename_i hn1
      have wc : (ev (ev A m t o).ctx m c (ev A m t o).out).out.widened = false := by
        split at hw
        · exact hw
        · split at hw
          · exact grows_not_widened (gEv (Grows.refl _)) hw
          · split at hw <;> simp only [AR.of] at hw
            · exact grows_not_widened (grows_place (grows_place (grows_convA (grows_convA (gEv (Grows.refl _)))))) hw
            · exact grows_not_widened (grows_convA (grows_heldA (grows_place (grows_place
                (grows_convA (grows_convA (gEv (Grows.refl _)))))))) hw
      obtain ⟨h1, h2⟩ := IH.expr m A ρ t o dep hG hS (grows_not_widened (gEv (Grows.refl _)) wc)
      have HC : ∀ x, eval C n dep ρ t = .ok x →
          Concl C ρ (eval C n dep ρ c) (ev (ev A m t o).ctx m c (ev A m t o).out) :=
        fun x hx => IH.expr m _ ρ c _ dep hGc (h1 x hx).2.1 wc
      split at hw
      · -- `c` never reached
        rename_i hn
        simp only [AR.of] at hw ⊢
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · obtain ⟨tv, cv, _, _, _, _, ht, hc, _⟩ := alloc_ok hv
          have := ((HC _ ht).1 _ hc).1; rw [hn] at this; exact absurd this Mem.not_never
        · rcases alloc_err hv hk with h | ⟨tv, ht, h⟩ | ⟨tv, cv, ht, hc, _⟩ | ⟨tv, cv, _, ht, hc, _⟩
          · exact Wit.grow (h2 k l h hk) (gEv (Grows.refl _))
          · exact (HC _ ht).2 k l h hk
          all_goals (have := ((HC _ ht).1 _ hc).1; rw [hn] at this; exact absurd this Mem.not_never)
      · rename_i hn2
        have ws : (ev (ev (ev A m t o).ctx m c (ev A m t o).out).ctx m s
            (ev (ev A m t o).ctx m c (ev A m t o).out).out).out.widened = false := by
          split at hw
          · exact hw
          · split at hw <;> simp only [AR.of] at hw
            · exact grows_not_widened (grows_place (grows_place (grows_convA (grows_convA (Grows.refl _))))) hw
            · exact grows_not_widened (grows_convA (grows_heldA (grows_place (grows_place
                (grows_convA (grows_convA (Grows.refl _))))))) hw
        have HS : ∀ x y, eval C n dep ρ t = .ok x → eval C n dep ρ c = .ok y →
            Concl C ρ (eval C n dep ρ s) (ev (ev (ev A m t o).ctx m c (ev A m t o).out).ctx m s
              (ev (ev A m t o).ctx m c (ev A m t o).out).out) :=
          fun x y hx hy => IH.expr m _ ρ s _ dep hGs ((HC x hx).1 y hy).2.1 ws
        have hgt : Grows (ev A m t o).out (ev (ev A m t o).ctx m c (ev A m t o).out).out := gEv (Grows.refl _)
        have hgc : Grows (ev (ev A m t o).ctx m c (ev A m t o).out).out
            (ev (ev (ev A m t o).ctx m c (ev A m t o).out).ctx m s (ev (ev A m t o).ctx m c (ev A m t o).out).out).out :=
          gEv (Grows.refl _)
        split at hw
        · -- `s` never reached
          rename_i hn
          simp only [AR.of] at hw ⊢
          refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
          · obtain ⟨tv, cv, sv, _, _, _, ht, hc, hs, _⟩ := alloc_ok hv
            have := ((HS _ _ ht hc).1 _ hs).1; rw [hn] at this; exact absurd this Mem.not_never
          · rcases alloc_err hv hk with h | ⟨tv, ht, h⟩ | ⟨tv, cv, ht, hc, h⟩ | ⟨tv, cv, sv, ht, hc, hs, _⟩
            · exact Wit.grow (h2 k l h hk) (hgt.trans hgc)
            · exact Wit.grow ((HC _ ht).2 k l h hk) hgc
            · exact (HS _ _ ht hc).2 k l h hk
            · have := ((HS _ _ ht hc).1 _ hs).1; rw [hn] at this; exact absurd this Mem.not_never
        · -- the three reached
          generalize ev (ev (ev A m t o).ctx m c (ev A m t o).out).ctx m s
            (ev (ev A m t o).ctx m c (ev A m t o).out).out = rs at HS hgc ⊢
          generalize ev (ev A m t o).ctx m c (ev A m t o).out = rc at HC HS hgt hgc ⊢
          generalize ev A m t o = rt at h1 h2 HC HS hgt ⊢
          have GPC : Grows rs.out (convA rs.out cc rc.abs).2 := grows_convA (Grows.refl _)
          have GPS : Grows (convA rs.out cc rc.abs).2 (convA (convA rs.out cc rc.abs).2 cs rs.abs).2 :=
            grows_convA (Grows.refl _)
          have MPC : ∀ cv c', Mem rc.abs (.num cv) → convN cc cv = .ok c' → Mem (convA rs.out cc rc.abs).1 (.num c') :=
            fun _ _ h hc => convN_mem h hc
          have WPC : ∀ cv e, Mem rc.abs (.num cv) → convN cc cv = .error e → Wit C (convA rs.out cc rc.abs).2 :=
            fun _ _ h hc => (convN_wit h hc).2
          generalize convA rs.out cc rc.abs = PC at GPC GPS MPC WPC ⊢
          have MPS : ∀ sv s', Mem rs.abs (.num sv) → convN cs sv = .ok s' → Mem (convA PC.2 cs rs.abs).1 (.num s') :=
            fun _ _ h hc => convN_mem h hc
          have WPS : ∀ sv e, Mem rs.abs (.num sv) → convN cs sv = .error e → Wit C (convA PC.2 cs rs.abs).2 :=
            fun _ _ h hc => (convN_wit h hc).2
          generalize convA PC.2 cs rs.abs = PS at GPS MPS WPS ⊢
          generalize hP : (rt.abs.iv.getD Iv.top).times (PC.1.iv.getD Iv.top) = P
          generalize hQ : P.over (PS.1.iv.getD Iv.top) = Q
          generalize hR : Q.round .down 1 = Rd
          -- what the evaluation's values are in
          have hin : ∀ tv cv sv c' s', eval C n dep ρ t = .ok (.num tv) → eval C n dep ρ c = .ok (.num cv) →
              eval C n dep ρ s = .ok (.num sv) → convN cc cv = .ok c' → convN cs sv = .ok s' →
              P.In (tv * c') ∧ (PS.1.iv.getD Iv.top).In s' ∧ Mem PS.1 (.num s') ∧ Sat rs.ctx ρ ∧
              (s' ≠ 0 → Q.In (tv * c' / s') ∧ Rd.In (roundInt .down (tv * c' / s') 1)) := by
            intro tv cv sv c' s' ht hc hs hcc hcs
            obtain ⟨hmt, _, _⟩ := h1 _ ht
            obtain ⟨hmc, _, _⟩ := (HC _ ht).1 _ hc
            obtain ⟨hms, hss, _⟩ := (HS _ _ ht hc).1 _ hs
            have hpc := MPC _ _ hmc hcc
            have hps := MPS _ _ hms hcs
            have hp : P.In (tv * c') := hP ▸ In_times (In_ivD hmt) (In_ivD hpc)
            refine ⟨hp, In_ivD hps, hps, hss, fun hs0 => ?_⟩
            have hq : Q.In (tv * c' / s') := hQ ▸ In_over hp (In_ivD hps) hs0
            exact ⟨hq, hR ▸ In_round .down hq (by decide)⟩
          split
          · -- the divisor is 0: never past it
            rename_i h0
            simp only [beq_iff_eq] at h0
            refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
            · obtain ⟨tv, cv, sv, c', s', r, ht, hc, hs, hcc, hcs, hr, _⟩ := alloc_ok hv
              obtain ⟨_, hsi, _, _, _⟩ := hin _ _ _ _ _ ht hc hs hcc hcs
              exact absurd (isExact_sound hsi h0) (allocateN_ok hr).1
            · simp only [AR.of]
              rcases alloc_err hv hk with h | ⟨tv, ht, h⟩ | ⟨tv, cv, ht, hc, h⟩ | ⟨tv, cv, sv, ht, hc, hs, h⟩
              · exact Wit.grow (h2 k l h hk) (hgt.trans (hgc.trans (GPC.trans (GPS.trans (grows_place (grows_place
                  (Grows.refl _)))))))
              · exact Wit.grow ((HC _ ht).2 k l h hk) (hgc.trans (GPC.trans (GPS.trans (grows_place (grows_place
                  (Grows.refl _))))))
              · exact Wit.grow ((HS _ _ ht hc).2 k l h hk) (GPC.trans (GPS.trans (grows_place (grows_place
                  (Grows.refl _)))))
              · obtain ⟨hmt, _, _⟩ := h1 _ ht
                obtain ⟨hmc, _, _⟩ := (HC _ ht).1 _ hc
                obtain ⟨hms, _, _⟩ := (HS _ _ ht hc).1 _ hs
                rcases h with h | ⟨c', hcc, h⟩ | ⟨c', s', hcc, hcs, h⟩
                · exact Wit.grow (WPC _ _ hmc h) (GPS.trans (grows_place (grows_place (Grows.refl _))))
                · exact Wit.grow (WPS _ _ hms h) (grows_place (grows_place (Grows.refl _)))
                · obtain ⟨_, hsi, hps, _, _⟩ := hin _ _ _ _ _ ht hc hs hcc hcs
                  have hs0 := isExact_sound hsi h0
                  subst hs0
                  -- a division by 0: the place for 0 takes it
                  exact Wit.grow (wit_place (what := .zero) hps rfl) (grows_place (Grows.refl _))
          · rename_i h0
            simp only [AR.of] at hw ⊢
            refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
            · obtain ⟨tv, cv, sv, c', s', r, ht, hc, hs, hcc, hcs, hr, r', hr', rfl⟩ := alloc_ok hv
              obtain ⟨hs0, _, _, hfr, rfl⟩ := allocateN_ok hr
              obtain ⟨_, _, _, hss, hqr⟩ := hin _ _ _ _ _ ht hc hs hcc hcs
              exact ⟨convN_mem (heldA_mem (Mem.num (hqr hs0).2) hfr) hr', hss, fun _ h => by cases h⟩
            · rcases alloc_err hv hk with h | ⟨tv, ht, h⟩ | ⟨tv, cv, ht, hc, h⟩ | ⟨tv, cv, sv, ht, hc, hs, h⟩
              · exact Wit.grow (h2 k l h hk) (hgt.trans (hgc.trans (GPC.trans (GPS.trans (grows_convA (grows_heldA
                  (grows_place (grows_place (Grows.refl _)))))))))
              · exact Wit.grow ((HC _ ht).2 k l h hk) (hgc.trans (GPC.trans (GPS.trans (grows_convA (grows_heldA
                  (grows_place (grows_place (Grows.refl _))))))))
              · exact Wit.grow ((HS _ _ ht hc).2 k l h hk) (GPC.trans (GPS.trans (grows_convA (grows_heldA
                  (grows_place (grows_place (Grows.refl _)))))))
              · obtain ⟨hmt, _, _⟩ := h1 _ ht
                obtain ⟨hmc, _, _⟩ := (HC _ ht).1 _ hc
                obtain ⟨hms, _, _⟩ := (HS _ _ ht hc).1 _ hs
                rcases h with h | ⟨c', hcc, h⟩ | ⟨c', s', hcc, hcs, ⟨a, ha⟩ | ⟨r, hr, h⟩⟩
                · exact Wit.grow (WPC _ _ hmc h) (GPS.trans (grows_convA (grows_heldA (grows_place (grows_place
                    (Grows.refl _))))))
                · exact Wit.grow (WPS _ _ hms h) (grows_convA (grows_heldA (grows_place (grows_place (Grows.refl _)))))
                · obtain ⟨hp, _, hps, _, hqr⟩ := hin _ _ _ _ _ ht hc hs hcc hcs
                  -- where the allocation fails: 0, or a value past int64
                  rcases allocateN_err ha with hf | ⟨hs0, _⟩ | ⟨hs0, hf⟩ | ⟨hs0, hfp, hfq, hf⟩
                  · -- `t × c`
                    have hPf : P.fits = false := by
                      cases hpf : P.fits
                      · rfl
                      · rw [fits_sound hp hpf] at hf; cases hf
                    exact Wit.grow (wit_overflow (by simp only [hPf, okNum_num, Bool.not_false, ite_true]))
                      (grows_convA (grows_heldA (Grows.refl _)))
                  · subst hs0
                    exact Wit.grow (wit_place (what := .zero) hps rfl)
                      (grows_convA (grows_heldA (grows_place (Grows.refl _))))
                  · -- the quotient
                    have hQf : Q.fits = false := by
                      cases hqf : Q.fits
                      · rfl
                      · rw [fits_sound (hqr hs0).1 hqf] at hf; cases hf
                    refine Wit.grow (wit_overflow ?_) (grows_convA (grows_heldA (Grows.refl _)))
                    cases hpf : P.fits <;> simp [okNum_num, hpf, hQf]
                  · -- the rounding
                    refine Wit.grow ?_ (grows_convA (grows_heldA (Grows.refl _)))
                    cases hpf : P.fits
                    · exact wit_overflow (by simp [okNum_num, hpf])
                    · cases hqf : Q.fits
                      · exact wit_overflow (by simp [okNum_num, hqf])
                      · simp only [Bool.not_true, Bool.false_eq_true, ite_false]
                        exact wit_place (Mem.num (hqr hs0).2) ⟨_, rfl, hf⟩
                · -- the answer read in the type of `allocate`
                  obtain ⟨_, _, _, _, hqr⟩ := hin _ _ _ _ _ ht hc hs hcc hcs
                  obtain ⟨hs0, _, _, hfr, rfl⟩ := allocateN_ok hr
                  exact (convN_wit (heldA_mem (Mem.num (hqr hs0).2) hfr) h).2


-- ── Roundings ────────────────────────────────────────────────────────────────

theorem round_ok {dep id line : Nat} {ρ : Env} {way : Way} {x g : Expr} {cg out : Option Conv} {top : Abs} {v : Val}
    (h : eval C (n + 1) dep ρ (.round id line way x g cg out top) = .ok v) :
    ∃ xv gv g' r', eval C n dep ρ x = .ok (.num xv) ∧ eval C n dep ρ g = .ok (.num gv) ∧ convN cg gv = .ok g' ∧
      0 < g' ∧ fits (roundInt way xv g') = true ∧ convN out (roundInt way xv g') = .ok r' ∧
      top.has (.num r') = true ∧ v = .num r' := by
  simp only [eval] at h
  obtain ⟨xv, hx, h1⟩ := bind_ok h
  obtain ⟨gv, hg, h2⟩ := bind_ok h1
  obtain ⟨xv', hx', h3⟩ := bind_ok h2
  obtain ⟨gv', hg', h4⟩ := bind_ok h3
  obtain ⟨g', hcg, h5⟩ := bind_ok h4
  have q1 := numOf_ok hx'
  have q2 := numOf_ok hg'
  subst q1 q2
  split at h5
  · exact absurd h5 failK_ne_ok
  · rename_i hpos
    obtain ⟨r, hr, h6⟩ := bind_ok h5
    obtain ⟨r', hr', h7⟩ := bind_ok h6
    obtain ⟨rfl, hf⟩ := held_ok (r := roundInt way xv' g') (liftA_ok hr)
    split at h7
    · rename_i htop
      cases h7
      exact ⟨_, _, _, _, hx, hg, hcg, Rat.not_le.mp hpos, hf, hr', htop, rfl⟩
    · exact absurd h7 failK_ne_ok

theorem round_err {dep id line : Nat} {ρ : Env} {way : Way} {x g : Expr} {cg out : Option Conv} {top : Abs}
    {k : Kind} {l : Nat} (h : eval C (n + 1) dep ρ (.round id line way x g cg out top) = .error (.failed k l))
    (hk : k.ofEffect = true) :
    eval C n dep ρ x = .error (.failed k l) ∨
    (∃ xv, eval C n dep ρ x = .ok xv ∧ eval C n dep ρ g = .error (.failed k l)) ∨
    ∃ xv gv, eval C n dep ρ x = .ok (.num xv) ∧ eval C n dep ρ g = .ok (.num gv) ∧
      (convN cg gv = .error (.failed k l) ∨
       ∃ g', convN cg gv = .ok g' ∧ 0 < g' ∧
         (fits (roundInt way xv g') = false ∨
          (fits (roundInt way xv g') = true ∧ convN out (roundInt way xv g') = .error (.failed k l)))) := by
  simp only [eval] at h
  rcases bind_err h with e1 | ⟨xv, hx, h1⟩
  · exact Or.inl e1
  rcases bind_err h1 with e2 | ⟨gv, hg, h2⟩
  · exact Or.inr (Or.inl ⟨xv, hx, e2⟩)
  rcases bind_err h2 with e3 | ⟨xv', hx', h3⟩
  · rw [type_noEffect (numOf_err e3)] at hk; cases hk
  rcases bind_err h3 with e4 | ⟨gv', hg', h4⟩
  · rw [type_noEffect (numOf_err e4)] at hk; cases hk
  have q1 := numOf_ok hx'
  have q2 := numOf_ok hg'
  subst q1 q2
  refine Or.inr (Or.inr ⟨_, _, hx, hg, ?_⟩)
  rcases bind_err h4 with e5 | ⟨g', hcg, h5⟩
  · exact Or.inl e5
  refine Or.inr ⟨g', hcg, ?_⟩
  split at h5
  · rw [fail_type h5] at hk; cases hk
  · rename_i hpos
    refine ⟨Rat.not_le.mp hpos, ?_⟩
    rcases bind_err h5 with e6 | ⟨r, hr, h6⟩
    · rcases liftA_err e6 with ⟨e7, _⟩ | ⟨e7, _⟩
      · exact Or.inl (held_err (r := roundInt way xv' g') e7).2
      · simp [roundN, held] at e7; split at e7 <;> simp at e7
    · obtain ⟨rfl, hf⟩ := held_ok (r := roundInt way xv' g') (liftA_ok hr)
      rcases bind_err h6 with e8 | ⟨r', _, h7⟩
      · exact Or.inr ⟨hf, e8⟩
      · split at h7
        · cases h7
        · rw [fail_type h7] at hk; cases hk

theorem case_round (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {way : Way}
    {x g : Expr} {cg out : Option Conv} {top : Abs} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.round id line way x g cg out top) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.round id line way x g cg out top)) (evI A m (.round id line way x g cg out top) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGx : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hn
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · obtain ⟨xv, _, _, _, hx, _⟩ := round_ok hv
        have := (h1 _ hx).1; rw [hn] at this; exact absurd this Mem.not_never
      · rcases round_err hv hk with h | ⟨xv, hx, _⟩ | ⟨xv, _, hx, _⟩
        · exact h2 k l h hk
        all_goals (have := (h1 _ hx).1; rw [hn] at this; exact absurd this Mem.not_never)
    · rename_i hn1
      have wg : (ev (ev A m x o).ctx m g (ev A m x o).out).out.widened = false := by
        split at hw
        · exact hw
        · split at hw
          · split at hw <;> simp only [AR.of] at hw
            · exact grows_not_widened (grows_convA (grows_heldA (grows_convA (Grows.refl _)))) hw
            · exact grows_not_widened (grows_convA (Grows.refl _)) hw
          · simp only [AR.of] at hw; exact grows_not_widened (grows_place (grows_convA (Grows.refl _))) hw
      obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (gEv (Grows.refl _)) wg)
      have HG : ∀ xv, eval C n dep ρ x = .ok xv →
          Concl C ρ (eval C n dep ρ g) (ev (ev A m x o).ctx m g (ev A m x o).out) :=
        fun xv hx => IH.expr m _ ρ g _ dep hGx (h1 xv hx).2.1 wg
      have hgx : Grows (ev A m x o).out (ev (ev A m x o).ctx m g (ev A m x o).out).out := gEv (Grows.refl _)
      split at hw
      · rename_i hn
        simp only [AR.of] at hw ⊢
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · obtain ⟨xv, gv, _, _, hx, hg, _⟩ := round_ok hv
          have := ((HG _ hx).1 _ hg).1; rw [hn] at this; exact absurd this Mem.not_never
        · rcases round_err hv hk with h | ⟨xv, hx, h⟩ | ⟨xv, gv, hx, hg, _⟩
          · exact Wit.grow (h2 k l h hk) hgx
          · exact (HG _ hx).2 k l h hk
          · have := ((HG _ hx).1 _ hg).1; rw [hn] at this; exact absurd this Mem.not_never
      · generalize ev (ev A m x o).ctx m g (ev A m x o).out = rg at HG hgx ⊢
        generalize ev A m x o = rx at h1 h2 HG hgx ⊢
        have GPG : Grows rg.out (convA rg.out cg rg.abs).2 := grows_convA (Grows.refl _)
        have MPG : ∀ gv g', Mem rg.abs (.num gv) → convN cg gv = .ok g' → Mem (convA rg.out cg rg.abs).1 (.num g') :=
          fun _ _ h hc => convN_mem h hc
        have WPG : ∀ gv e, Mem rg.abs (.num gv) → convN cg gv = .error e → Wit C (convA rg.out cg rg.abs).2 :=
          fun _ _ h hc => (convN_wit h hc).2
        generalize convA rg.out cg rg.abs = PG at GPG MPG WPG ⊢
        -- the failures of the two sides and of the step
        have Eside : ∀ k l, k.ofEffect = true → ∀ (O : Out), Grows PG.2 O →
            (eval C n dep ρ x = .error (.failed k l) ∨
              (∃ xv, eval C n dep ρ x = .ok xv ∧ eval C n dep ρ g = .error (.failed k l)) ∨
              ∃ xv gv, eval C n dep ρ x = .ok (.num xv) ∧ eval C n dep ρ g = .ok (.num gv) ∧
                convN cg gv = .error (.failed k l)) → Wit C O := by
          intro k l hk O gO h
          rcases h with h | ⟨xv, hx, h⟩ | ⟨xv, gv, hx, hg, h⟩
          · exact Wit.grow (h2 k l h hk) (hgx.trans (GPG.trans gO))
          · exact Wit.grow ((HG _ hx).2 k l h hk) (GPG.trans gO)
          · exact Wit.grow (WPG _ _ ((HG _ hx).1 _ hg).1 h) gO
        split
        · rename_i grid hgrid
          split
          · -- a step above 0: the rounding of the values
            rename_i hpos
            simp only [AR.of]
            refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
            · obtain ⟨xv, gv, g', r', hx, hg, hcg, hg0, hf, hr', _, rfl⟩ := round_ok hv
              obtain ⟨hmx, _, _⟩ := h1 _ hx
              obtain ⟨hmg, hsg, _⟩ := (HG _ hx).1 _ hg
              have hgg := isExact_sound (In_ivD (MPG _ _ hmg hcg)) hgrid
              subst hgg
              exact ⟨convN_mem (heldA_mem (Mem.num (In_round way (In_ivD hmx) hg0)) hf) hr', hsg,
                fun _ h => by cases h⟩
            · rcases round_err hv hk with h | h | ⟨xv, gv, hx, hg, h | ⟨g', hcg, hg0, hf | ⟨hf, h⟩⟩⟩
              · exact Eside k l hk _ (grows_convA (grows_heldA (Grows.refl _))) (Or.inl h)
              · exact Eside k l hk _ (grows_convA (grows_heldA (Grows.refl _))) (Or.inr (Or.inl h))
              · exact Eside k l hk _ (grows_convA (grows_heldA (Grows.refl _))) (Or.inr (Or.inr ⟨xv, gv, hx, hg, h⟩))
              · obtain ⟨hmx, _, _⟩ := h1 _ hx
                obtain ⟨hmg, _, _⟩ := (HG _ hx).1 _ hg
                have hgg := isExact_sound (In_ivD (MPG _ _ hmg hcg)) hgrid
                subst hgg
                exact Wit.grow (heldA_wit (Mem.num (In_round way (In_ivD hmx) hg0)) hf) (grows_convA (Grows.refl _))
              · obtain ⟨hmx, _, _⟩ := h1 _ hx
                obtain ⟨hmg, _, _⟩ := (HG _ hx).1 _ hg
                have hgg := isExact_sound (In_ivD (MPG _ _ hmg hcg)) hgrid
                subst hgg
                exact (convN_wit (heldA_mem (Mem.num (In_round way (In_ivD hmx) hg0)) hf) h).2
          · -- a step at or below 0: the evaluation fails on its types
            rename_i hpos
            simp only [AR.of]
            refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
            · obtain ⟨xv, gv, g', r', hx, hg, hcg, hg0, _⟩ := round_ok hv
              obtain ⟨hmg, _, _⟩ := (HG _ hx).1 _ hg
              have hgg := isExact_sound (In_ivD (MPG _ _ hmg hcg)) hgrid
              subst hgg
              exact absurd hg0 hpos
            · rcases round_err hv hk with h | h | ⟨xv, gv, hx, hg, h | ⟨g', hcg, hg0, _⟩⟩
              · exact Eside k l hk _ (Grows.refl _) (Or.inl h)
              · exact Eside k l hk _ (Grows.refl _) (Or.inr (Or.inl h))
              · exact Eside k l hk _ (Grows.refl _) (Or.inr (Or.inr ⟨xv, gv, hx, hg, h⟩))
              · obtain ⟨hmg, _, _⟩ := (HG _ hx).1 _ hg
                have hgg := isExact_sound (In_ivD (MPG _ _ hmg hcg)) hgrid
                subst hgg
                exact absurd hg0 hpos
        · -- a step that is no one value: every value of the type, and the place for int64
          simp only [AR.of]
          refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
          · obtain ⟨xv, gv, g', r', hx, hg, _, _, _, _, htop, rfl⟩ := round_ok hv
            exact ⟨has_sound' htop, ((HG _ hx).1 _ hg).2.1, fun _ h => by cases h⟩
          · exact wit_overflow rfl

end

end KanameCore
