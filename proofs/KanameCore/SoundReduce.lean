/-
  `reduce` (kaname's DESIGN 5.5, 15.2.2): the function laid over the running value as many times as
  the list can have elements, or until the running value no longer moves. The analysis that lets
  an end go (`widened`) is left out (DESIGN 15.4).
-/
import KanameCore.SoundApply

namespace KanameCore

mutual
theorem Abs.eq_of_beq : ∀ {a b : Abs}, Abs.beq a b = true → a = b
  | .never, b, h => by cases b <;> simp_all [Abs.beq]
  | .any, b, h => by cases b <;> simp_all [Abs.beq]
  | .num i, b, h => by cases b <;> simp_all [Abs.beq]
  | .date i, b, h => by cases b <;> simp_all [Abs.beq]
  | .set s, b, h => by cases b <;> simp_all [Abs.beq]
  | .str, b, h => by cases b <;> simp_all [Abs.beq]
  | .opt n a, b, h => by
    cases b with
    | opt m b' =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Abs.eq_of_beq h.2]
    | _ => simp [Abs.beq] at h
  | .list c a, b, h => by
    cases b with
    | list d b' =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [h.1, Abs.eq_of_beq h.2]
    | _ => simp [Abs.beq] at h
  | .record f r, b, h => by
    cases b with
    | record g s =>
      simp only [Abs.beq, Bool.and_eq_true, beq_iff_eq] at h
      rw [Abs.eq_of_beqFields h.1, h.2]
    | _ => simp [Abs.beq] at h
  | .transfer f, b, h => by
    cases b with
    | transfer g =>
      simp only [Abs.beq] at h
      rw [Abs.eq_of_beqFields h]
    | _ => simp [Abs.beq] at h
theorem Abs.eq_of_beqFields : ∀ {xs ys : List (String × Abs)}, Abs.beqFields xs ys = true → xs = ys
  | [], [], _ => rfl
  | (n, x) :: xs, (m, y) :: ys, h => by
    simp only [Abs.beqFields, Bool.and_eq_true, beq_iff_eq] at h
    obtain ⟨⟨h1, h2⟩, h3⟩ := h
    rw [h1, Abs.eq_of_beq h2, Abs.eq_of_beqFields h3]
  | [], _ :: _, h => by simp [Abs.beqFields] at h
  | _ :: _, [], h => by simp [Abs.beqFields] at h
end

theorem convA_widened {o : Out} {c : Option Conv} {a : Abs} : (convA o c a).2.widened = o.widened := by
  cases c with
  | none => rfl
  | some c =>
    simp only [convA]
    repeat' split
    all_goals first | rfl | (unfold Out.place; split <;> rfl)

/-- A list of at most `exactRounds` elements. -/
theorem exactRounds_ge {count : Iv} {len e : Nat} (hc : count.In (len : Rat)) (he : exactRounds count = some e) :
    len ≤ e := by
  unfold exactRounds at he
  split at he
  · rename_i h hh
    split at he
    · cases he
      have hle : (len : Rat) ≤ h := by have := hc.hi; rw [hh] at this; exact this
      have h0 : (0 : Rat) ≤ h := Rat.le_trans (by exact_mod_cast Nat.zero_le len) hle
      have hfl : (len : Int) ≤ h.floor := Rat.le_floor_iff.mpr (by exact_mod_cast hle)
      have hnum : h.floor ≤ h.num := by rw [Rat.floor_def]; exact Int.ediv_le_self _ (Rat.num_nonneg.mpr h0)
      omega
    · cases he
  · cases he

theorem count_not_neg {count : Iv} {len : Nat} (hc : count.In (len : Rat))
    (h : count.hi.any (fun h => decide (h.num < 0)) = true) : False := by
  rcases hh : count.hi with _ | u
  · rw [hh] at h; cases h
  · rw [hh] at h
    simp only [Option.any_some, decide_eq_true_eq] at h
    have h1 := hc.hi; rw [hh] at h1
    have h2 := (num_neg_iff u).mp h
    have : (0 : Rat) ≤ len := by exact_mod_cast Nat.zero_le len
    simp only [EndGe] at h1
    grind

theorem bind_of {α β : Type} {x : R α} {f : α → R β} {y : α} {r : R β} (hx : x = .ok y) (hf : f y = r) :
    (x >>= f) = r := by
  rw [hx]; exact hf

section
variable {C : Ctx} {res : List (Option Abs)} {n : Nat}

/-- **`reduce`'s function once**: what it answers for an element and a running value. -/
theorem lam2_step (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {acc x : String} {body : Expr}
    {c : Option Conv} {S elem : Abs} {el a : Val} {o : Out} {dep : Nat} (hG : Good C res A) (hS : Sat A ρ)
    (he : Mem elem el) (ha : Mem S a) (hw : (lam2 A m acc x S elem body c o).2.widened = false) :
    (∀ a', (eval C n dep ((x, el) :: (acc, a) :: ρ) body >>= coerce c) = .ok a' →
      Mem (lam2 A m acc x S elem body c o).1 a') ∧
    (∀ k l, (eval C n dep ((x, el) :: (acc, a) :: ρ) body >>= coerce c) = .error (.failed k l) →
      k.ofEffect = true → Wit C (lam2 A m acc x S elem body c o).2) := by
  cases m with
  | zero => simp only [lam2]; exact ⟨fun a' _ => Mem.any a', fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    simp only [lam2] at hw ⊢
    have hS' : Sat ((A.bindNew acc S).bindNew x elem) ((x, el) :: (acc, a) :: ρ) := (hS.bindNew ha).bindNew he
    obtain ⟨h1, h2⟩ := IH.expr m ((A.bindNew acc S).bindNew x elem) _ body o dep hG hS'
      (grows_not_widened (grows_convA (Grows.refl _)) hw)
    refine ⟨fun a' h => ?_, fun k l h hk => ?_⟩
    · obtain ⟨v, hv, hc⟩ := bind_ok h
      exact convA_mem (h1 v hv).1 hc
    · rcases bind_err h with h | ⟨v, hv, h⟩
      · exact Wit.grow (h2 k l h hk) (grows_convA (Grows.refl _))
      · exact (coerce_wit (h1 v hv).1 h).2

/-- **A running value that no longer moves holds every running value.** -/
theorem fix_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (acc x : String) (body : Expr)
    (c : Option Conv) (S elem : Abs) (xs : List Val) (a : Val) (o : Out) (dep : Nat) (hG : Good C res A)
    (hS : Sat A ρ) (hel : ∀ el ∈ xs, Mem elem el) (ha : Mem S a)
    (hfix : (S.join (lam2 A m acc x S elem body c o).1 == S) = true)
    (hw : (lam2 A m acc x S elem body c o).2.widened = false) :
    (∀ fin, evalFold C (n + 1) dep ρ acc x body c xs a = .ok fin → Mem S fin) ∧
    (∀ k l, evalFold C (n + 1) dep ρ acc x body c xs a = .error (.failed k l) → k.ofEffect = true →
      Wit C (lam2 A m acc x S elem body c o).2) := by
  have hJ : S.join (lam2 A m acc x S elem body c o).1 = S := Abs.eq_of_beq hfix
  cases xs with
  | nil =>
    refine ⟨fun fin h => ?_, fun k l h => ?_⟩
    · simp only [evalFold] at h; cases h; exact ha
    · simp only [evalFold] at h; cases h
  | cons el rest =>
    have he : Mem elem el := hel el (List.mem_cons_self ..)
    have hrest : ∀ e ∈ rest, Mem elem e := fun e h => hel e (List.mem_cons_of_mem _ h)
    obtain ⟨s1, s2⟩ := lam2_step (dep := dep) IH hG hS he ha hw
    refine ⟨fun fin h => ?_, fun k l h hk => ?_⟩
    · simp only [evalFold] at h
      obtain ⟨v, hv, h1⟩ := bind_ok h
      obtain ⟨a', ha', h2⟩ := bind_ok h1
      have hm : Mem S a' := hJ ▸ mem_join_right (s1 a' (bind_of hv ha'))
      exact (IH.fix m A ρ acc x body c S elem rest a' o dep hG hS hrest hm hfix hw).1 fin h2
    · simp only [evalFold] at h
      rcases bind_err h with h | ⟨v, hv, h1⟩
      · exact s2 k l (by rw [h]; rfl) hk
      · rcases bind_err h1 with h2 | ⟨a', ha', h2⟩
        · exact s2 k l (bind_of hv h2) hk
        · have hm : Mem S a' := hJ ▸ mem_join_right (s1 a' (bind_of hv ha'))
          exact (IH.fix m A ρ acc x body c S elem rest a' o dep hG hS hrest hm hfix hw).2 k l h2 hk

theorem rounds_mono {A : ACtx} {acc x : String} {body : Expr} {c : Option Conv} {elem : Abs} {v : Val} :
    ∀ (m K : Nat) (S : Abs) (o : Out), Mem S v → Mem (reduceRounds A m acc x body c elem K S o).1.1 v
  | 0, K, S, o, h => by simp only [reduceRounds]; exact mem_join_left h
  | m + 1, 0, S, o, h => by simp only [reduceRounds]; exact h
  | m + 1, K + 1, S, o, h => by
    simp only [reduceRounds]
    split
    · exact h
    · exact rounds_mono m K _ _ (mem_join_left h)

/-- **The rounds of `reduce`'s function**: every running value among what the rounds come to, when
    the list has no more elements than rounds, or the running value stopped moving. -/
theorem rounds_succ (IH : SoundAt C res n) (m : Nat) (A : ACtx) (ρ : Env) (acc x : String) (body : Expr)
    (c : Option Conv) (elem : Abs) (K : Nat) (S : Abs) (xs : List Val) (a : Val) (o : Out) (dep : Nat)
    (hG : Good C res A) (hS : Sat A ρ) (hel : ∀ el ∈ xs, Mem elem el) (ha : Mem S a)
    (hlen : xs.length ≤ K ∨ (reduceRounds A m acc x body c elem K S o).1.2 = true)
    (hw : (reduceRounds A m acc x body c elem K S o).2.widened = false) :
    (∀ fin, evalFold C (n + 1) dep ρ acc x body c xs a = .ok fin →
      Mem (reduceRounds A m acc x body c elem K S o).1.1 fin) ∧
    (∀ k l, evalFold C (n + 1) dep ρ acc x body c xs a = .error (.failed k l) → k.ofEffect = true →
      Wit C (reduceRounds A m acc x body c elem K S o).2) := by
  cases m with
  | zero =>
    simp only [reduceRounds]
    exact ⟨fun fin _ => mem_join_right (Mem.any fin), fun _ _ _ _ => wit_fuel⟩
  | succ m =>
    cases K with
    | zero =>
      simp only [reduceRounds] at hlen ⊢
      have hnil : xs = [] := by
        rcases hlen with h | h
        · exact List.eq_nil_of_length_eq_zero (Nat.le_zero.mp h)
        · cases h
      subst hnil
      refine ⟨fun fin h => ?_, fun k l h => ?_⟩
      · simp only [evalFold] at h; cases h; exact ha
      · simp only [evalFold] at h; cases h
    | succ K =>
      simp only [reduceRounds] at hlen hw ⊢
      split at hw
      · -- the running value stops moving
        rename_i hfix
        simp only [hfix, ite_true]
        exact fix_succ IH m A ρ acc x body c S elem xs a o dep hG hS hel ha hfix hw
      · rename_i hmove
        simp only [hmove, Bool.false_eq_true, ite_false] at hlen ⊢
        have hwl : (lam2 A m acc x S elem body c o).2.widened = false :=
          grows_not_widened (gRounds (Grows.refl _)) hw
        cases xs with
        | nil =>
          refine ⟨fun fin h => ?_, fun k l h => ?_⟩
          · simp only [evalFold] at h; cases h; exact rounds_mono m K _ _ (mem_join_left ha)
          · simp only [evalFold] at h; cases h
        | cons el rest =>
          have he : Mem elem el := hel el (List.mem_cons_self ..)
          have hrest : ∀ e ∈ rest, Mem elem e := fun e h => hel e (List.mem_cons_of_mem _ h)
          have hlen' : rest.length ≤ K ∨
              (reduceRounds A m acc x body c elem K (S.join (lam2 A m acc x S elem body c o).1)
                (lam2 A m acc x S elem body c o).2).1.2 = true := by
            rcases hlen with h | h
            · exact Or.inl (Nat.le_of_succ_le_succ h)
            · exact Or.inr h
          obtain ⟨s1, s2⟩ := lam2_step (dep := dep) IH hG hS he ha hwl
          refine ⟨fun fin h => ?_, fun k l h hk => ?_⟩
          · simp only [evalFold] at h
            obtain ⟨v, hv, h1⟩ := bind_ok h
            obtain ⟨a', ha', h2⟩ := bind_ok h1
            have hm := mem_join_right (a := S) (s1 a' (bind_of hv ha'))
            exact (IH.rounds m A ρ acc x body c elem K _ rest a' _ dep hG hS hrest hm hlen' hw).1 fin h2
          · simp only [evalFold] at h
            rcases bind_err h with h | ⟨v, hv, h1⟩
            · exact Wit.grow (s2 k l (by rw [h]; rfl) hk) (gRounds (Grows.refl _))
            · rcases bind_err h1 with h2 | ⟨a', ha', h2⟩
              · exact Wit.grow (s2 k l (bind_of hv h2) hk) (gRounds (Grows.refl _))
              · have hm := mem_join_right (a := S) (s1 a' (bind_of hv ha'))
                exact (IH.rounds m A ρ acc x body c elem K _ rest a' _ dep hG hS hrest hm hlen' hw).2 k l h2 hk

theorem case_reduce (IH : SoundAt C res n) {m : Nat} {A : ACtx} {ρ : Env} {o : Out} {dep id line : Nat}
    {list first : Expr} {acc x : String} {body : Expr} {c out : Option Conv} (hG : Good C res A) (hS : Sat A ρ)
    (hw : (evI A m (.reduce id line list first acc x body c out) o).out.widened = false) :
    Concl C ρ (eval C (n + 1) dep ρ (.reduce id line list first acc x body c out))
      (evI A m (.reduce id line list first acc x body c out) o) := by
  cases m with
  | zero => exact concl_fuel hS
  | succ m =>
    simp only [evI] at hw ⊢
    cases m with
    | zero => simp only [reduceAbs]; exact concl_fuel hS
    | succ m =>
      have hGl : Good C res (ev A m list o).ctx := hG.same ((sat m).ev A list o).2
      have hG0 : Good C res (ev (ev A m list o).ctx m first (ev A m list o).out).ctx :=
        hGl.same ((sat m).ev _ first _).2
      simp only [reduceAbs] at hw ⊢
      split
      · -- a list with fewer than no elements: never reached
        rename_i hneg
        simp only [hneg, ite_true, AR.of] at hw ⊢
        obtain ⟨h1, h2⟩ := IH.expr m A ρ list o dep hG hS hw
        refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
        · simp only [eval] at hv
          obtain ⟨lv, hlv, hv1⟩ := bind_ok hv
          obtain ⟨hm, _, _⟩ := h1 lv hlv
          split at hv1
          · exact (count_not_neg (walked_sound hm).1 hneg).elim
          · exact absurd hv1 failK_ne_ok
        · simp only [eval] at hv
          rcases bind_err hv with hv | ⟨lv, hlv, hv1⟩
          · exact h2 k l hv hk
          · obtain ⟨hm, _, _⟩ := h1 lv hlv
            split at hv1
            · exact (count_not_neg (walked_sound hm).1 hneg).elim
            · rw [fail_type hv1] at hk; cases hk
      · rename_i hneg
        simp only [hneg, Bool.false_eq_true, ite_false] at hw
        have w0 : (ev A m list o).out.widened = false := by
          split at hw
          · exact grows_not_widened (gEv (Grows.refl _)) hw
          · split at hw
            · simp only [AR.of] at hw; rw [convA_widened] at hw; simp at hw
            · simp only [AR.of] at hw; rw [convA_widened] at hw
              exact grows_not_widened (gRounds (gEv (Grows.refl _))) hw
        obtain ⟨h1, h2⟩ := IH.expr m A ρ list o dep hG hS w0
        split
        · -- the first value never reached
          rename_i hnev
          simp only [hnev, AR.of] at hw ⊢
          refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
          · simp only [eval] at hv
            obtain ⟨lv, hlv, hv1⟩ := bind_ok hv
            obtain ⟨_, hs, _⟩ := h1 lv hlv
            split at hv1
            · obtain ⟨a0, ha0, _⟩ := bind_ok hv1
              have := ((IH.expr m _ ρ first _ dep hGl hs hw).1 a0 ha0).1
              rw [hnev] at this; exact absurd this Mem.not_never
            · exact absurd hv1 failK_ne_ok
          · simp only [eval] at hv
            rcases bind_err hv with hv | ⟨lv, hlv, hv1⟩
            · exact Wit.grow (h2 k l hv hk) (gEv (Grows.refl _))
            · obtain ⟨_, hs, _⟩ := h1 lv hlv
              split at hv1
              · rcases bind_err hv1 with hv2 | ⟨a0, ha0, _⟩
                · exact (IH.expr m _ ρ first _ dep hGl hs hw).2 k l hv2 hk
                · have := ((IH.expr m _ ρ first _ dep hGl hs hw).1 a0 ha0).1
                  rw [hnev] at this; exact absurd this Mem.not_never
              · rw [fail_type hv1] at hk; cases hk
        · rename_i hne
          split at hw
          · rename_i hnev; exact absurd hnev hne
          · split
            · -- the running value let go: left out
              rename_i hwid
              simp only [hwid, ite_true, AR.of] at hw
              rw [convA_widened] at hw; simp at hw
            · rename_i hwid
              simp only [hwid, Bool.false_eq_true, ite_false, AR.of] at hw ⊢
              rw [convA_widened] at hw
              have w1 : (ev (ev A m list o).ctx m first (ev A m list o).out).out.widened = false :=
                grows_not_widened (gRounds (Grows.refl _)) hw
              -- the list has no more elements than rounds, or the rounds settled
              have hlen : ∀ xs : List Val, Mem (ev A m list o).abs (.list xs) →
                  xs.length ≤ (exactRounds (walked (ev A m list o).abs).1).getD 3 ∨
                  (reduceRounds (ev (ev A m list o).ctx m first (ev A m list o).out).ctx m acc x body c
                    (walked (ev A m list o).abs).2 ((exactRounds (walked (ev A m list o).abs).1).getD 3)
                    (ev (ev A m list o).ctx m first (ev A m list o).out).abs
                    (ev (ev A m list o).ctx m first (ev A m list o).out).out).1.2 = true := by
                intro xs hm
                cases hset : (reduceRounds (ev (ev A m list o).ctx m first (ev A m list o).out).ctx m acc x body c
                    (walked (ev A m list o).abs).2 ((exactRounds (walked (ev A m list o).abs).1).getD 3)
                    (ev (ev A m list o).ctx m first (ev A m list o).out).abs
                    (ev (ev A m list o).ctx m first (ev A m list o).out).out).1.2
                · left
                  rw [hset] at hwid
                  cases hex : exactRounds (walked (ev A m list o).abs).1 with
                  | none => rw [hex] at hwid; simp at hwid
                  | some e => simp only [Option.getD_some]; exact exactRounds_ge (walked_sound hm).1 hex
                · exact Or.inr rfl
              refine ⟨fun v hv => ?_, fun k l hv hk => ?_⟩
              · simp only [eval] at hv
                obtain ⟨lv, hlv, hv1⟩ := bind_ok hv
                obtain ⟨hm, hs, _hn⟩ := h1 lv hlv
                split at hv1
                · rename_i xs
                  obtain ⟨a0, ha0, hv2⟩ := bind_ok hv1
                  obtain ⟨a1, ha1, hv3⟩ := bind_ok hv2
                  obtain ⟨hm0, hs0, _⟩ := (IH.expr m _ ρ first _ dep hGl hs w1).1 a0 ha0
                  obtain ⟨r1, _⟩ := IH.rounds m _ ρ acc x body c _ _ _ xs a0 _ dep hG0 hs0 (walked_sound hm).2 hm0
                    (hlen xs hm) hw
                  exact ⟨convA_mem (r1 a1 ha1) hv3, hs0, fun _ _ => trivial⟩
                · exact absurd hv1 failK_ne_ok
              · simp only [eval] at hv
                rcases bind_err hv with hv | ⟨lv, hlv, hv1⟩
                · exact Wit.grow (h2 k l hv hk) (grows_convA (gRounds (gEv (Grows.refl _))))
                · obtain ⟨hm, hs, _hn⟩ := h1 lv hlv
                  split at hv1
                  · rename_i xs
                    rcases bind_err hv1 with hv2 | ⟨a0, ha0, hv2⟩
                    · exact Wit.grow ((IH.expr m _ ρ first _ dep hGl hs w1).2 k l hv2 hk)
                        (grows_convA (gRounds (Grows.refl _)))
                    · obtain ⟨hm0, hs0, _⟩ := (IH.expr m _ ρ first _ dep hGl hs w1).1 a0 ha0
                      obtain ⟨r1, r2⟩ := IH.rounds m _ ρ acc x body c _ _ _ xs a0 _ dep hG0 hs0 (walked_sound hm).2
                        hm0 (hlen xs hm) hw
                      rcases bind_err hv2 with hv3 | ⟨a1, ha1, hv3⟩
                      · exact Wit.grow (r2 k l hv3 hk) (grows_convA (Grows.refl _))
                      · exact (coerce_wit (r1 a1 ha1) hv3).2
                  · rw [fail_type hv1] at hk; cases hk

end

end KanameCore
