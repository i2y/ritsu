/-
  The forms of DESIGN 24 (kaname's DESIGN 15.2.2): a dict written out, a builtin that takes no
  function, one that applies a function to each element of a list or to each entry or value of a
  dict, `to_dict` and `merge_with`. Each answer is among what the analysis says of it, and each
  failure of one of the five kinds is one the analysis kept a place for.
-/
import KanameCore.SoundText

namespace KanameCore

theorem gPairs {o₀ o : Out} {A : ACtx} {m : Nat} {pairs : List (Expr × Option Conv × Expr × Option Conv)}
    {ks vs : Abs} {k : Nat} (h : Grows o₀ o) : Grows o₀ (pairsAbs A m pairs ks vs k o).out :=
  h.trans ((sat m).pairs _ _ _ _ _ _).1
theorem gLam1 {o₀ o : Out} {A : ACtx} {t : Option Term} {m : Nat} {x : String} {elem : Abs} {body : Expr}
    {c : Option Conv} (h : Grows o₀ o) : Grows o₀ (lam1 A t m x elem body c o).2 := h.trans ((sat m).lam1 _ _ _ _ _ _ _)
theorem gCond {o₀ o : Out} {A : ACtx} {t : Option Term} {m : Nat} {x : String} {elem : Abs} {body : Expr}
    {c : Option Conv} (h : Grows o₀ o) : Grows o₀ (condition A t m x elem body c o).2 := h.trans ((sat m).cond _ _ _ _ _ _ _)

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

-- ── A dict written out ──────────────────────────────────────────────────────

/-- The pairs written out, the later of two of one key kept: from one pair to as many as are written. -/
theorem dictLit_mem {key : Key} {ks vs : Abs} {k : Nat} {pre : List (Val × Val)}
    (hpre : ∀ p ∈ pre, Mem ks p.1 ∧ Mem vs p.2) (hk : pre.length = k) :
    Mem (.dict (Iv.ints ((min k 1 : Nat) : Int) (k : Int)) ks vs) (.dict (dictFrom key pre)) := by
  refine Mem.dict ?_ (fun q hq => (hpre q (dictFrom_mem hq)).1) (fun q hq => (hpre q (dictFrom_mem hq)).2)
    (dictFrom_nodup key pre)
  have hle := dictFrom_length_le key pre
  have hlo : ((min k 1 : Nat) : Int) ≤ ((dictFrom key pre).length : Int) := by
    cases pre with
    | nil => subst hk; simp
    | cons p rest =>
      have hne := dictFrom_ne_nil (k := key) (List.cons_ne_nil p rest)
      have : 1 ≤ (dictFrom key (p :: rest)).length := List.length_pos_iff.mpr hne
      omega
  have hhi : ((dictFrom key pre).length : Int) ≤ (k : Int) := by omega
  exact In_ints hlo hhi

theorem pairs_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env)
    (pairs : List (Expr × Option Conv × Expr × Option Conv)) (ks vs : Abs) (k : Nat) (o : Out) (dep : Nat)
    (pre : List (Val × Val)) (key : Key) (hG : Good C res A) (hS : Sat A ρ)
    (hw : (pairsAbs A m pairs ks vs k o).out.widened = false) (hpre : ∀ p ∈ pre, Mem ks p.1 ∧ Mem vs p.2)
    (hk : pre.length = k) :
    (∀ ps, evalPairs C (n + 1) dep ρ pairs = .ok ps →
        Mem (pairsAbs A m pairs ks vs k o).abs (.dict (dictFrom key (pre ++ ps))) ∧
        Sat (pairsAbs A m pairs ks vs k o).ctx ρ) ∧
    (∀ k' l, evalPairs C (n + 1) dep ρ pairs = .error (.failed k' l) → k'.ofEffect = true →
      Wit C (pairsAbs A m pairs ks vs k o).out) := by
  cases m with
  | zero =>
    simp only [pairsAbs]
    exact ⟨fun _ _ => ⟨Mem.any _, hS⟩, fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    cases pairs with
    | nil =>
      simp only [pairsAbs, AR.of]
      refine ⟨fun ps h => ?_, fun k' l h => ?_⟩
      · simp [evalPairs] at h; subst h
        simp only [List.append_nil]
        exact ⟨dictLit_mem hpre hk, hS⟩
      · simp [evalPairs] at h
    | cons p rest =>
      obtain ⟨x, cx, y, cy⟩ := p
      simp only [pairsAbs] at hw ⊢
      split at hw
      · -- no key comes
        rename_i hn
        simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS hw
        refine ⟨fun ps h => ?_, fun k' l h hk' => ?_⟩
        · simp only [evalPairs] at h
          obtain ⟨v, hv, _⟩ := bind_ok h
          have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
        · simp only [evalPairs] at h
          rcases bind_err h with h | ⟨v, hv, _⟩
          · exact h2 k' l h hk'
          · have := (h1 v hv).1; rw [hn] at this; exact absurd this Mem.not_never
      · have hGx : Good C res (ev A m x o).ctx := hG.same ((sat m).ev A x o).2
        split at hw
        · -- no value comes
          rename_i _ hn
          simp only [AR.of] at hw ⊢
          obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened (gEv (grows_convA (Grows.refl _))) hw)
          refine ⟨fun ps h => ?_, fun k' l h hk' => ?_⟩
          · simp only [evalPairs] at h
            obtain ⟨v, hv, h3⟩ := bind_ok h
            obtain ⟨v', hv', h4⟩ := bind_ok h3
            obtain ⟨u, hu, _⟩ := bind_ok h4
            obtain ⟨_, hs, _⟩ := h1 v hv
            have := (IH.expr m _ ρ y _ dep hGx hs hw).1 u hu
            rw [hn] at this; exact absurd this.1 Mem.not_never
          · simp only [evalPairs] at h
            rcases bind_err h with h | ⟨v, hv, h3⟩
            · exact Wit.grow (h2 k' l h hk') (gEv (grows_convA (Grows.refl _)))
            · obtain ⟨hm, hs, _⟩ := h1 v hv
              obtain ⟨y1, y2⟩ := IH.expr m _ ρ y _ dep hGx hs hw
              rcases bind_err h3 with h4 | ⟨v', _, h4⟩
              · exact Wit.grow (coerce_wit hm h4).2 (gEv (Grows.refl _))
              · rcases bind_err h4 with h5 | ⟨u, hu, _⟩
                · exact y2 k' l h5 hk'
                · have := (y1 u hu).1; rw [hn] at this; exact absurd this Mem.not_never
        · have gx : Grows (ev A m x o).out (pairsAbs (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).ctx m rest
              (ks.join (convA (ev A m x o).out cx (ev A m x o).abs).1)
              (vs.join (convA (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).out cy
                (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).abs).1) (k + 1)
              (convA (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).out cy
                (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).abs).2).out :=
            gPairs (grows_convA (gEv (grows_convA (Grows.refl _))))
          have gy : Grows (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).out
              (pairsAbs (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).ctx m rest
              (ks.join (convA (ev A m x o).out cx (ev A m x o).abs).1)
              (vs.join (convA (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).out cy
                (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).abs).1) (k + 1)
              (convA (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).out cy
                (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).abs).2).out :=
            gPairs (grows_convA (Grows.refl _))
          obtain ⟨h1, h2⟩ := IH.expr m A ρ x o dep hG hS (grows_not_widened gx hw)
          refine ⟨fun ps h => ?_, fun k' l h hk' => ?_⟩
          · simp only [evalPairs] at h
            obtain ⟨v, hv, h3⟩ := bind_ok h
            obtain ⟨v', hv', h4⟩ := bind_ok h3
            obtain ⟨u, hu, h5⟩ := bind_ok h4
            obtain ⟨u', hu', h6⟩ := bind_ok h5
            obtain ⟨more, hmore, h7⟩ := bind_ok h6
            cases h7
            obtain ⟨hm, hs, _⟩ := h1 v hv
            obtain ⟨y1, _⟩ := IH.expr m _ ρ y _ dep hGx hs (grows_not_widened gy hw)
            obtain ⟨hmu, hsu, _⟩ := y1 u hu
            have hGy := hGx.same ((sat m).ev _ y (convA (ev A m x o).out cx (ev A m x o).abs).2).2
            have hpre' : ∀ p ∈ pre ++ [(v', u')],
                Mem (ks.join (convA (ev A m x o).out cx (ev A m x o).abs).1) p.1 ∧
                Mem (vs.join (convA (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).out cy
                  (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).abs).1) p.2 := by
              intro p hp
              rcases List.mem_append.mp hp with hp | hp
              · exact ⟨mem_join_left (hpre p hp).1, mem_join_left (hpre p hp).2⟩
              · simp at hp; subst hp
                exact ⟨mem_join_right (convA_mem hm hv'), mem_join_right (convA_mem hmu hu')⟩
            obtain ⟨e1, e2⟩ := (IH.pairs m _ ρ rest _ _ (k + 1) _ dep (pre ++ [(v', u')]) key hGy hsu hw hpre'
              (by simp [hk])).1 more hmore
            refine ⟨?_, e2⟩
            have : pre ++ (v', u') :: more = (pre ++ [(v', u')]) ++ more := by simp
            rw [this]; exact e1
          · simp only [evalPairs] at h
            rcases bind_err h with h | ⟨v, hv, h3⟩
            · exact Wit.grow (h2 k' l h hk') gx
            · obtain ⟨hm, hs, _⟩ := h1 v hv
              obtain ⟨y1, y2⟩ := IH.expr m _ ρ y _ dep hGx hs (grows_not_widened gy hw)
              rcases bind_err h3 with h4 | ⟨v', hv', h4⟩
              · exact Wit.grow (coerce_wit hm h4).2 (gPairs (grows_convA (gEv (Grows.refl _))))
              · rcases bind_err h4 with h5 | ⟨u, hu, h5⟩
                · exact Wit.grow (y2 k' l h5 hk') gy
                · obtain ⟨hmu, hsu, _⟩ := y1 u hu
                  rcases bind_err h5 with h6 | ⟨u', hu', h6⟩
                  · exact Wit.grow (coerce_wit hmu h6).2 (gPairs (Grows.refl _))
                  · rcases bind_err h6 with h7 | ⟨more, _, h7⟩
                    · have hGy := hGx.same ((sat m).ev _ y (convA (ev A m x o).out cx (ev A m x o).abs).2).2
                      have hpre' : ∀ p ∈ pre ++ [(v', u')],
                          Mem (ks.join (convA (ev A m x o).out cx (ev A m x o).abs).1) p.1 ∧
                          Mem (vs.join (convA (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).out cy
                            (ev (ev A m x o).ctx m y (convA (ev A m x o).out cx (ev A m x o).abs).2).abs).1) p.2 := by
                        intro p hp
                        rcases List.mem_append.mp hp with hp | hp
                        · exact ⟨mem_join_left (hpre p hp).1, mem_join_left (hpre p hp).2⟩
                        · simp at hp; subst hp
                          exact ⟨mem_join_right (convA_mem hm hv'), mem_join_right (convA_mem hmu hu')⟩
                      exact (IH.pairs m _ ρ rest _ _ (k + 1) _ dep (pre ++ [(v', u')]) key hGy hsu hw hpre'
                        (by simp [hk])).2 k' l h7 hk'
                    · cases h7

-- ── The functions given to `to_dict` and `merge_with` ──────────────────────

/-- **A function given to a builtin, once**: what it answers for an element. -/
theorem lam1_step (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {x : String} {body : Expr}
    {c : Option Conv} {elem : Abs} {el : Val} {o : Out} {dep : Nat} (hG : Good C res A) (hS : Sat A ρ)
    (he : Mem elem el) (hw : (lam1 A none m x elem body c o).2.widened = false) :
    (∀ a', (eval C n dep ((x, el) :: ρ) body >>= coerce c) = .ok a' → Mem (lam1 A none m x elem body c o).1 a') ∧
    (∀ k l, (eval C n dep ((x, el) :: ρ) body >>= coerce c) = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam1 A none m x elem body c o).2) := by
  cases m with
  | zero => simp only [lam1]; exact ⟨fun a' _ => Mem.any a', fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    simp only [lam1, ACtx.bindLam_none] at hw ⊢
    have hG' : Good C res (A.bindNew x elem) := hG
    obtain ⟨h1, h2⟩ := IH.expr m (A.bindNew x elem) _ body o dep hG' (hS.bindNew he)
      (grows_not_widened (grows_convA (Grows.refl _)) hw)
    refine ⟨fun a' h => ?_, fun k l h hk => ?_⟩
    · obtain ⟨v, hv, hc⟩ := bind_ok h
      exact convA_mem (h1 v hv).1 hc
    · rcases bind_err h with h | ⟨v, hv, h⟩
      · exact Wit.grow (h2 k l h hk) (grows_convA (Grows.refl _))
      · exact (coerce_wit (h1 v hv).1 h).2

theorem each2_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (x : String) (kb : Expr) (kc : Option Conv)
    (y : String) (vb : Expr) (vc : Option Conv) (xs : List Val) (elem : Abs) (o : Out) (dep : Nat)
    (hG : Good C res A) (hS : Sat A ρ) (hel : ∀ el ∈ xs, Mem elem el)
    (hw : (lam1 A none m y elem vb vc (lam1 A none m x elem kb kc o).2).2.widened = false) :
    (∀ ps, evalEach2 C (n + 1) dep ρ x kb kc y vb vc xs = .ok ps → ps.length = xs.length ∧
      ∀ p ∈ ps, Mem (lam1 A none m x elem kb kc o).1 p.1 ∧ Mem (lam1 A none m y elem vb vc (lam1 A none m x elem kb kc o).2).1 p.2) ∧
    (∀ k l, evalEach2 C (n + 1) dep ρ x kb kc y vb vc xs = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam1 A none m y elem vb vc (lam1 A none m x elem kb kc o).2).2) := by
  cases xs with
  | nil =>
    refine ⟨fun ps h => ?_, fun k l h => ?_⟩
    · simp [evalEach2] at h; subst h; simp
    · simp [evalEach2] at h
  | cons el rest =>
    have he : Mem elem el := hel el (List.mem_cons_self ..)
    have hrest : ∀ e ∈ rest, Mem elem e := fun e h => hel e (List.mem_cons_of_mem _ h)
    have gk : Grows (lam1 A none m x elem kb kc o).2 (lam1 A none m y elem vb vc (lam1 A none m x elem kb kc o).2).2 :=
      gLam1 (Grows.refl _)
    obtain ⟨k1, k2⟩ := lam1_step (dep := dep) IH hG hS he (grows_not_widened gk hw) (x := x) (body := kb) (c := kc)
    obtain ⟨v1, v2⟩ := lam1_step (dep := dep) IH hG hS he hw (x := y) (body := vb) (c := vc)
    obtain ⟨r1, r2⟩ := IH.each2 m A ρ x kb kc y vb vc rest elem o dep hG hS hrest hw
    refine ⟨fun ps h => ?_, fun k l h hk => ?_⟩
    · simp only [evalEach2] at h
      obtain ⟨a, ha, h1⟩ := bind_ok h
      obtain ⟨a', ha', h2⟩ := bind_ok h1
      obtain ⟨b, hb, h3⟩ := bind_ok h2
      obtain ⟨b', hb', h4⟩ := bind_ok h3
      obtain ⟨more, hmore, h5⟩ := bind_ok h4
      cases h5
      obtain ⟨l1, l2⟩ := r1 more hmore
      refine ⟨by simp [l1], fun p hp => ?_⟩
      simp only [List.mem_cons] at hp
      rcases hp with rfl | hp
      · exact ⟨k1 a' (bind_of ha ha'), v1 b' (bind_of hb hb')⟩
      · exact l2 p hp
    · simp only [evalEach2] at h
      rcases bind_err h with h | ⟨a, ha, h1⟩
      · exact Wit.grow (k2 k l (by rw [h]; rfl) hk) gk
      · rcases bind_err h1 with h2 | ⟨a', _, h2⟩
        · exact Wit.grow (k2 k l (bind_of ha h2) hk) gk
        · rcases bind_err h2 with h3 | ⟨b, hb, h3⟩
          · exact v2 k l (by rw [h3]; rfl) hk
          · rcases bind_err h3 with h4 | ⟨b', _, h4⟩
            · exact v2 k l (bind_of hb h4) hk
            · rcases bind_err h4 with h5 | ⟨more, _, h5⟩
              · exact r2 k l h5 hk
              · cases h5

theorem merge_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (key : Key) (ps : List (Val × Val))
    (x y : String) (body : Expr) (c : Option Conv) (va vb : Abs) (qs : List (Val × Val)) (o : Out) (dep : Nat)
    (hG : Good C res A) (hS : Sat A ρ) (hps : ∀ p ∈ ps, Mem va p.2) (hqs : ∀ q ∈ qs, Mem vb q.2)
    (hw : (lam2 A m x y va vb body c o).2.widened = false) :
    (∀ more, evalMerge C (n + 1) dep ρ key ps x y body c qs = .ok more → more.map (·.1) = qs.map (·.1) ∧
      ∀ q ∈ more, Mem vb q.2 ∨ Mem (lam2 A m x y va vb body c o).1 q.2) ∧
    (∀ k l, evalMerge C (n + 1) dep ρ key ps x y body c qs = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam2 A m x y va vb body c o).2) := by
  cases qs with
  | nil =>
    refine ⟨fun more h => ?_, fun k l h => ?_⟩
    · simp [evalMerge] at h; subst h; simp
    · simp [evalMerge] at h
  | cons q rest =>
    obtain ⟨kq, w⟩ := q
    have hw' : Mem vb w := hqs (kq, w) (List.mem_cons_self ..)
    have hrest : ∀ q ∈ rest, Mem vb q.2 := fun q h => hqs q (List.mem_cons_of_mem _ h)
    obtain ⟨r1, r2⟩ := IH.merge m A ρ key ps x y body c va vb rest o dep hG hS hps hrest hw
    refine ⟨fun more h => ?_, fun k l h hk => ?_⟩
    · simp only [evalMerge] at h
      split at h
      · rename_i kp va' hf
        have hmem := List.mem_of_find?_eq_some hf
        obtain ⟨s1, _⟩ := lam2_step (dep := dep) IH hG hS hw' (hps _ hmem) hw (acc := x) (x := y) (body := body) (c := c)
        obtain ⟨r0, hr0, h1⟩ := bind_ok h
        obtain ⟨u, hu, h2⟩ := bind_ok h1
        obtain ⟨more', hmore, h3⟩ := bind_ok h2
        cases h3
        obtain ⟨l1, l2⟩ := r1 more' hmore
        refine ⟨by simp [l1], fun q hq => ?_⟩
        simp only [List.mem_cons] at hq
        rcases hq with rfl | hq
        · exact Or.inr (s1 u (bind_of hr0 hu))
        · exact l2 q hq
      · obtain ⟨u, hu, h1⟩ := bind_ok h
        cases hu
        obtain ⟨more', hmore, h2⟩ := bind_ok h1
        cases h2
        obtain ⟨l1, l2⟩ := r1 more' hmore
        refine ⟨by simp [l1], fun q hq => ?_⟩
        simp only [List.mem_cons] at hq
        rcases hq with rfl | hq
        · exact Or.inl hw'
        · exact l2 q hq
    · simp only [evalMerge] at h
      split at h
      · rename_i kp va' hf
        have hmem := List.mem_of_find?_eq_some hf
        obtain ⟨_, s2⟩ := lam2_step (dep := dep) IH hG hS hw' (hps _ hmem) hw (acc := x) (x := y) (body := body) (c := c)
        rcases bind_err h with h1 | ⟨r0, hr0, h1⟩
        · exact s2 k l (by rw [h1]; rfl) hk
        · rcases bind_err h1 with h2 | ⟨u, _, h2⟩
          · exact s2 k l (bind_of hr0 h2) hk
          · rcases bind_err h2 with h3 | ⟨more', _, h3⟩
            · exact r2 k l h3 hk
            · cases h3
      · rcases bind_err h with h1 | ⟨u, _, h1⟩
        · cases h1
        · rcases bind_err h1 with h2 | ⟨more', _, h2⟩
          · exact r2 k l h2 hk
          · cases h2

-- ── What a builtin's answer is held to ──────────────────────────────────────

theorem sized_not_failed {v : Val} {k : Kind} {l : Nat} (h : sized v = .error (.failed k l)) : False := by
  unfold sized at h
  split at h <;> (try split at h) <;> simp at h

theorem coerceAll_ok {o : Out} : ∀ {args : List (Expr × Option Conv)} {avals : List Abs} {vs vs' : List Val},
    AllMem avals vs → coerceAll args vs = .ok vs' → AllMem (convAll o args avals).1 vs'
  | (_, c) :: rest, a :: as, v :: vs, vs', hall, h => by
    simp only [coerceAll] at h
    obtain ⟨v', hv', h1⟩ := bind_ok h
    obtain ⟨more, hmore, h2⟩ := bind_ok h1
    cases h2
    simp only [convAll, AllMem] at hall ⊢
    exact ⟨convA_mem hall.1 hv', coerceAll_ok hall.2 hmore⟩
  | [], _, _, vs', _, h => by simp [coerceAll] at h; subst h; simp [convAll, AllMem]
  | _ :: _, [], [], vs', _, h => by simp [coerceAll] at h; subst h; simp [convAll, AllMem]
  | _ :: _, _ :: _, [], _, hall, _ => by simp [AllMem] at hall
  | _ :: _, [], _ :: _, _, hall, _ => by simp [AllMem] at hall

theorem coerceAll_err {o : Out} : ∀ {args : List (Expr × Option Conv)} {avals : List Abs} {vs : List Val} {e : Stop},
    AllMem avals vs → coerceAll args vs = .error e → (∃ k l, e = .failed k l) → Wit C (convAll o args avals).2
  | (_, c) :: rest, a :: as, v :: vs, e, hall, h, he => by
    simp only [coerceAll] at h
    simp only [AllMem] at hall
    simp only [convAll]
    rcases bind_err h with h | ⟨v', _, h1⟩
    · exact Wit.grow (coerce_wit hall.1 h).2 (convAll_grows _ _ (Grows.refl _))
    · rcases bind_err h1 with h2 | ⟨more, _, h2⟩
      · exact coerceAll_err hall.2 h2 he
      · cases h2
  | [], _, _, e, _, h, _ => by simp [coerceAll] at h
  | _ :: _, _, [], e, _, h, _ => by simp [coerceAll] at h
  | _ :: _, [], _ :: _, _, hall, _, _ => by simp [AllMem] at hall

/-- A builtin other than `numbers` fails only where the check of types does not let it through. -/
theorem primVal_type {line : Nat} {op : Prim} {key : Key} {vs : List Val} {k : Kind} {l : Nat}
    (hop : op ≠ .numbers) (hv : primVal line op key vs = .error (.failed k l)) : k = .type := by
  unfold primVal at hv
  split at hv
  all_goals first
    | (cases hv; done)
    | (simp only [failK, Except.error.injEq, Stop.failed.injEq] at hv; exact hv.1.symm)
    | (exact absurd rfl hop)
    | (repeat' split at hv) <;> first
        | (cases hv; done)
        | (simp only [failK, Except.error.injEq, Stop.failed.injEq] at hv; exact hv.1.symm)

/-- **A builtin that fails with one of the five kinds**: `numbers` whose count is past int64, where the
    analysis kept its place. -/
theorem primVal_err {o : Out} {id line : Nat} {op : Prim} {key : Key} {avals : List Abs} {vs : List Val}
    {k : Kind} {l : Nat} (hall : AllMem avals vs) (hv : primVal line op key vs = .error (.failed k l))
    (hk : k.ofEffect = true) : Wit C (primAbs o id op avals).2 := by
  by_cases hop : op = .numbers
  · subst hop
    match vs, hall, hv with
    | [a, b], hall, hv =>
      simp only [primVal] at hv
      split at hv
      · rename_i i j hi hj
        obtain ⟨r, rfl, hr, rfl⟩ := wholeOf_some hi
        obtain ⟨q, rfl, hq, rfl⟩ := wholeOf_some hj
        obtain ⟨a0, as1, rfl, h0, h1⟩ := allMem_cons hall
        obtain ⟨a1, as2, rfl, h1', h2⟩ := allMem_cons h1
        obtain rfl := allMem_nil h2
        split at hv
        · cases hv
        · split at hv
          · rename_i hbig
            have ha := In_ivD h0
            have hb := In_ivD h1'
            rw [rat_of_den_one hr] at ha
            rw [rat_of_den_one hq] at hb
            have hraw := numbers_raw ha hb
            simp only [primAbs, List.headD, List.getD_cons_succ, List.getD_cons_zero]
            have hm : Mem (.num { (((a1.iv.getD Iv.top).minus (a0.iv.getD Iv.top)).plus (Iv.exact 1)) with
                lo := (((a1.iv.getD Iv.top).minus (a0.iv.getD Iv.top)).plus (Iv.exact 1)).lo.map nonNeg })
                (.num (((q.num - r.num + 1 : Int)) : Rat)) := by
              refine Mem.num ⟨?_, hraw.hi, hraw.den⟩
              cases hlo : (((a1.iv.getD Iv.top).minus (a0.iv.getD Iv.top)).plus (Iv.exact 1)).lo with
              | none => simp [EndLe]
              | some lv =>
                have el := hraw.lo; rw [hlo] at el; simp only [EndLe] at el
                simp only [Option.map_some, EndLe, nonNeg_eq]
                have : (0 : Rat) ≤ (((q.num - r.num + 1 : Int)) : Rat) := by
                  exact_mod_cast (show (0 : Int) ≤ q.num - r.num + 1 by omega)
                split <;> grind
            have hf : FailsAt .overflow (.num (((q.num - r.num + 1 : Int)) : Rat)) := by
              refine ⟨_, rfl, ?_⟩
              unfold fits
              rw [Rat.num_intCast]
              have hn : decide ((q.num - r.num + 1) ≤ i64max) = false := decide_eq_false (by omega)
              rw [hn]
              simp
            split <;> exact wit_place hm hf
          · split at hv <;> cases hv
      · simp only [failK, Except.error.injEq, Stop.failed.injEq] at hv
        rw [← hv.1] at hk; simp [Kind.ofEffect] at hk
    | [], _, hv | [_], _, hv | _ :: _ :: _ :: _, _, hv =>
      simp only [primVal, failK, Except.error.injEq, Stop.failed.injEq] at hv
      rw [← hv.1] at hk; simp [Kind.ofEffect] at hk
  · have := primVal_type hop hv
    subst this; simp [Kind.ofEffect] at hk

-- ── A dict written out ──────────────────────────────────────────────────────

theorem case_dict (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id : Nat} {key : Key}
    {pairs : List (Expr × Option Conv × Expr × Option Conv)} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.dict id key pairs) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.dict id key pairs)) (evI A m (.dict id key pairs) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    obtain ⟨e1, e2⟩ := IH.pairs m A ρ pairs .never .never 0 o dep [] key hG hS hw (fun _ h => by cases h) rfl
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨ps, hps, hv1⟩ := bind_ok hv
      obtain rfl := sized_eq hv1
      obtain ⟨hm, hs⟩ := e1 ps hps
      exact ⟨by simpa using hm, hs, fun t ht => by cases ht⟩
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨ps, _, hv1⟩
      · exact e2 k l hv hk
      · exact absurd hv1 (fun h => sized_not_failed h)

-- ── A builtin that takes no function ────────────────────────────────────────

theorem case_prim (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {op : Prim}
    {key : Key} {args : List (Expr × Option Conv)} {top : Option Abs} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.prim id line op key args top) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.prim id line op key args top)) (evI A m (.prim id line op key args top) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    split at hw
    · rename_i hnone
      simp only [AR.of] at hw ⊢
      obtain ⟨a1, a2⟩ := IH.list m A ρ (args.map (·.1)) o dep hG hS hw
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
    · rename_i avals hsome
      simp only [AR.of] at hw ⊢
      have gl : Grows (evList A m (args.map (·.1)) o).2.1
          (primAbs (convAll (evList A m (args.map (·.1)) o).2.1 args avals).2 id op
            (convAll (evList A m (args.map (·.1)) o).2.1 args avals).1).2 :=
        primAbs_grows (convAll_grows _ _ (Grows.refl _))
      obtain ⟨a1, a2⟩ := IH.list m A ρ (args.map (·.1)) o dep hG hS (grows_not_widened gl hw)
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨vals, hvals, hv1⟩ := bind_ok hv
        obtain ⟨vals', hvals', hv2⟩ := bind_ok hv1
        obtain ⟨u, hu, hv3⟩ := bind_ok hv2
        obtain ⟨u', hu', hv4⟩ := bind_ok hv3
        obtain ⟨avs, havs, hall, hsat⟩ := a1 vals hvals
        rw [hsome] at havs; cases havs
        have hall' := coerceAll_ok (o := (evList A m (args.map (·.1)) o).2.1) (args := args) hall hvals'
        have hm := primAbs_sound (o := (convAll (evList A m (args.map (·.1)) o).2.1 args avals).2) (id := id) hall' hu hu'
        refine ⟨?_, hsat, fun t ht => ?_⟩
        · cases top with
          | none => cases hv4; exact hm
          | some t =>
            simp only at hv4
            split at hv4
            · rename_i hhas; cases hv4; exact has_sound' hhas
            · exact absurd hv4 failK_ne_ok
        · trivial
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨vals, hvals, hv1⟩
        · exact Wit.grow (a2 k l hv hk) gl
        · obtain ⟨avs, havs, hall, _⟩ := a1 vals hvals
          rw [hsome] at havs; cases havs
          rcases bind_err hv1 with hv2 | ⟨vals', hvals', hv2⟩
          · exact Wit.grow (coerceAll_err hall hv2 ⟨k, l, rfl⟩) (primAbs_grows (Grows.refl _))
          · have hall' := coerceAll_ok (o := (evList A m (args.map (·.1)) o).2.1) (args := args) hall hvals'
            rcases bind_err hv2 with hv3 | ⟨u, _, hv3⟩
            · exact primVal_err hall' hv3 hk
            · rcases bind_err hv3 with hv4 | ⟨u', _, hv4⟩
              · exact absurd hv4 (fun h => sized_not_failed h)
              · cases top with
                | none => cases hv4
                | some t =>
                  simp only at hv4
                  split at hv4
                  · cases hv4
                  · rw [fail_type hv4] at hk; cases hk

-- ── The builtins that apply a function to each element ──────────────────────

theorem stopAt_map (v : Val) : stopAt .map v = false := by
  unfold stopAt; split <;> simp_all

theorem walked_not_neg {a : Abs} {xs : List Val} (h : Mem a (.list xs)) :
    ((walked a).1.hi.any fun h => h.num < 0) = false := by
  have h1 := (walked_sound h).1.hi
  rcases hch : (walked a).1.hi with _ | u
  · rfl
  · rw [hch] at h1
    simp only [EndGe] at h1
    have h0 : (0 : Rat) ≤ (xs.length : Rat) := by exact_mod_cast Nat.zero_le _
    have : 0 ≤ u := Rat.le_trans h0 h1
    have : 0 ≤ u.num := (@Rat.num_nonneg u).mpr this
    simp only [Option.any_some, decide_eq_false_iff_not]
    omega

theorem bestBy_some {k : Key} {want : Ordering} : ∀ (b : Val × Val) (l : List (Val × Val)),
    bestBy k want (some b) l ≠ none
  | b, [] => by simp [bestBy]
  | b, p :: ps => by simp only [bestBy]; split <;> exact bestBy_some _ ps

theorem bestBy_mem {k : Key} {want : Ordering} : ∀ (best : Option (Val × Val)) (l : List (Val × Val)) (b : Val × Val),
    bestBy k want best l = some b → best = some b ∨ b ∈ l
  | best, [], b, h => by simp only [bestBy] at h; exact Or.inl h
  | none, p :: ps, b, h => by
    simp only [bestBy] at h
    rcases bestBy_mem (some p) ps b h with h1 | h1
    · cases h1; exact Or.inr List.mem_cons_self
    · exact Or.inr (List.mem_cons_of_mem _ h1)
  | some c, p :: ps, b, h => by
    simp only [bestBy] at h
    rcases bestBy_mem _ ps b h with h1 | h1
    · split at h1
      · cases h1; exact Or.inr List.mem_cons_self
      · cases h1; exact Or.inl rfl
    · exact Or.inr (List.mem_cons_of_mem _ h1)

theorem bestBy_none {k : Key} {want : Ordering} : ∀ (l : List (Val × Val)), bestBy k want none l = none → l = []
  | [], _ => rfl
  | p :: ps, h => by simp only [bestBy] at h; exact absurd h (bestBy_some p ps)

theorem groupsOf_perm (k : Key) (ps : List (Val × Val)) :
    (groupsOf k ps).Perm ((firstOfEach (ps.map (·.1))).map fun key =>
      (key, Val.list ((ps.filter fun p => sameKey p.1 key).map (·.2)))) :=
  List.mergeSort_perm _ _

theorem groupsOf_length (k : Key) (ps : List (Val × Val)) :
    (groupsOf k ps).length = (firstOfEach (ps.map (·.1))).length := by
  rw [(groupsOf_perm k ps).length_eq]; simp

theorem groupsOf_nodup (k : Key) (ps : List (Val × Val)) : ((groupsOf k ps).map (·.1)).Nodup := by
  have hp := (groupsOf_perm k ps).map (·.1)
  rw [hp.nodup_iff]
  simp only [List.map_map]
  have : ((·.1) ∘ fun key => (key, Val.list ((ps.filter fun p => sameKey p.1 key).map (·.2)))) = (id : Val → Val) := by
    funext key; rfl
  rw [this, List.map_id]
  exact firstOfEach_nodup _

theorem groupsOf_mem {k : Key} {ps : List (Val × Val)} {g : Val × Val} (h : g ∈ groupsOf k ps) :
    g.1 ∈ ps.map (·.1) ∧ g.2 = .list ((ps.filter fun p => sameKey p.1 g.1).map (·.2)) := by
  have := (groupsOf_perm k ps).mem_iff.mp h
  obtain ⟨key, hkey, rfl⟩ := List.mem_map.mp this
  exact ⟨firstOfEach_mem.mp hkey, rfl⟩

/-- **A list walked by a function** (`sort_by`, `max_by`, `flat_map`, `filter_map`, `group_by` and the
    rest): the list, the answers of the function for its elements, and what is left to answer. -/
theorem walkK_list_ok (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {w : WalkK} {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key} {v : Val}
    (hG : Good C res A) (hS : Sat A ρ) (hlw : w ≠ .filterD ∧ w ≠ .mapValues ∧ w ≠ .last)
    (hwr : (ev A m coll o).out.widened = false)
    (hv : eval C (n + 1) dep ρ (.walkK id line w coll x body c key) = .ok v) :
    ∃ xs answers, Sat (ev A m coll o).ctx ρ ∧ ((walked (ev A m coll o).abs).1.hi.any fun h => h.num < 0) = false ∧
      (walked (ev A m coll o).abs).1.In (xs.length : Rat) ∧ (∀ y ∈ xs, Mem (walked (ev A m coll o).abs).2 y) ∧
      answers.length = xs.length ∧
      ((lam1 (ev A m coll o).ctx (totalsOf coll) m x (walked (ev A m coll o).abs).2 body c (ev A m coll o).out).2.widened = false →
        ∀ a ∈ answers, Mem (lam1 (ev A m coll o).ctx (totalsOf coll) m x (walked (ev A m coll o).abs).2 body c (ev A m coll o).out).1 a) ∧
      (if (w == .flatMap && !answers.all (fun a => (listOf a).isSome)) = true then failK .type line
        else if (w == .groupBy && decide (i64max < (xs.length : Int))) = true then .error .diverged
        else sized (foldK w key xs answers)) = .ok v := by
  have hG1 : Good C res (ev A m coll o).ctx := hG.same ((sat m).ev A coll o).2
  obtain ⟨hl1, _⟩ := IH.expr m A ρ coll o dep hG hS hwr
  simp only [eval] at hv
  obtain ⟨cv, hcv, hv⟩ := bind_ok hv
  obtain ⟨hml, hsl, hnr⟩ := hl1 cv hcv
  obtain ⟨hl, hr, hlast⟩ := hlw
  cases cv with
  | list xs =>
    obtain ⟨hcnt, helem⟩ := walked_sound hml
    have hneg := walked_not_neg hml
    cases w
    case filterD => exact absurd rfl hl
    case mapValues => exact absurd rfl hr
    case last => exact absurd rfl hlast
    all_goals
      simp only at hv
      obtain ⟨answers, has, hv⟩ := bind_ok hv
      have hlen := (evalEach_shape n dep ρ x body c .map xs answers has).2.1 (fun a _ => stopAt_map a)
      exact ⟨xs, answers, hsl, hneg, hcnt, helem, hlen,
        fun hwl a ha => (lamBody IH hG1 hsl helem (totOK_of_eval hcv) hwl (w := .map) (dep := dep)).1 answers has a ha, hv⟩
  | _ =>
    cases w <;> simp only at hv <;> first | exact absurd rfl hl | exact absurd rfl hr | exact absurd hv failK_ne_ok

theorem walkK_list_err (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {w : WalkK} {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key} {k : Kind} {l : Nat}
    (hG : Good C res A) (hS : Sat A ρ) (hlw : w ≠ .filterD ∧ w ≠ .mapValues ∧ w ≠ .last)
    (hwr : (ev A m coll o).out.widened = false)
    (hv : eval C (n + 1) dep ρ (.walkK id line w coll x body c key) = .error (.failed k l)) (hk : k.ofEffect = true) :
    Wit C (ev A m coll o).out ∨ (((walked (ev A m coll o).abs).1.hi.any fun h => h.num < 0) = false ∧
      ((lam1 (ev A m coll o).ctx (totalsOf coll) m x (walked (ev A m coll o).abs).2 body c (ev A m coll o).out).2.widened = false →
        Wit C (lam1 (ev A m coll o).ctx (totalsOf coll) m x (walked (ev A m coll o).abs).2 body c (ev A m coll o).out).2)) := by
  have hG1 : Good C res (ev A m coll o).ctx := hG.same ((sat m).ev A coll o).2
  obtain ⟨hl1, hl2⟩ := IH.expr m A ρ coll o dep hG hS hwr
  simp only [eval] at hv
  rcases bind_err hv with hv | ⟨cv, hcv, hv⟩
  · exact Or.inl (hl2 k l hv hk)
  obtain ⟨hml, hsl, hnr⟩ := hl1 cv hcv
  obtain ⟨hl, hr, hlast⟩ := hlw
  cases cv with
  | list xs =>
    obtain ⟨hcnt, helem⟩ := walked_sound hml
    have hneg := walked_not_neg hml
    cases w
    case filterD => exact absurd rfl hl
    case mapValues => exact absurd rfl hr
    case last => exact absurd rfl hlast
    all_goals
      simp only at hv
      rcases bind_err hv with hv | ⟨answers, _, hv⟩
      · exact Or.inr ⟨hneg, fun hwl => (lamBody IH hG1 hsl helem (totOK_of_eval hcv) hwl (w := .map) (dep := dep)).2 k l hv hk⟩
      · exfalso
        try simp only [Bool.true_and, beq_self_eq_true] at hv
        first
          | exact sized_not_failed hv
          | (split at hv
             · rw [fail_type hv] at hk; cases hk
             · first | exact sized_not_failed hv | (split at hv <;> first | cases hv | exact sized_not_failed hv))
          | (split at hv <;> first | cases hv | exact sized_not_failed hv)
  | _ =>
    exfalso
    cases w <;> simp only at hv <;> first
      | exact absurd rfl hl | exact absurd rfl hr
      | (rw [fail_type hv] at hk; cases hk)

/-- What is left of a walk of a list once the function has answered for each element. -/
theorem tail_ok {w : WalkK} {line : Nat} {key : Key} {xs answers : List Val} {v : Val}
    (h : (if (w == .flatMap && !answers.all (fun a => (listOf a).isSome)) = true then failK .type line
        else if (w == .groupBy && decide (i64max < (xs.length : Int))) = true then .error .diverged
        else sized (foldK w key xs answers)) = .ok v) :
    (w = .flatMap → answers.all (fun a => (listOf a).isSome) = true) ∧ (w = .groupBy → (xs.length : Int) ≤ i64max) ∧
      sized (foldK w key xs answers) = .ok v := by
  split at h
  · exact absurd h failK_ne_ok
  · rename_i h1
    split at h
    · cases h
    · rename_i h2
      refine ⟨fun hw => ?_, fun hw => ?_, h⟩
      · subst hw
        cases hall : answers.all (fun a => (listOf a).isSome)
        · exfalso; apply h1; rw [hall]; rfl
        · rfl
      · subst hw; simp at h2; omega

theorem walkK_never_branch (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {w : WalkK} {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key}
    (hG : Good C res A) (hS : Sat A ρ) (hlw : w ≠ .filterD ∧ w ≠ .mapValues ∧ w ≠ .last)
    (hwr : (ev A m coll o).out.widened = false)
    (hneg : ((walked (ev A m coll o).abs).1.hi.any fun h => h.num < 0) = true) :
    Concl C ρ (eval C (n + 1) dep ρ (.walkK id line w coll x body c key))
      (AR.of .never (ev A m coll o).out (ev A m coll o).ctx) := by
  refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
  · obtain ⟨_, _, _, hneg', _⟩ := walkK_list_ok IH hG hS hlw hwr hv
    rw [hneg] at hneg'; cases hneg'
  · rcases walkK_list_err IH hG hS hlw hwr hv hk with h | ⟨hneg', _⟩
    · exact h
    · rw [hneg] at hneg'; cases hneg'

theorem walkK_lam_branch (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {w : WalkK} {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key} {R : AR}
    (hG : Good C res A) (hS : Sat A ρ) (hlw : w ≠ .filterD ∧ w ≠ .mapValues ∧ w ≠ .last)
    (hRo : R.out = (lam1 (ev A m coll o).ctx (totalsOf coll) m x (walked (ev A m coll o).abs).2 body c (ev A m coll o).out).2)
    (hRc : R.ctx = (ev A m coll o).ctx) (hRn : R.nar = .none) (hw : R.out.widened = false)
    (hF : ∀ xs answers v, (walked (ev A m coll o).abs).1.In (xs.length : Rat) →
      (∀ y ∈ xs, Mem (walked (ev A m coll o).abs).2 y) → answers.length = xs.length →
      (∀ a ∈ answers, Mem (lam1 (ev A m coll o).ctx (totalsOf coll) m x (walked (ev A m coll o).abs).2 body c (ev A m coll o).out).1 a) →
      (w = .flatMap → answers.all (fun a => (listOf a).isSome) = true) → (w = .groupBy → (xs.length : Int) ≤ i64max) →
      sized (foldK w key xs answers) = .ok v → Mem R.abs v) :
    Concl C ρ (eval C (n + 1) dep ρ (.walkK id line w coll x body c key)) R := by
  rw [hRo] at hw
  have hwr := grows_not_widened (gLam1 (Grows.refl _)) hw
  refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
  · obtain ⟨xs, answers, hsl, _, hcnt, helem, hlen, hans, htail⟩ := walkK_list_ok IH hG hS hlw hwr hv
    obtain ⟨t1, t2, t3⟩ := tail_ok htail
    refine ⟨hF xs answers v hcnt helem hlen (hans hw) t1 t2 t3, by rw [hRc]; exact hsl, fun t _ => by rw [hRn]; trivial⟩
  · rw [hRo]
    rcases walkK_list_err IH hG hS hlw hwr hv hk with h | ⟨_, h⟩
    · exact Wit.grow h (gLam1 (Grows.refl _))
    · exact h hw

theorem sorted_mem {count : Iv} {elem : Abs} {xs answers : List Val} {f : Val × Val → Val × Val → Bool}
    (hcnt : count.In (xs.length : Rat)) (helem : ∀ y ∈ xs, Mem elem y) (hlen : answers.length = xs.length) :
    Mem (.list count (if noneAtAll count elem = true then .never else elem))
      (.list (((answers.zip xs).mergeSort f).map (·.2))) := by
  have hp := List.mergeSort_perm (answers.zip xs) f
  have hm : (((answers.zip xs).mergeSort f).map (·.2)).Perm xs := by
    have := hp.map (·.2)
    rw [List.map_snd_zip (by omega)] at this
    exact this
  refine Mem.list ?_ (fun y hy => ?_)
  · rw [hm.length_eq]; exact hcnt
  · have hy' := hm.mem_iff.mp hy
    split
    · rename_i h; have := nil_of_noneAtAll hcnt helem h; subst this; simp at hy'
    · exact helem y hy'

theorem best_mem {count : Iv} {elem : Abs} {xs answers : List Val} {key : Key} {want : Ordering}
    (hcnt : count.In (xs.length : Rat)) (helem : ∀ y ∈ xs, Mem elem y) (hlen : answers.length = xs.length) :
    Mem (optOf (count.lo.all fun l => l.num ≤ 0) (if noneAtAll count elem = true then .never else elem))
      (((bestBy key want none (answers.zip xs)).map (·.2)).getD .none) := by
  cases hb : bestBy key want none (answers.zip xs) with
  | none =>
    have hz := bestBy_none _ hb
    have hxs : xs = [] := by
      cases xs with
      | nil => rfl
      | cons y ys =>
        cases answers with
        | nil => simp at hlen
        | cons a as => simp at hz
    subst hxs
    exact Mem_optOf_none (mayBeEmpty_nil hcnt)
  | some p =>
    simp only [Option.map_some, Option.getD_some]
    rcases bestBy_mem none _ p hb with h | h
    · cases h
    · have hy := (List.of_mem_zip h).2
      refine Mem_optOf ?_
      split
      · rename_i hn; have := nil_of_noneAtAll hcnt helem hn; subst this; simp at hy
      · exact helem _ hy

theorem kept_mem {v : Abs} {a : Val} (h : Mem v a) (hn : a ≠ .none) :
    Mem (match (generalizing := false) v with | .opt _ y => y | y => y) a := by
  cases v with
  | opt b y =>
    cases h with
    | optNone => exact absurd rfl hn
    | optSome _ hy => exact hy
  | _ => exact h

theorem nil_of_zip_nil {xs answers : List Val} (hlen : answers.length = xs.length) (h : xs = []) : answers = [] := by
  subst h; exact List.length_eq_zero_iff.mp hlen

theorem groupBy_mem {count : Iv} {elem kv : Abs} {xs answers : List Val} {key : Key}
    (hcnt : count.In (xs.length : Rat)) (helem : ∀ y ∈ xs, Mem elem y) (hlen : answers.length = xs.length)
    (hans : ∀ a ∈ answers, Mem kv a) (hxs : (xs.length : Int) ≤ i64max)
    (hsz : ((groupsOf key (answers.zip xs)).length : Int) ≤ i64max) :
    Mem (.dict (oneUp count) kv (.list (countIv (some 1) count.hi) elem)) (.dict (groupsOf key (answers.zip xs))) := by
  have hfst : (answers.zip xs).map (·.1) = answers := List.map_fst_zip (by omega)
  refine Mem.dict ?_ (fun g hg => ?_) (fun g hg => ?_) (groupsOf_nodup key _)
  · rw [groupsOf_length, hfst]
    refine oneUp_sound hcnt (by rw [← hlen]; exact firstOfEach_length_le answers) (fun hpos => ?_) (by
      rw [groupsOf_length, hfst] at hsz; exact hsz)
    have hne : answers ≠ [] := by
      intro h; rw [h] at hlen; simp at hlen; omega
    exact List.length_pos_iff.mpr (firstOfEach_ne_nil hne)
  · obtain ⟨hk, _⟩ := groupsOf_mem hg
    rw [hfst] at hk
    exact hans _ hk
  · obtain ⟨hk, hv⟩ := groupsOf_mem hg
    rw [hv]
    obtain ⟨p, hp, hpk⟩ := List.mem_map.mp hk
    have hpf : p ∈ (answers.zip xs).filter fun q => sameKey q.1 g.1 := by
      refine List.mem_filter.mpr ⟨hp, ?_⟩
      rw [hpk]; exact sameKey_iff.mpr rfl
    have hlen1 : 1 ≤ (((answers.zip xs).filter fun q => sameKey q.1 g.1).map (·.2)).length := by
      rw [List.length_map]; exact List.length_pos_iff.mpr (List.ne_nil_of_mem hpf)
    have hlen2 : (((answers.zip xs).filter fun q => sameKey q.1 g.1).map (·.2)).length ≤ xs.length := by
      rw [List.length_map]
      exact Nat.le_trans (List.length_filter_le _ _) (by simp only [List.length_zip]; exact Nat.min_le_right _ _)
    refine Mem.list (countIv_sound ?_ (fun h hh => Or.inl ?_) ?_) (fun y hy => ?_)
    · simp only [EndLe]; exact_mod_cast hlen1
    · have := hcnt.hi; rw [hh] at this; simp only [EndGe] at this
      exact Rat.le_trans (by exact_mod_cast hlen2) this
    · omega
    · obtain ⟨q, hq, rfl⟩ := List.mem_map.mp hy
      exact helem _ (List.of_mem_zip (List.mem_filter.mp hq).1).2

theorem lists_of_all : ∀ {as : List Val}, as.all (fun a => (listOf a).isSome) = true → as = (as.filterMap listOf).map .list
  | [], _ => rfl
  | a :: rest, h => by
    simp only [List.all_cons, Bool.and_eq_true] at h
    obtain ⟨h1, h2⟩ := h
    have ih := lists_of_all h2
    cases a with
    | list ys =>
      simp only [List.filterMap_cons, listOf, List.map_cons]
      exact congrArg _ ih
    | _ => simp [listOf] at h1

theorem flat_mem {count : Iv} {xs answers : List Val} {lv : Abs}
    (hcnt : count.In (xs.length : Rat)) (hlen : answers.length = xs.length)
    (hans : ∀ a ∈ answers, Mem lv a) (hall : answers.all (fun a => (listOf a).isSome) = true)
    (hsz : (((answers.filterMap listOf).flatten).length : Int) ≤ i64max) :
    Mem (.list (timesCount count (match (generalizing := false) lv with | .list ci ei => (ci, ei) | _ => (anyCount, Abs.any)).1)
        (match (generalizing := false) lv with | .list ci ei => (ci, ei) | _ => (anyCount, Abs.any)).2)
      (.list ((answers.filterMap listOf).flatten)) := by
  have heq := lists_of_all hall
  have hin : ∀ ys ∈ answers.filterMap listOf,
      (match (generalizing := false) lv with | .list ci ei => (ci, ei) | _ => (anyCount, Abs.any)).1.In (ys.length : Rat) ∧
      ∀ y ∈ ys, Mem (match (generalizing := false) lv with | .list ci ei => (ci, ei) | _ => (anyCount, Abs.any)).2 y := by
    intro ys hys
    have hm : Mem lv (.list ys) := hans _ (by rw [heq]; exact List.mem_map_of_mem hys)
    cases lv with
    | list ci ei =>
      cases hm with
      | list hc he => exact ⟨hc, he⟩
    | _ => exact ⟨anyCount_In _, fun y _ => Mem.any y⟩
  have hlen' : (answers.filterMap listOf).length = xs.length := by
    have := congrArg List.length heq; simp only [List.length_map] at this; omega
  refine Mem.list (timesCount_sound (by rw [hlen']; exact hcnt) (fun ys hys => (hin ys hys).1) hsz) (fun y hy => ?_)
  obtain ⟨ys, hys, hy⟩ := List.mem_flatten.mp hy
  exact (hin ys hys).2 y hy

theorem filterMap_mem {count : Iv} {elem lv : Abs} {xs answers : List Val}
    (hcnt : count.In (xs.length : Rat)) (helem : ∀ y ∈ xs, Mem elem y) (hlen : answers.length = xs.length)
    (hans : ∀ a ∈ answers, Mem lv a) :
    Mem (.list (upTo count) (if noneAtAll count elem = true then .never
      else (match (generalizing := false) lv with | .opt _ y => y | y => y)))
      (.list (answers.filter fun a => match a with | .none => false | _ => true)) := by
  refine Mem.list ?_ (fun a ha => ?_)
  · unfold upTo
    refine In_mk' (by simp only [EndLe]; exact_mod_cast Nat.zero_le _) ?_ (by simp [DenLe])
    have eh := hcnt.hi
    cases hh : count.hi with
    | none => trivial
    | some u =>
      rw [hh] at eh; simp only [EndGe] at eh ⊢
      have hle : (answers.filter fun a => match a with | .none => false | _ => true).length ≤ xs.length := by
        rw [← hlen]; exact List.length_filter_le _ _
      have : (((answers.filter fun a => match a with | .none => false | _ => true).length : Nat) : Rat) ≤ (xs.length : Rat) := by
        exact_mod_cast hle
      exact Rat.le_trans this eh
  · obtain ⟨hin, hp⟩ := List.mem_filter.mp ha
    have hn : a ≠ .none := by intro h; subst h; simp at hp
    split
    · rename_i h
      have hx := nil_of_noneAtAll hcnt helem h
      have := nil_of_zip_nil hlen hx; subst this; simp at hin
    · exact kept_mem (hans a hin) hn

theorem upTo_sound {c : Iv} {n L : Nat} (hc : c.In (n : Rat)) (hL : L ≤ n) : (upTo c).In (L : Rat) := by
  unfold upTo
  refine In_mk' (by simp only [EndLe]; exact_mod_cast Nat.zero_le _) ?_ (by simp [DenLe])
  have eh := hc.hi
  cases hh : c.hi with
  | none => trivial
  | some u =>
    rw [hh] at eh; simp only [EndGe] at eh ⊢
    exact Rat.le_trans (by exact_mod_cast hL) eh

theorem groupsOf_nil (k : Key) : groupsOf k [] = [] := by
  simp [groupsOf, firstOfEach, firstOfEachAux]

/-- **`last`**: the element nearest the end where the condition holds, or none. -/
theorem walkK_last (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A (m + 1) (.walkK id line .last coll x body c key) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.walkK id line .last coll x body c key))
      (evI A (m + 1) (.walkK id line .last coll x body c key) o) := by
  simp only [evI] at hw ⊢
  have hG1 : Good C res (ev A m coll o).ctx := hG.same ((sat m).ev A coll o).2
  split
  · rename_i hneg
    rw [ite_eq_left hneg] at hw
    simp only [AR.of] at hw ⊢
    obtain ⟨hl1, hl2⟩ := IH.expr m A ρ coll o dep hG hS hw
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨cv, hcv, hv⟩ := bind_ok hv
      obtain ⟨hml, _, _⟩ := hl1 cv hcv
      cases cv with
      | list xs => rw [walked_not_neg hml] at hneg; cases hneg
      | _ => simp only at hv; exact absurd hv failK_ne_ok
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨cv, hcv, hv⟩
      · exact hl2 k l hv hk
      · obtain ⟨hml, _, _⟩ := hl1 cv hcv
        cases cv with
        | list xs => rw [walked_not_neg hml] at hneg; cases hneg
        | _ => simp only at hv; rw [fail_type hv] at hk; cases hk
  · rename_i hneg
    rw [ite_eq_right hneg] at hw
    simp only [AR.of] at hw ⊢
    have gc : Grows (ev A m coll o).out
        (condition (ev A m coll o).ctx (totalsOf coll) m x (walked (ev A m coll o).abs).2 body c (ev A m coll o).out).2 :=
      gCond (Grows.refl _)
    obtain ⟨hl1, hl2⟩ := IH.expr m A ρ coll o dep hG hS (grows_not_widened gc hw)
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨cv, hcv, hv⟩ := bind_ok hv
      obtain ⟨hml, hsl, _⟩ := hl1 cv hcv
      cases cv with
      | list xs =>
        simp only at hv
        obtain ⟨answers, has, hv⟩ := bind_ok hv
        obtain ⟨_, _, hv⟩ := bind_ok hv
        cases hv
        obtain ⟨hcnt, helem⟩ := walked_sound hml
        have helem' : ∀ y ∈ xs.reverse, Mem (walked (ev A m coll o).abs).2 y :=
          fun y hy => helem y (List.mem_reverse.mp hy)
        obtain ⟨hb1, _⟩ := condBody IH hG1 hsl helem' ((totOK_of_eval hcv).sub fun y hy => List.mem_reverse.mp hy) hw (w := .first) (dep := dep)
        obtain ⟨_, _, hheld⟩ := hb1 answers has
        refine ⟨?_, hsl, fun _ _ => trivial⟩
        simp only [foldK]
        cases hf : (xs.reverse.zip answers).find? (fun p => isTrue p.2) with
        | none => exact Mem.optNone
        | some p =>
          simp only [Option.map_some, Option.getD_some]
          have hp := List.mem_of_find?_eq_some hf
          have ht0 := List.find?_some hf
          have ht := isTrue_iff.mp (by simpa using ht0)
          have hm := hheld p.1 p.2 hp ht
          by_cases hn : p.1 = .none
          · rw [hn]; exact Mem.optNone
          · exact Mem.optSome hn (Mem_stripOpt hm hn)
      | _ => simp only at hv; exact absurd hv failK_ne_ok
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨cv, hcv, hv⟩
      · exact Wit.grow (hl2 k l hv hk) gc
      · obtain ⟨hml, hsl, _⟩ := hl1 cv hcv
        cases cv with
        | list xs =>
          simp only at hv
          obtain ⟨hcnt, helem⟩ := walked_sound hml
          have helem' : ∀ y ∈ xs.reverse, Mem (walked (ev A m coll o).abs).2 y :=
            fun y hy => helem y (List.mem_reverse.mp hy)
          rcases bind_err hv with hv | ⟨answers, _, hv⟩
          · exact (condBody IH hG1 hsl helem' ((totOK_of_eval hcv).sub fun y hy => List.mem_reverse.mp hy) hw (w := .first) (dep := dep)).2 k l hv hk
          · rcases bind_err hv with hv | ⟨_, _, hv⟩
            · have := answersOK_err hv
              simp only [Stop.failed.injEq] at this
              rw [this.1] at hk; cases hk
            · cases hv
        | _ => simp only at hv; rw [fail_type hv] at hk; cases hk

/-- What a dict's analysis says of its count, its keys and its values, where it is a dict the
    analysis follows, or anything. -/
theorem dparts_sound {d : Abs} {ps : List (Val × Val)} (h : Mem d (.dict ps)) :
    (match (generalizing := false) d with | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).1.In (ps.length : Rat) ∧
    (∀ p ∈ ps, Mem (match (generalizing := false) d with | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.1 p.1) ∧
    (∀ p ∈ ps, Mem (match (generalizing := false) d with | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.2 p.2) ∧
    ((ps.map (·.1)).Nodup ∨ ∀ L : Nat,
      (match (generalizing := false) d with | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).1.In (L : Rat)) := by
  cases h with
  | any => exact ⟨anyCount_In _, fun p _ => Mem.any _, fun p _ => Mem.any _, Or.inr anyCount_In⟩
  | optSome _ _ => exact ⟨anyCount_In _, fun p _ => Mem.any _, fun p _ => Mem.any _, Or.inr anyCount_In⟩
  | dict hc hk hv hn => exact ⟨hc, hk, hv, Or.inl hn⟩

theorem zip_entry {ps : List (Val × Val)} {answers : List Val} {p : Val × Val} {a : Val}
    (h : (p, a) ∈ ps.zip answers) : (entryV p, a) ∈ (ps.map entryV).zip answers := by
  rw [List.zip_map_left]
  exact List.mem_map_of_mem (f := Prod.map entryV id) h

theorem entry_field_key (p : Val × Val) : (entryV p).field? "key" = some p.1 := rfl
theorem entry_field_value (p : Val × Val) : (entryV p).field? "value" = some p.2 := rfl

/-- **`d.filter(e => …)`**: the pairs whose entries hold the condition, each where the condition held. -/
theorem walkK_filterD (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A (m + 1) (.walkK id line .filterD coll x body c key) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.walkK id line .filterD coll x body c key))
      (evI A (m + 1) (.walkK id line .filterD coll x body c key) o) := by
  simp only [evI] at hw ⊢
  have hG1 : Good C res (ev A m coll o).ctx := hG.same ((sat m).ev A coll o).2
  split
  · rename_i hnev
    rw [hnev] at hw
    simp only [AR.of] at hw ⊢
    obtain ⟨hl1, hl2⟩ := IH.expr m A ρ coll o dep hG hS hw
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨cv, hcv, _⟩ := bind_ok hv
      have := (hl1 cv hcv).1; rw [hnev] at this; exact absurd this Mem.not_never
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨cv, hcv, _⟩
      · exact hl2 k l hv hk
      · have := (hl1 cv hcv).1; rw [hnev] at this; exact absurd this Mem.not_never
  · rename_i hnev
    simp only [AR.of] at hw ⊢
    have hwr := grows_not_widened (gCond (Grows.refl _)) hw
    obtain ⟨hl1, hl2⟩ := IH.expr m A ρ coll o dep hG hS hwr
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨cv, hcv, hv⟩ := bind_ok hv
      obtain ⟨hml, hsl, _⟩ := hl1 cv hcv
      cases cv with
      | dict ps =>
        simp only at hv
        obtain ⟨answers, has, hv⟩ := bind_ok hv
        obtain ⟨_, _, hv⟩ := bind_ok hv
        obtain rfl := sized_eq hv
        obtain ⟨hc, hk, hvv, _⟩ := dparts_sound hml
        have helem : ∀ y ∈ ps.map entryV, Mem (entryAbs
            (match (generalizing := false) (ev A m coll o).abs with
              | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.1
            (match (generalizing := false) (ev A m coll o).abs with
              | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.2) y := by
          intro y hy; obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hy; exact Mem_entry (hk p hp) (hvv p hp)
        obtain ⟨hb1, _⟩ := condBody IH hG1 hsl helem (TotOK.none ρ _) hw (w := .map) (dep := dep)
        obtain ⟨_, _, hheld⟩ := hb1 answers has
        refine ⟨Mem.dict ?_ (fun q hq => ?_) (fun q hq => ?_) (dictFrom_nodup key _), hsl, fun _ _ => trivial⟩
        · refine upTo_sound hc (Nat.le_trans (dictFrom_length_le key _) ?_)
          rw [List.length_map]
          exact Nat.le_trans (List.length_filter_le _ _) (by simp only [List.length_zip]; exact Nat.min_le_left _ _)
        · obtain ⟨pa, hpa, rfl⟩ := List.mem_map.mp (dictFrom_mem hq)
          obtain ⟨hz, ht⟩ := List.mem_filter.mp hpa
          exact Mem_field (hheld (entryV pa.1) pa.2 (zip_entry hz) (isTrue_iff.mp ht)) (entry_field_key pa.1)
        · obtain ⟨pa, hpa, rfl⟩ := List.mem_map.mp (dictFrom_mem hq)
          obtain ⟨hz, ht⟩ := List.mem_filter.mp hpa
          exact Mem_field (hheld (entryV pa.1) pa.2 (zip_entry hz) (isTrue_iff.mp ht)) (entry_field_value pa.1)
      | _ => simp only at hv; exact absurd hv failK_ne_ok
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨cv, hcv, hv⟩
      · exact Wit.grow (hl2 k l hv hk) (gCond (Grows.refl _))
      · obtain ⟨hml, hsl, _⟩ := hl1 cv hcv
        cases cv with
        | dict ps =>
          simp only at hv
          obtain ⟨_, hk', hvv, _⟩ := dparts_sound hml
          have helem : ∀ y ∈ ps.map entryV, Mem (entryAbs
              (match (generalizing := false) (ev A m coll o).abs with
                | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.1
              (match (generalizing := false) (ev A m coll o).abs with
                | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.2) y := by
            intro y hy; obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hy; exact Mem_entry (hk' p hp) (hvv p hp)
          rcases bind_err hv with hv | ⟨answers, _, hv⟩
          · exact (condBody IH hG1 hsl helem (TotOK.none ρ _) hw (w := .map) (dep := dep)).2 k l hv hk
          · rcases bind_err hv with hv | ⟨_, _, hv⟩
            · have := answersOK_err hv
              simp only [Stop.failed.injEq] at this
              rw [this.1] at hk; cases hk
            · exact absurd hv (fun h => sized_not_failed h)
        | _ => simp only at hv; rw [fail_type hv] at hk; cases hk

theorem mem_ite_never {c : Prop} [Decidable c] {a : Abs} {v : Val} (h : c → False) (hv : Mem a v) :
    Mem (if c then .never else a) v := by
  rw [ite_eq_right h]; exact hv

theorem map_keys_zip {ps : List (Val × Val)} {answers : List Val} (hlen : answers.length = ps.length) :
    ((ps.zip answers).map fun pu => (pu.1.1, pu.2)).map (·.1) = ps.map (·.1) := by
  rw [List.map_map]
  show (ps.zip answers).map (fun pu => pu.1.1) = ps.map (·.1)
  rw [show (fun pu : (Val × Val) × Val => pu.1.1) = (fun p : Val × Val => p.1) ∘ (fun pu => pu.1) from rfl,
    ← List.map_map, List.map_fst_zip (by omega)]

/-- **`d.map_values(v => …)`**: the same keys, each value what the function answers for it. -/
theorem walkK_mapValues (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A (m + 1) (.walkK id line .mapValues coll x body c key) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.walkK id line .mapValues coll x body c key))
      (evI A (m + 1) (.walkK id line .mapValues coll x body c key) o) := by
  simp only [evI] at hw ⊢
  have hG1 : Good C res (ev A m coll o).ctx := hG.same ((sat m).ev A coll o).2
  split
  · rename_i hnev
    rw [hnev] at hw
    simp only [AR.of] at hw ⊢
    obtain ⟨hl1, hl2⟩ := IH.expr m A ρ coll o dep hG hS hw
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨cv, hcv, _⟩ := bind_ok hv
      have := (hl1 cv hcv).1; rw [hnev] at this; exact absurd this Mem.not_never
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨cv, hcv, _⟩
      · exact hl2 k l hv hk
      · have := (hl1 cv hcv).1; rw [hnev] at this; exact absurd this Mem.not_never
  · rename_i hnev
    simp only [AR.of] at hw ⊢
    have hwr := grows_not_widened (gLam1 (Grows.refl _)) hw
    obtain ⟨hl1, hl2⟩ := IH.expr m A ρ coll o dep hG hS hwr
    refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
    · simp only [eval] at hv
      obtain ⟨cv, hcv, hv⟩ := bind_ok hv
      obtain ⟨hml, hsl, _⟩ := hl1 cv hcv
      cases cv with
      | dict ps =>
        simp only at hv
        obtain ⟨answers, has, hv⟩ := bind_ok hv
        obtain rfl := sized_eq hv
        obtain ⟨hc, hk, hvv, hnd⟩ := dparts_sound hml
        have helem : ∀ y ∈ ps.map (·.2), Mem
            (match (generalizing := false) (ev A m coll o).abs with
              | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.2 y := by
          intro y hy; obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hy; exact hvv p hp
        obtain ⟨hb1, _⟩ := lamBody IH hG1 hsl helem (TotOK.none ρ _) hw (w := .map) (dep := dep)
        have hans := hb1 answers has
        have hlen : answers.length = ps.length := by
          have := (evalEach_shape n dep ρ x body c .map _ answers has).2.1 (fun a _ => stopAt_map a)
          simpa using this
        refine ⟨?_, hsl, fun _ _ => trivial⟩
        refine Mem.dict ?_ (fun q hq => ?_) (fun q hq => ?_) (dictFrom_nodup key _)
        · rcases hnd with hnd | hany
          · rw [dictFrom_length_eq (by rw [map_keys_zip hlen]; exact hnd)]
            simp only [List.length_map, List.length_zip]
            rw [hlen, Nat.min_self]; exact hc
          · exact hany _
        · obtain ⟨pu, hpu, rfl⟩ := List.mem_map.mp (dictFrom_mem hq)
          exact hk pu.1 (List.of_mem_zip hpu).1
        · obtain ⟨pu, hpu, rfl⟩ := List.mem_map.mp (dictFrom_mem hq)
          have hu := hans pu.2 (List.of_mem_zip hpu).2
          have hp1 := (List.of_mem_zip hpu).1
          refine mem_ite_never (fun hnone => ?_) hu
          have hc' : (match (generalizing := false) (ev A m coll o).abs with
              | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).1.In ((ps.map Prod.snd).length : Rat) := by
            simpa using hc
          have := nil_of_noneAtAll hc' helem hnone
          have hps : ps = [] := by simpa using this
          subst hps; simp at hp1
      | _ => simp only at hv; exact absurd hv failK_ne_ok
    · simp only [eval] at hv
      rcases bind_err hv with hv | ⟨cv, hcv, hv⟩
      · exact Wit.grow (hl2 k l hv hk) (gLam1 (Grows.refl _))
      · obtain ⟨hml, hsl, _⟩ := hl1 cv hcv
        cases cv with
        | dict ps =>
          simp only at hv
          obtain ⟨_, _, hvv, _⟩ := dparts_sound hml
          have helem : ∀ y ∈ ps.map (·.2), Mem
              (match (generalizing := false) (ev A m coll o).abs with
                | .dict cnt k v => (cnt, k, v) | _ => (anyCount, Abs.any, Abs.any)).2.2 y := by
            intro y hy; obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hy; exact hvv p hp
          rcases bind_err hv with hv | ⟨answers, _, hv⟩
          · exact (lamBody IH hG1 hsl helem (TotOK.none ρ _) hw (w := .map) (dep := dep)).2 k l hv hk
          · exact absurd hv (fun h => sized_not_failed h)
        | _ => simp only at hv; rw [fail_type hv] at hk; cases hk

theorem mem_ite_ar {c : Prop} [Decidable c] {X Y : AR} {v : Val} (h1 : c → Mem X.abs v) (h2 : ¬c → Mem Y.abs v) :
    Mem (if c then X else Y).abs v := by
  split
  · exact h1 ‹_›
  · exact h2 ‹_›

theorem dict_nil_mem {key : Key} : Mem (.dict (Iv.ints 0 0) .never .never) (.dict (dictFrom key [])) := by
  have h0 : dictFrom key [] = [] := by simp [dictFrom, lastOfEach]
  rw [h0]
  exact Mem.dict (In_ints (Int.le_refl 0) (Int.le_refl 0)) (fun q hq => by simp at hq) (fun q hq => by simp at hq)
    List.nodup_nil

/-- **A builtin that applies a function to each element of a list or to each entry or value of a dict.** -/
theorem case_walkK (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {w : WalkK}
    {coll : Expr} {x : String} {body : Expr} {c : Option Conv} {key : Key} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.walkK id line w coll x body c key) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.walkK id line w coll x body c key))
      (evI A m (.walkK id line w coll x body c key) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    cases w
    case filterD => exact walkK_filterD IH hG hS hw
    case mapValues => exact walkK_mapValues IH hG hS hw
    case last => exact walkK_last IH hG hS hw
    case sortBy | sortByDesc =>
      simp only [evI] at hw ⊢
      split
      · rename_i hneg; rw [ite_eq_left hneg] at hw
        exact walkK_never_branch IH hG hS (by decide) hw hneg
      · rename_i hneg; rw [ite_eq_right hneg] at hw
        refine walkK_lam_branch IH hG hS (by decide) rfl rfl rfl hw ?_
        intro xs answers v hcnt helem hlen _ _ _ hsz
        obtain rfl := sized_eq hsz
        exact sorted_mem hcnt helem hlen
    case maxBy | minBy =>
      simp only [evI] at hw ⊢
      split
      · rename_i hneg; rw [ite_eq_left hneg] at hw
        exact walkK_never_branch IH hG hS (by decide) hw hneg
      · rename_i hneg; rw [ite_eq_right hneg] at hw
        refine walkK_lam_branch IH hG hS (by decide) rfl rfl rfl hw ?_
        intro xs answers v hcnt helem hlen _ _ _ hsz
        obtain rfl := sized_eq hsz
        exact best_mem hcnt helem hlen
    case filterMap =>
      simp only [evI] at hw ⊢
      split
      · rename_i hneg; rw [ite_eq_left hneg] at hw
        exact walkK_never_branch IH hG hS (by decide) hw hneg
      · rename_i hneg; rw [ite_eq_right hneg] at hw
        refine walkK_lam_branch IH hG hS (by decide) rfl rfl rfl hw ?_
        intro xs answers v hcnt helem hlen hans _ _ hsz
        obtain rfl := sized_eq hsz
        exact filterMap_mem hcnt helem hlen hans
    case flatMap =>
      simp only [evI] at hw ⊢
      split
      · rename_i hneg; rw [ite_eq_left hneg] at hw
        exact walkK_never_branch IH hG hS (by decide) hw hneg
      · rename_i hneg; rw [ite_eq_right hneg] at hw
        refine walkK_lam_branch IH hG hS (by decide) (by split <;> rfl) (by split <;> rfl) (by split <;> rfl) hw ?_
        intro xs answers v hcnt helem hlen hans h1 _ hsz
        have hall := h1 rfl
        have hsz' := sized_list (xs := (answers.filterMap listOf).flatten) hsz
        obtain rfl := sized_eq hsz
        refine mem_ite_ar (fun hn => ?_) (fun _ => ?_)
        · have hx := nil_of_noneAtAll hcnt helem hn
          have ha := nil_of_zip_nil hlen hx
          subst ha
          exact Mem.list (In_ints (Int.le_refl 0) (Int.le_refl 0)) (fun y hy => by simp at hy)
        · exact flat_mem hcnt hlen hans hall hsz'
    case groupBy =>
      simp only [evI] at hw ⊢
      split
      · rename_i hneg; rw [ite_eq_left hneg] at hw
        exact walkK_never_branch IH hG hS (by decide) hw hneg
      · rename_i hneg; rw [ite_eq_right hneg] at hw
        refine walkK_lam_branch IH hG hS (by decide) (by split <;> rfl) (by split <;> rfl) (by split <;> rfl) hw ?_
        intro xs answers v hcnt helem hlen hans _ h2 hsz
        have hxs := h2 rfl
        have hsz' := sized_dict (ps := groupsOf key (answers.zip xs)) hsz
        obtain rfl := sized_eq hsz
        refine mem_ite_ar (fun hn => ?_) (fun hn => ?_)
        · have hx := nil_of_noneAtAll hcnt helem hn
          have ha := nil_of_zip_nil hlen hx
          subst hx; subst ha
          show Mem _ (.dict (groupsOf key ([].zip [])))
          simp only [List.zip_nil_left, groupsOf_nil]
          exact Mem.dict (In_ints (Int.le_refl 0) (Int.le_refl 0)) (fun q hq => by simp at hq)
            (fun q hq => by simp at hq) List.nodup_nil
        · exact groupBy_mem hcnt helem hlen hans hxs hsz'

/-- **`xs.to_dict(x => key, y => value)`**: a pair for each element, the later of two of one key kept. -/
theorem case_toDict (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {key : Key}
    {list : Expr} {x : String} {kb : Expr} {kc : Option Conv} {y : String} {vb : Expr} {vc : Option Conv}
    (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.toDict id line key list x kb kc y vb vc) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.toDict id line key list x kb kc y vb vc))
      (evI A m (.toDict id line key list x kb kc y vb vc) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    have hG1 : Good C res (ev A m list o).ctx := hG.same ((sat m).ev A list o).2
    split
    · rename_i hneg; rw [ite_eq_left hneg] at hw
      simp only [AR.of] at hw ⊢
      obtain ⟨hl1, hl2⟩ := IH.expr m A ρ list o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨lv, hlv, hv⟩ := bind_ok hv
        obtain ⟨hml, _, _⟩ := hl1 lv hlv
        cases lv with
        | list xs => rw [walked_not_neg hml] at hneg; cases hneg
        | _ => simp only at hv; exact absurd hv failK_ne_ok
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨lv, hlv, hv⟩
        · exact hl2 k l hv hk
        · obtain ⟨hml, _, _⟩ := hl1 lv hlv
          cases lv with
          | list xs => rw [walked_not_neg hml] at hneg; cases hneg
          | _ => simp only at hv; rw [fail_type hv] at hk; cases hk
    · rename_i hneg; rw [ite_eq_right hneg] at hw
      have hw2 : (lam1 (ev A m list o).ctx none m y (walked (ev A m list o).abs).2 vb vc
          (lam1 (ev A m list o).ctx none m x (walked (ev A m list o).abs).2 kb kc (ev A m list o).out).2).2.widened = false := by
        revert hw; split <;> exact fun h => h
      have gl : Grows (ev A m list o).out (lam1 (ev A m list o).ctx none m y (walked (ev A m list o).abs).2 vb vc
          (lam1 (ev A m list o).ctx none m x (walked (ev A m list o).abs).2 kb kc (ev A m list o).out).2).2 :=
        gLam1 (gLam1 (Grows.refl _))
      obtain ⟨hl1, hl2⟩ := IH.expr m A ρ list o dep hG hS (grows_not_widened gl hw2)
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨lv, hlv, hv⟩ := bind_ok hv
        obtain ⟨hml, hsl, _⟩ := hl1 lv hlv
        cases lv with
        | list xs =>
          simp only at hv
          obtain ⟨ps, hps, hv⟩ := bind_ok hv
          have hsz := sized_dict hv
          obtain rfl := sized_eq hv
          obtain ⟨hcnt, helem⟩ := walked_sound hml
          obtain ⟨hlen, hpm⟩ := (IH.each2 m _ ρ x kb kc y vb vc xs _ _ dep hG1 hsl helem hw2).1 ps hps
          refine ⟨?_, by split <;> exact hsl, fun _ _ => by split <;> trivial⟩
          refine mem_ite_ar (fun hn => ?_) (fun hn => ?_)
          · have hx := nil_of_noneAtAll hcnt helem hn
            subst hx
            have : ps = [] := List.length_eq_zero_iff.mp hlen
            subst this
            exact dict_nil_mem
          · refine dict_count_mem (oneUp_sound hcnt (by rw [← hlen]; exact dictFrom_length_le key ps) (fun hpos => ?_) hsz)
              (fun q hq => (hpm q hq).1) (fun q hq => (hpm q hq).2)
            have hne : ps ≠ [] := by intro h; rw [h] at hlen; simp at hlen; omega
            exact List.length_pos_iff.mpr (dictFrom_ne_nil hne)
        | _ => simp only at hv; exact absurd hv failK_ne_ok
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨lv, hlv, hv⟩
        · exact Wit.grow (hl2 k l hv hk) (by split <;> exact gl)
        · obtain ⟨hml, hsl, _⟩ := hl1 lv hlv
          cases lv with
          | list xs =>
            simp only at hv
            obtain ⟨_, helem⟩ := walked_sound hml
            rcases bind_err hv with hv | ⟨ps, _, hv⟩
            · have := (IH.each2 m _ ρ x kb kc y vb vc xs _ _ dep hG1 hsl helem hw2).2 k l hv hk
              revert this; split <;> exact fun h => h
            · exact absurd hv (fun h => sized_not_failed h)
          | _ => simp only at hv; rw [fail_type hv] at hk; cases hk

/-- **`d.merge_with(o, (a, b) => …)`**: the pairs of both, the function's answer where both have a key. -/
theorem case_mergeWith (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat} {key : Key}
    {a b : Expr} {x y : String} {body : Expr} {c : Option Conv} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.mergeWith id line key a b x y body c) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.mergeWith id line key a b x y body c))
      (evI A m (.mergeWith id line key a b x y body c) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    have hGa : Good C res (ev A m a o).ctx := hG.same ((sat m).ev A a o).2
    split
    · rename_i hnev
      rw [hnev] at hw
      simp only [AR.of] at hw ⊢
      obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS hw
      refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
      · simp only [eval] at hv
        obtain ⟨av, hav, _⟩ := bind_ok hv
        have := (h1 av hav).1; rw [hnev] at this; exact absurd this Mem.not_never
      · simp only [eval] at hv
        rcases bind_err hv with hv | ⟨av, hav, _⟩
        · exact h2 k l hv hk
        · have := (h1 av hav).1; rw [hnev] at this; exact absurd this Mem.not_never
    · split
      · rename_i hnev
        rw [hnev] at hw
        simp only [AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened (gEv (Grows.refl _)) hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨av, hav, hv⟩ := bind_ok hv
          obtain ⟨bv, hbv, _⟩ := bind_ok hv
          obtain ⟨_, hs, _⟩ := h1 av hav
          have := ((IH.expr m _ ρ b _ dep hGa hs hw).1 bv hbv).1
          rw [hnev] at this; exact absurd this Mem.not_never
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨av, hav, hv⟩
          · exact Wit.grow (h2 k l hv hk) (gEv (Grows.refl _))
          · obtain ⟨_, hs, _⟩ := h1 av hav
            obtain ⟨b1, b2⟩ := IH.expr m _ ρ b _ dep hGa hs hw
            rcases bind_err hv with hv | ⟨bv, hbv, _⟩
            · exact b2 k l hv hk
            · have := (b1 bv hbv).1; rw [hnev] at this; exact absurd this Mem.not_never
      · simp only [AR.of] at hw ⊢
        have gb : Grows (ev (ev A m a o).ctx m b (ev A m a o).out).out
            (lam2 (ev (ev A m a o).ctx m b (ev A m a o).out).ctx m x y (dictParts (ev A m a o).abs).2.2
              (dictParts (ev (ev A m a o).ctx m b (ev A m a o).out).abs).2.2 body c
              (ev (ev A m a o).ctx m b (ev A m a o).out).out).2 := gLam2 (Grows.refl _)
        have ga : Grows (ev A m a o).out
            (lam2 (ev (ev A m a o).ctx m b (ev A m a o).out).ctx m x y (dictParts (ev A m a o).abs).2.2
              (dictParts (ev (ev A m a o).ctx m b (ev A m a o).out).abs).2.2 body c
              (ev (ev A m a o).ctx m b (ev A m a o).out).out).2 := gLam2 (gEv (Grows.refl _))
        obtain ⟨h1, h2⟩ := IH.expr m A ρ a o dep hG hS (grows_not_widened ga hw)
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨av, hav, hv⟩ := bind_ok hv
          obtain ⟨bv, hbv, hv⟩ := bind_ok hv
          obtain ⟨hma, hs, _⟩ := h1 av hav
          obtain ⟨b1, _⟩ := IH.expr m _ ρ b _ dep hGa hs (grows_not_widened gb hw)
          obtain ⟨hmb, hsb, _⟩ := b1 bv hbv
          have hGb : Good C res (ev (ev A m a o).ctx m b (ev A m a o).out).ctx :=
            hGa.same ((sat m).ev _ b (ev A m a o).out).2
          cases av with
          | dict ps =>
            cases bv with
            | dict qs =>
              simp only at hv
              obtain ⟨more, hmore, hv⟩ := bind_ok hv
              have hsz := sized_dict hv
              obtain rfl := sized_eq hv
              obtain ⟨hc, hk, hvv, hnd⟩ := dictParts_sound hma
              obtain ⟨hc', hk', hvv', hnd'⟩ := dictParts_sound hmb
              obtain ⟨hkeys, hmv⟩ := (IH.merge m _ ρ key ps x y body c _ _ qs _ dep hGb hsb hvv hvv' hw).1 more hmore
              have hml : more.length = qs.length := by
                have := congrArg List.length hkeys; simpa using this
              refine ⟨dict_count_mem (mergedC_sound hc hnd (by rw [hml]; exact hc') (by rw [hkeys]; exact hnd') hsz)
                (fun q hq => ?_) (fun q hq => ?_), hsb, fun _ _ => trivial⟩
              · rcases List.mem_append.mp hq with hq | hq
                · exact mem_join_left (hk q hq)
                · have : q.1 ∈ qs.map (·.1) := by rw [← hkeys]; exact List.mem_map_of_mem hq
                  obtain ⟨q', hq', he⟩ := List.mem_map.mp this
                  rw [← he]; exact mem_join_right (hk' q' hq')
              · rcases List.mem_append.mp hq with hq | hq
                · exact mem_join_left (mem_join_left (hvv q hq))
                · rcases hmv q hq with h | h
                  · exact mem_join_left (mem_join_right h)
                  · exact mem_join_right h
            | _ => simp only at hv; exact absurd hv failK_ne_ok
          | _ => simp only at hv; exact absurd hv failK_ne_ok
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨av, hav, hv⟩
          · exact Wit.grow (h2 k l hv hk) ga
          · obtain ⟨hma, hs, _⟩ := h1 av hav
            obtain ⟨b1, b2⟩ := IH.expr m _ ρ b _ dep hGa hs (grows_not_widened gb hw)
            rcases bind_err hv with hv | ⟨bv, hbv, hv⟩
            · exact Wit.grow (b2 k l hv hk) gb
            · obtain ⟨hmb, hsb, _⟩ := b1 bv hbv
              have hGb : Good C res (ev (ev A m a o).ctx m b (ev A m a o).out).ctx :=
                hGa.same ((sat m).ev _ b (ev A m a o).out).2
              cases av with
              | dict ps =>
                cases bv with
                | dict qs =>
                  simp only at hv
                  obtain ⟨_, _, hvv, _⟩ := dictParts_sound hma
                  obtain ⟨_, _, hvv', _⟩ := dictParts_sound hmb
                  rcases bind_err hv with hv | ⟨more, _, hv⟩
                  · exact (IH.merge m _ ρ key ps x y body c _ _ qs _ dep hGb hsb hvv hvv' hw).2 k l hv hk
                  · exact absurd hv (fun h => sized_not_failed h)
                | _ => simp only at hv; rw [fail_type hv] at hk; cases hk
              | _ => simp only at hv; rw [fail_type hv] at hk; cases hk

end

end KanameCore
