/-
  A call read again (kaname's DESIGN 26.5): the body of a generic function analysed once more with
  the values the call gives its parameters. What the call answers is among what that reading says
  (cut to what the result declares), and a failure inside it is one of the places the reading kept
  that is not shown, or one of a function the reading calls (`reread_case`).

  The reading starts from the values of the arguments, not from what the parameters declare: the
  theorem of a function's analysis (`ranges_sound`) holds in any scope whose names hold the
  environment's values (`Sat`), and a call's arguments are among the values the analysis says of
  them. The evaluation of the body is a step shorter than the call's, so the theorem of the step
  before is what the call takes (`SoundAt` at every fewer step, `sound_all`).
-/
import KanameCore.SoundFlow

namespace KanameCore

-- ── A call of a function, as the evaluation makes it ─────────────────────────

theorem callFn_ok {C : Ctx} {n dep f : Nat} {args : List Val} {v : Val} {fn : Fn} (hf : C.M.fns[f]? = some fn)
    (h : callFn C n dep f args = .ok v) :
    ∃ n' raw, n = n' + 1 ∧ eval C n' (dep + 1) ((fn.params.map (·.name)).zip args).reverse fn.body = .ok raw ∧
      coerce fn.conv raw = .ok v ∧ fn.ret.outside false v = false ∧ fn.declared.has v = true := by
  cases n with
  | zero => simp [callFn] at h
  | succ n' =>
    simp only [callFn, hf] at h
    split at h
    · cases h
    · obtain ⟨raw, hraw, h1⟩ := bind_ok h
      obtain ⟨v', hv', h2⟩ := bind_ok h1
      split at h2
      · exact absurd h2 failK_ne_ok
      · rename_i hout
        split at h2
        · rename_i hhas
          cases h2
          exact ⟨n', raw, rfl, hraw, hv', by simpa using hout, hhas⟩
        · exact absurd h2 failK_ne_ok

theorem callFn_err {C : Ctx} {n dep f : Nat} {args : List Val} {k : Kind} {l : Nat} {fn : Fn}
    (hf : C.M.fns[f]? = some fn) (h : callFn C n dep f args = .error (.failed k l)) (hk : k.ofEffect = true) :
    ∃ n', n = n' + 1 ∧
      (eval C n' (dep + 1) ((fn.params.map (·.name)).zip args).reverse fn.body = .error (.failed k l) ∨
       (∃ raw, eval C n' (dep + 1) ((fn.params.map (·.name)).zip args).reverse fn.body = .ok raw ∧
          coerce fn.conv raw = .error (.failed k l)) ∨
       (∃ raw v, eval C n' (dep + 1) ((fn.params.map (·.name)).zip args).reverse fn.body = .ok raw ∧
          coerce fn.conv raw = .ok v ∧ fn.ret.outside false v = true)) := by
  cases n with
  | zero => simp [callFn] at h
  | succ n' =>
    refine ⟨n', rfl, ?_⟩
    simp only [callFn, hf] at h
    split at h
    · cases h
    · rcases bind_err h with h1 | ⟨raw, hraw, h1⟩
      · exact Or.inl h1
      · rcases bind_err h1 with h2 | ⟨v, hv, h2⟩
        · exact Or.inr (Or.inl ⟨raw, hraw, h2⟩)
        · split at h2
          · rename_i hout; exact Or.inr (Or.inr ⟨raw, v, hraw, hv, hout⟩)
          · split at h2
            · cases h2
            · rw [fail_type h2] at hk; cases hk

-- ── The values a call gives its parameters ──────────────────────────────────

/-- The first argument that goes to the parameter `k`, as the analysis reads it and as the evaluation
    gives it: read in its unit, among the values the analysis says; or none on either side. -/
theorem given_find {k : Nat} :
    ∀ (args : List (Nat × Expr × Option Conv)) (avs : List Abs) (vals : List Val) (given : List (Nat × Val)),
      args.length = vals.length → AllMem avs vals → convArgs args vals = .ok given →
      (∃ x c a v v', (args.zip avs).find? (fun y => y.1.1 == k) = some ((k, x, c), a) ∧
          given.find? (·.1 == k) = some (k, v') ∧ Mem a v ∧ coerce c v = .ok v') ∨
        ((args.zip avs).find? (fun y => y.1.1 == k) = none ∧ given.find? (·.1 == k) = none)
  | [], avs, vals, given, _, _, hg => by
    simp only [convArgs] at hg; cases hg
    exact Or.inr ⟨by simp, by simp⟩
  | (j, x, c) :: rest, a :: as, v :: vs, given, hl, hall, hg => by
    obtain ⟨hm, hall'⟩ := hall
    simp only [convArgs, bind, Except.bind] at hg
    rcases hc : coerce c v with e | v' <;> rw [hc] at hg <;> simp at hg
    rcases hmore : convArgs rest vs with e | more <;> rw [hmore] at hg <;> simp at hg
    subst hg
    by_cases hj : j = k
    · subst hj
      exact Or.inl ⟨x, c, a, v, v', by simp, by simp, hm, hc⟩
    · have ih := given_find (k := k) rest as vs more (by simpa using hl) hall' hmore
      have hjk : (j == k) = false := by simp [hj]
      simp only [List.zip_cons_cons, List.find?_cons, hjk]
      exact ih
  | (j, x, c) :: rest, [], vals, given, hl, hall, hg => by
    cases vals with
    | nil => simp at hl
    | cons _ _ => simp [AllMem] at hall
  | (j, x, c) :: rest, a :: as, [], given, hl, hall, _ => by simp at hl

/-- Two lists alike place by place: the same name, the value among what the analysis says. -/
theorem paired_of_get : ∀ (env : List (String × Abs)) (ρ : List (String × Val)), env.length = ρ.length →
    (∀ (i : Nat) (h1 : i < env.length) (h2 : i < ρ.length), env[i].1 = ρ[i].1 ∧ Mem env[i].2 ρ[i].2) →
    Paired env ρ
  | [], [], _, _ => trivial
  | (x, a) :: env, (y, v) :: ρ, hl, h => by
    have h0 := h 0 (by simp) (by simp)
    simp only [List.getElem_cons_zero] at h0
    refine ⟨h0.1, h0.2, paired_of_get env ρ (by simpa using hl) fun i h1 h2 => ?_⟩
    have := h (i + 1) (by simp; omega) (by simp; omega)
    simpa using this
  | [], _ :: _, hl, _ => by simp at hl
  | _ :: _, [], hl, _ => by simp at hl

/-- **The scope a call reads the body in holds the values it gives** (DESIGN 26.5): each parameter
    the value of its first argument, read in its unit, or none where no argument goes (which its type
    takes, as the call held it). -/
theorem sat_reread {A : ACtx} {fn : Fn} {f : Nat} {args : List (Nat × Expr × Option Conv)} {avs : List Abs}
    {vals : List Val} {given : List (Nat × Val)} (hl : args.length = vals.length) (hall : AllMem avs vals)
    (hg : convArgs args vals = .ok given)
    (hhas : (fn.params.zip (place fn.params.length given)).all (fun (p, v) => p.abs.has v) = true) :
    Sat (rereadCtx A fn f args avs) ((fn.params.map (·.name)).zip (place fn.params.length given)).reverse := by
  simp only [rereadCtx]
  apply sat_paired
  apply Paired.reverse
  apply paired_of_get
  · simp [place]
  · intro i h1 h2
    have hi : i < fn.params.length := by simpa using h1
    simp only [List.getElem_map, List.getElem_zipIdx, List.getElem_zip, Nat.zero_add]
    refine ⟨by simp, ?_⟩
    simp only [place, List.getElem_map, List.getElem_range]
    have hhas_i : fn.params[i].abs.has (((given.find? (·.1 == i)).map (·.2)).getD .none) = true := by
      have hmem : (fn.params[i], ((given.find? (·.1 == i)).map (·.2)).getD Val.none) ∈ fn.params.zip (place fn.params.length given) := by
        have hlen : i < (fn.params.zip (place fn.params.length given)).length := by simp [place]; exact hi
        have := List.getElem_mem hlen
        simpa [List.getElem_zip, place] using this
      have := List.all_eq_true.mp hhas _ hmem
      simpa using this
    rcases given_find (k := i) args avs vals given hl hall hg with ⟨x, c, a, v, v', hf, hgf, hm, hc⟩ | ⟨hf, hgf⟩
    · simp only [givenOne, hf, hgf, Option.map_some, Option.getD_some]
      exact Mem_coerce hm hc
    · simp only [givenOne, hf, hgf, Option.map_none, Option.getD_none]
      have : ((given.find? (·.1 == i)).map (·.2)).getD Val.none = Val.none := by simp [hgf]
      rw [this] at hhas_i
      exact has_sound' hhas_i

-- ── A call read again ───────────────────────────────────────────────────────

theorem rereads_fn {A : ACtx} {f : Nat} {fn : Fn} (h : rereads A f = some fn) : A.M.fns[f]? = some fn := by
  unfold rereads at h
  split at h
  · rename_i fn' hf
    split at h
    · cases h; exact hf
    · cases h
  · cases h

/-- **A call read again** (DESIGN 26.5): an answer of the call is among what the reading of the body
    says, cut to what the result declares, and the scope after the call holds the environment; a
    failure of one of the five kinds inside the call is one of the places of the reading that is not
    shown, or one of a function it calls. The reading is of the body, a step shorter than the call:
    the theorem of the steps before (`IHs`) is what it takes. -/
theorem reread_case {C : Ctx} {res : List (Option Abs)} {A : ACtx} {o : Out} {ρ : Env}
    {f line n dep m : Nat} {fn : Fn} {args : List (Nat × Expr × Option Conv)} {avs : List Abs} {vals : List Val}
    (IHs : ∀ k, k < n → SoundAt C res k) (hre : rereads A f = some fn)
    (hG : Good C res A) (hS : Sat A ρ) (hall : AllMem avs vals) (hsyms : ArgSyms ρ (args.map (·.2.1)) vals)
    (hw : (rereadEnd fn ((args.zip avs).foldl (callStep fn) (o, A)).1
            (ev (rereadCtx ((args.zip avs).foldl (callStep fn) (o, A)).2 fn f args avs) m fn.body
              ((args.zip avs).foldl (callStep fn) (o, A)).1)).2.widened = false) :
    (∀ v, (convArgs args vals >>= fun given => callArgs C.M line f given >>= fun ps => callFn C n dep f ps) = .ok v →
      Mem (rereadEnd fn ((args.zip avs).foldl (callStep fn) (o, A)).1
            (ev (rereadCtx ((args.zip avs).foldl (callStep fn) (o, A)).2 fn f args avs) m fn.body
              ((args.zip avs).foldl (callStep fn) (o, A)).1)).1 v ∧
        Sat ((args.zip avs).foldl (callStep fn) (o, A)).2 ρ) ∧
    (∀ k l, (convArgs args vals >>= fun given => callArgs C.M line f given >>= fun ps => callFn C n dep f ps) =
      .error (.failed k l) → k.ofEffect = true →
      Wit C (rereadEnd fn ((args.zip avs).foldl (callStep fn) (o, A)).1
            (ev (rereadCtx ((args.zip avs).foldl (callStep fn) (o, A)).2 fn f args avs) m fn.body
              ((args.zip avs).foldl (callStep fn) (o, A)).1)).2) := by
  -- the scope after the arguments, and the reading
  generalize hF : (args.zip avs).foldl (callStep fn) (o, A) = F at hw ⊢
  generalize hB : rereadCtx F.2 fn f args avs = B at hw ⊢
  generalize hR : ev B m fn.body F.1 = r at hw ⊢
  have hfnA : A.M.fns[f]? = some fn := rereads_fn hre
  have hfn : C.M.fns[f]? = some fn := by rw [← hG.1]; exact hfnA
  have hsame : Same A F.2 := by rw [← hF]; exact callFold_same (Same.refl A)
  have hgF : Grows o F.1 := by rw [← hF]; exact callFold_grows (Grows.refl o)
  have hGB : Good C res B := by
    rw [← hB]; simp only [rereadCtx]
    exact ⟨hsame.1.trans hG.1, hsame.2.1.trans hG.2⟩
  have hgEnd : Grows r.out (rereadEnd fn F.1 r).2 := rereadEnd_grows (Grows.refl _)
  have hwr : r.out.widened = false := grows_not_widened hgEnd hw
  have hgR : Grows F.1 r.out := by rw [← hR]; exact ((sat m).ev _ _ _).1
  have hl : args.length = vals.length := by simpa using ArgSyms.length hsyms
  obtain ⟨hw1, hw2⟩ := callFold_wit (C := C) (fn := fn) args avs vals o A hall
  rw [hF] at hw1 hw2
  -- what the arguments come to, where the call holds them
  have hargs : ∀ given, convArgs args vals = .ok given → ∀ ps, callArgs C.M line f given = .ok ps →
      (∀ k v, (k, v) ∈ given → ∀ p, fn.params[k]? = some p → p.slot.outside true v = false) ∧
      ps = place fn.params.length given ∧
      (fn.params.zip (place fn.params.length given)).all (fun (p, v) => p.abs.has v) = true := by
    intro given hg ps hps
    simp only [callArgs, hfn] at hps
    split at hps
    · simp [failK] at hps
    · rename_i hnout
      split at hps
      · rename_i hhas
        cases hps
        have hnout' : ∀ k w, (k, w) ∈ given → paramOutside fn k w = false := by simpa using hnout
        refine ⟨fun k w hkw p hp => ?_, rfl, hhas⟩
        have := hnout' k w hkw
        simp only [paramOutside, hp] at this
        exact this
      · simp [failK] at hps
  -- the reading, from the values the call gives
  have hread : ∀ given, convArgs args vals = .ok given →
      (fn.params.zip (place fn.params.length given)).all (fun (p, v) => p.abs.has v) = true →
      ∀ n', n = n' + 1 →
      Concl C ((fn.params.map (·.name)).zip (place fn.params.length given)).reverse
        (eval C n' (dep + 1) ((fn.params.map (·.name)).zip (place fn.params.length given)).reverse fn.body) r := by
    intro given hg hhas n' hn
    have hSB : Sat B ((fn.params.map (·.name)).zip (place fn.params.length given)).reverse := by
      rw [← hB]; exact sat_reread hl hall hg hhas
    rw [← hR]
    exact (IHs n' (by omega)).expr m B _ fn.body F.1 (dep + 1) hGB hSB (by rw [hR]; exact hwr)
  constructor
  · intro v hv
    obtain ⟨given, hg, hv⟩ := bind_ok hv
    obtain ⟨ps, hps, hv⟩ := bind_ok hv
    obtain ⟨hout, rfl, hhas⟩ := hargs given hg ps hps
    refine ⟨?_, ?_⟩
    · obtain ⟨n', raw, hn, hraw, hc, _, hdecl⟩ := callFn_ok hfn hv
      obtain ⟨hm, _, _⟩ := (hread given hg hhas n' hn).1 raw hraw
      simp only [rereadEnd]
      exact cutD_sound D _ _ _ (convA_mem hm hc) (has_sound' hdecl)
    · rw [← hF]; exact callFold_sat args avs vals o A hall hsyms hS given hg hout
  · intro k l hv hk
    rcases bind_err hv with hv | ⟨given, hg, hv⟩
    · exact Wit.grow (hw2 _ hv).2 (hgR.trans hgEnd)
    · rcases bind_err hv with hv | ⟨ps, hps, hv⟩
      · simp only [callArgs, hfn] at hv
        split at hv
        · rename_i hany
          have := failK_err hv; subst this
          obtain ⟨⟨k', w⟩, hkw, hp⟩ := List.any_eq_true.mp hany
          simp only [paramOutside] at hp
          split at hp
          · rename_i p hpk
            exact Wit.grow (hw1 given hg k' w hkw p hpk hp) (hgR.trans hgEnd)
          · simp at hp
        · split at hv
          · simp at hv
          · have := failK_err hv; subst this; simp [Kind.ofEffect] at hk
      · obtain ⟨_, rfl, hhas⟩ := hargs given hg ps hps
        obtain ⟨n', hn, h1 | ⟨raw, hraw, h1⟩ | ⟨raw, v, hraw, hc, hout⟩⟩ := callFn_err hfn hv hk
        all_goals have RD := hread given hg hhas n' hn
        · exact Wit.grow (RD.2 k l h1 hk) hgEnd
        · simp only [rereadEnd]
          have := (coerce_wit (C := C) (o := r.out) (RD.1 raw hraw).1 h1).2
          split
          · exact grows_reset (grows_place (Grows.refl _)) |> Wit.grow this
          · exact Wit.grow this (grows_reset (Grows.refl _))
        · have hm := convA_mem (o := r.out) (RD.1 raw hraw).1 hc
          simp only [rereadEnd]
          split
          · exact Wit.grow (wit_place (e := fn.answer) (key := "answer") (kind := .range) (what := .slot fn.ret) hm hout)
              (grows_reset (Grows.refl _))
          · rename_i hno
            simp only [Bool.or_eq_true, not_or, Option.isSome_iff_ne_none, ne_eq, Decidable.not_not] at hno
            rw [outside_noRange hno.1 hno.2] at hout; cases hout

end KanameCore
