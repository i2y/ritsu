/-
  Function values (kaname's DESIGN 27.3, 27.4, 15.2.2): a lambda made a value, and a function value
  called.

  A lambda made a value is the lambda with the values of the names in scope (`Val.closure`); the
  analysis says the lambda with what the names in scope are (`Abs.fns`), and each name the closure
  caught has a value among what the analysis read of it (`Mem.clos`), as the scope it was made in
  holds the environment (`Sat`). A function value called is one of the lambdas it can be: its body,
  read with what it caught and its parameters given the values of the call, is the body the
  evaluation goes into, one more call deep, with the names it caught and the call's values; the
  induction on the steps gives its answer and its failures. A function value the analysis does not
  see (a parameter of a function type) is left out, as a running value let go (`widened`).
-/
import KanameCore.SoundDict

namespace KanameCore

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

-- ── The names a lambda caught ───────────────────────────────────────────────

/-- Two lists of values one by one among their abstract values, under the same names. -/
theorem paired_zip : ∀ (names : List String) (as : List Abs) (vs : List Val), AllMem as vs →
    Paired (names.zip as) (names.zip vs)
  | [], _, _, _ => by simp [Paired]
  | _ :: _, [], [], _ => by simp [Paired]
  | x :: names, a :: as, v :: vs, ⟨hm, hall⟩ => ⟨rfl, hm, paired_zip names as vs hall⟩
  | _ :: _, [], _ :: _, h => by simp [AllMem] at h
  | _ :: _, _ :: _, [], h => by simp [AllMem] at h

/-- The scope a lambda's body is read in holds the environment its body is evaluated in: its
    parameters given the values of the call, then the names it caught (`Mem.clos`). -/
theorem sat_lam {A : ACtx} {P : List (String × Abs)} {Q : Env} {ae : List (String × Abs)} {env : Env}
    (hp : Paired P Q) (h1 : ∀ x a, (ae.find? (·.1 == x)).map (·.2) = some a → (Env.get? env x).isSome = true)
    (h2 : ∀ x a v, (ae.find? (·.1 == x)).map (·.2) = some a → Env.get? env x = some v → Mem a v) :
    Sat { A with env := P ++ ae, facts := [] } (Q ++ env) where
  env := by
    induction P generalizing Q with
    | nil =>
      cases Q with
      | nil =>
        intro x a hx
        simp only [ACtx.lookup, List.nil_append] at hx
        obtain ⟨v, hv⟩ := Option.isSome_iff_exists.mp (h1 x a hx)
        exact ⟨v, by simpa using hv, h2 x a v hx hv⟩
      | cons _ _ => simp [Paired] at hp
    | cons p P ih =>
      cases Q with
      | nil => simp [Paired] at hp
      | cons q Q =>
        obtain ⟨y, b⟩ := p
        obtain ⟨z, w⟩ := q
        obtain ⟨hyz, hm, hp'⟩ := hp
        subst hyz
        intro x a hx
        have ih' := ih hp'
        have hl : ({ A with env := ((y, b) :: P) ++ ae, facts := [] } : ACtx).lookup x =
            if y = x then some b else ({ A with env := P ++ ae, facts := [] } : ACtx).lookup x := by
          unfold ACtx.lookup
          by_cases hyx : y = x
          · subst hyx; simp
          · simp [hyx]
        rw [hl] at hx
        rw [List.cons_append, get_cons]
        by_cases hyx : y = x
        · simp only [hyx, ite_true] at hx ⊢; cases hx; exact ⟨w, rfl, hm⟩
        · simp only [hyx, ite_false] at hx ⊢; exact ih' x a hx
  facts := by intro p r q h; simp at h

-- ── The lambdas of a function value, read ───────────────────────────────────

/-- What the bodies read before answer stays among what the reading answers. -/
theorem lams_keep {A : ACtx} : ∀ (k : Nat) (fs : List (Nat × List (String × Abs) × Bool)) (args : Option (List Abs))
    (acc : Abs) (o : Out) (v : Val), Mem acc v → Mem (lamsAbs A k fs args acc o).1 v
  | 0, _, _, _, _, v, _ => by simp only [lamsAbs]; exact Mem.any v
  | _ + 1, [], _, _, _, _, h => by simp only [lamsAbs]; exact h
  | k + 1, (i, ae, own) :: rest, args, acc, o, v, h => by
    simp only [lamsAbs]
    split
    · exact lams_keep k rest args _ _ v (mem_join_left h)
    · exact lams_keep k rest args _ _ v (mem_join_left h)

theorem gLams {o₀ o : Out} {A : ACtx} {k : Nat} {fs : List (Nat × List (String × Abs) × Bool)} {args : Option (List Abs)}
    {acc : Abs} (h : Grows o₀ o) : Grows o₀ (lamsAbs A k fs args acc o).2 := h.trans ((sat k).lams _ _ _ _ _)

/-- **A lambda of a function value, read with the values of a call**: where its body, evaluated with
    the names it caught and the call's values, answers, the answer read in the type of the lambda's
    answer is among what the reading answers; where it fails with one of the five kinds, the reading
    kept the evidence. -/
theorem lams_entry (IH : SoundAt C res n) {A : ACtx} (hG : Good C res A) {i : Nat} {ae : List (String × Abs)}
    {own : Bool} {lam : Lam} (hfind : C.M.lams.find? (·.id == i) = some lam) {cvals : List Abs} {ρ' : Env} {dep : Nat}
    (hS : Sat { A with ring := ringOf A.M lam.fn,
                       env := ((lam.params.map (·.1)).zip cvals).reverse ++ ae, facts := [], own := own, lam := own } ρ') :
    ∀ (k : Nat) (fs : List (Nat × List (String × Abs) × Bool)) (acc : Abs) (o : Out), (i, ae, own) ∈ fs →
    (lamsAbs A k fs (some cvals) acc o).2.widened = false →
    (∀ r, eval C n dep ρ' lam.body = .ok r → ∀ v, coerce lam.conv r = .ok v → Mem (lamsAbs A k fs (some cvals) acc o).1 v) ∧
    (∀ kd l, (eval C n dep ρ' lam.body >>= fun r => coerce lam.conv r) = .error (.failed kd l) → kd.ofEffect = true →
      Wit C (lamsAbs A k fs (some cvals) acc o).2)
  | 0, _, _, _, _, _ => by
    simp only [lamsAbs]
    exact ⟨fun _ _ v _ => Mem.any v, fun _ _ _ _ => wit_fuel⟩
  | _ + 1, [], _, _, hm, _ => by cases hm
  | k + 1, (j, ae', own') :: rest, acc, o, hm, hw => by
    rcases List.mem_cons.mp hm with heq | hrest
    · simp only [Prod.mk.injEq] at heq
      obtain ⟨rfl, rfl, rfl⟩ := heq
      have hfindA : A.M.lams.find? (·.id == i) = some lam := by rw [hG.1]; exact hfind
      simp only [lamsAbs, hfindA, Option.getD_some] at hw ⊢
      generalize hB : ({ A with ring := ringOf A.M lam.fn,
                                env := ((lam.params.map (·.1)).zip cvals).reverse ++ ae, facts := [], own := own,
                                lam := own } : ACtx) = B at hw hS ⊢
      have hGB : Good C res B := by rw [← hB]; exact hG
      have hwc := grows_not_widened (gLams (Grows.refl _)) hw
      have hwb := grows_not_widened (grows_convA (Grows.refl _)) hwc
      obtain ⟨c1, c2⟩ := IH.expr k B ρ' lam.body o dep hGB hS hwb
      refine ⟨fun r hr v hv => ?_, fun kd l he hk => ?_⟩
      · exact lams_keep k rest _ _ _ v (mem_join_right (convA_mem (c1 r hr).1 hv))
      · rcases bind_err he with he | ⟨r, hr, hc⟩
        · exact Wit.grow (c2 kd l he hk) (gLams (grows_convA (Grows.refl _)))
        · exact Wit.grow (coerce_wit (c1 r hr).1 hc).2 (gLams (Grows.refl _))
    · simp only [lamsAbs] at hw ⊢
      split at hw <;> exact lams_entry IH hG hfind hS k rest _ _ hrest hw

-- ── A lambda made a value ───────────────────────────────────────────────────

theorem case_lam {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} (hS : Sat A ρ) :
    Concl C ρ (eval C (n + 1) dep ρ (.lam id)) (evI A m (.lam id) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI, eval, AR.of]
    refine ⟨fun v hv => ?_, fun k l hv _ => by cases hv⟩
    cases hv
    refine ⟨Mem.clos (List.mem_singleton_self _) (fun x a hx => ?_) (fun x a v hx hv => ?_), hS, fun _ _ => trivial⟩
    · obtain ⟨v, hv, _⟩ := hS.env x a hx
      rw [hv]; rfl
    · obtain ⟨v', hv', hm⟩ := hS.env x a hx
      rw [hv'] at hv; cases hv; exact hm

-- ── A function value called ─────────────────────────────────────────────────

/-- **A function value called with values among the call's**: one of the lambdas the analysis says
    it can be, its body read with what the lambda caught and the values of the call (`lamsAbs`). -/
theorem callV_hit (IH : SoundAt C res n) {A : ACtx} (hG : Good C res A) {fa : Abs} {i : Nat} {env : Env}
    (hm : Mem fa (.closure i env)) {lam : Lam} (hfind : C.M.lams.find? (·.id == i) = some lam) {cvals : List Abs}
    {vs : List Val} (hall : AllMem cvals vs) {top : Abs} {o : Out} {dep : Nat} :
    ∀ k, (callVAbs A k fa cvals top o).2.widened = false →
    (∀ r, eval C n dep (((lam.params.map (·.1)).zip vs).reverse ++ env) lam.body = .ok r → ∀ v, coerce lam.conv r = .ok v →
      Mem (callVAbs A k fa cvals top o).1 v) ∧
    (∀ kd l, (eval C n dep (((lam.params.map (·.1)).zip vs).reverse ++ env) lam.body >>= fun r => coerce lam.conv r) =
      .error (.failed kd l) → kd.ofEffect = true → Wit C (callVAbs A k fa cvals top o).2)
  | 0, _ => by
    simp only [callVAbs]
    exact ⟨fun _ _ v _ => Mem.any v, fun _ _ _ _ => wit_fuel⟩
  | k + 1, hw => by
    -- a function value the analysis sees: the lambdas it can be, none it does not see
    cases fa with
    | fns fs u =>
      cases u with
      | some w => simp [callVAbs] at hw
      | none =>
        cases hm with
        | @clos _ _ _ ae own _ hin h1 h2 =>
          simp only [callVAbs] at hw ⊢
          have hS := sat_lam (A := { A with ring := ringOf A.M lam.fn, own := own, lam := own })
            (Paired.reverse (paired_zip (lam.params.map (·.1)) cvals vs hall)) h1 h2
          exact lams_entry IH hG hfind hS k fs _ o hin hw
    | _ => simp [callVAbs] at hw

theorem gCallV {o₀ o : Out} {A : ACtx} {k : Nat} {fa : Abs} {args : List Abs} {top : Abs} (h : Grows o₀ o) :
    Grows o₀ (callVAbs A k fa args top o).2 := h.trans ((sat k).callV _ _ _ _ _)

theorem case_callV (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {f : Expr}
    {args : List (Expr × Option Conv)} {top : Abs} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.callV id line f args top) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.callV id line f args top)) (evI A m (.callV id line f args top) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    have hGf : Good C res (ev A m f o).ctx := hG.same ((sat m).ev A f o).2
    simp only [evI] at hw ⊢
    split at hw
    · -- what is called is never reached
      rename_i hnever
      simp only [AR.of] at hw ⊢
      obtain ⟨f1, f2⟩ := IH.expr m A ρ f o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨fv, hfv, _⟩ := bind_ok hv
        have := (f1 fv hfv).1; rw [hnever] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨fv, hfv, _⟩
        · exact f2 k l hv hk
        · have := (f1 fv hfv).1; rw [hnever] at this; exact absurd this Mem.not_never
    · rename_i hnn
      split at hw
      · -- an argument never reached
        rename_i hnone
        simp only [AR.of] at hw ⊢
        have gl : Grows (ev A m f o).out (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.1 :=
          ((sat m).list _ _ _).1
        obtain ⟨f1, f2⟩ := IH.expr m A ρ f o dep hG hS (grows_not_widened gl hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨fv, hfv, hv1⟩ := bind_ok hv
          obtain ⟨vals, hvals, _⟩ := bind_ok hv1
          obtain ⟨_, hs, _⟩ := f1 fv hfv
          obtain ⟨avs, havs, _⟩ := (IH.list m _ ρ (args.map (·.1)) _ dep hGf hs hw).1 vals hvals
          rw [hnone] at havs; cases havs
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨fv, hfv, hv1⟩
          · exact Wit.grow (f2 k l hv hk) gl
          · obtain ⟨_, hs, _⟩ := f1 fv hfv
            rcases bind_err hv1 with hv2 | ⟨vals, hvals, _⟩
            · exact (IH.list m _ ρ (args.map (·.1)) _ dep hGf hs hw).2 k l hv2 hk
            · obtain ⟨avs, havs, _⟩ := (IH.list m _ ρ (args.map (·.1)) _ dep hGf hs hw).1 vals hvals
              rw [hnone] at havs; cases havs
      · rename_i avals hsome
        simp only [AR.of] at hw ⊢
        have gl : Grows (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.1
            (callVAbs (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.2 m (ev A m f o).abs
              (convAll (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.1 args avals).1 top
              (convAll (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.1 args avals).2).2 :=
          gCallV (convAll_grows _ _ (Grows.refl _))
        have gf : Grows (ev A m f o).out (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.1 :=
          ((sat m).list _ _ _).1
        have hwl := grows_not_widened gl hw
        obtain ⟨f1, f2⟩ := IH.expr m A ρ f o dep hG hS (grows_not_widened gf hwl)
        have hGl : Good C res (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.2 :=
          hGf.same ((sat m).list _ _ _).2
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨fv, hfv, hv1⟩ := bind_ok hv
          obtain ⟨vals, hvals, hv2⟩ := bind_ok hv1
          obtain ⟨vs, hvs, hv3⟩ := bind_ok hv2
          obtain ⟨hmf, hs, _⟩ := f1 fv hfv
          obtain ⟨avs, havs, hall, hsat⟩ := (IH.list m _ ρ (args.map (·.1)) _ dep hGf hs hwl).1 vals hvals
          rw [hsome] at havs; cases havs
          have hall' := coerceAll_ok (o := (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.1)
            (args := args) hall hvs
          refine ⟨?_, hsat, fun _ _ => trivial⟩
          cases fv with
          | closure i env =>
            simp only at hv3
            split at hv3
            · rename_i lam hfind
              split at hv3
              · cases hv3
              · obtain ⟨r, hr, hc⟩ := bind_ok hv3
                exact (callV_hit IH hGl hmf hfind hall' m hw).1 r hr v hc
            · exact absurd hv3 failK_ne_ok
          | _ => all_goals (simp only at hv3; exact absurd hv3 failK_ne_ok)
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨fv, hfv, hv1⟩
          · exact Wit.grow (f2 k l hv hk) (gf.trans gl)
          · obtain ⟨hmf, hs, _⟩ := f1 fv hfv
            have L := IH.list m _ ρ (args.map (·.1)) _ dep hGf hs hwl
            rcases bind_err hv1 with hv2 | ⟨vals, hvals, hv2⟩
            · exact Wit.grow (L.2 k l hv2 hk) gl
            · obtain ⟨avs, havs, hall, _⟩ := L.1 vals hvals
              rw [hsome] at havs; cases havs
              rcases bind_err hv2 with hv3 | ⟨vs, hvs, hv3⟩
              · exact Wit.grow (coerceAll_err hall hv3 ⟨k, l, rfl⟩) (gCallV (Grows.refl _))
              · have hall' := coerceAll_ok (o := (evList (ev A m f o).ctx m (args.map (·.1)) (ev A m f o).out).2.1)
                  (args := args) hall hvs
                cases fv with
                | closure i env =>
                  simp only at hv3
                  split at hv3
                  · rename_i lam hfind
                    split at hv3
                    · cases hv3
                    · exact (callV_hit IH hGl hmf hfind hall' m hw).2 k l hv3 hk
                  · rw [fail_type hv3] at hk; cases hk
                | _ => all_goals (simp only at hv3; rw [fail_type hv3] at hk; cases hk)

end

end KanameCore
