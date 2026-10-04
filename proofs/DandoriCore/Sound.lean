/-
  The check settles what it says (dandori's DESIGN 2.5, E020): a flow that `chkFlow` passes ends,
  in every run where the check looks — `succeed`, `fail`, the end of the flow, the end of
  `on failure` — with every case it does not hand over either not started or in a final state,
  and in one the events on the other side cannot take it out of (`chkFlow_sound`).

  The proof keeps, at each point, the invariant that every run that reaches it is described by
  what the check keeps there (`InvS`): for each case, a pair whose first half is what the flow last
  heard and whose second is a state from which external events lead to the case's true state.
  Every statement keeps it (`exec_sound`); at every end the check looked at the pairs, and so at
  the states.
-/
import DandoriCore.World

namespace DandoriCore

/-! ## Steps and closures -/

theorem Reach.trans {step : Nat → List Nat} {a b c : Nat} (h1 : Reach step a b) (h2 : Reach step b c) :
    Reach step a c := by
  induction h2 with
  | refl => exact h1
  | tail _ hv ih => exact Reach.tail ih hv

theorem closure_complete {step : Nat → List Nat} {b : Nat} {cls : List Nat} (h : closure step b = some cls)
    {t : Nat} (ht : Reach step b t) : t ∈ cls := by
  unfold closure at h
  generalize iterate step 64 [b] = cl0 at h
  unfold checkClosed at h
  split at h
  · next hc =>
    cases h
    simp only [Bool.and_eq_true, closedUnder, List.all_eq_true, List.contains_iff_mem] at hc
    obtain ⟨hcl, hb⟩ := hc
    induction ht with
    | refl => exact hb
    | tail _ hv ih => exact hcl _ ih _ hv
  · cases h

/-! ## What the check keeps, and what it describes -/

/-- A case at a point of a run (what the flow last heard, its true state) is described by the
    pairs: one of them heard the same, and either the case is not started and the pair says so,
    or the external events lead from the pair's state to the true state. -/
def Inv (m : Machine) (P : List Pair) (kt : Option Nat × Option Nat) : Prop :=
  ∃ b, (kt.1, b) ∈ P ∧ ((b = none ∧ kt.2 = none) ∨ ∃ b0 t, b = some b0 ∧ kt.2 = some t ∧ Reach m.ext b0 t)

def InvS (env : Env) (A : Abs) (w : World) : Prop := ∀ i < env.n, Inv (env.ms i) (A i) (w i)

/-- Every pair of one is a pair of the other, for each case. -/
def LeA (env : Env) (a b : Abs) : Prop := ∀ i < env.n, ∀ p ∈ a i, p ∈ b i

def LeO (env : Env) (a b : Option Abs) : Prop := ∀ A, a = some A → ∃ B, b = some B ∧ LeA env A B

def Res.le (env : Env) (r s : Res) : Prop := LeO env r.normal s.normal ∧ LeO env r.brk s.brk ∧ LeO env r.raised s.raised

/-- What a run of a block came to agrees with what the check said of it. -/
def EndFine (env : Env) (leaving : List Nat) (w : World) : Prop :=
  ∀ c < env.n, c ∉ leaving → ∀ t, (w c).2 = some t → ∀ t', Reach (env.ms c).ext t t' → t' ∈ (env.ms c).finals

def OutOk (env : Env) (r : Res) : Out → Prop
  | .normal w => ∃ A, r.normal = some A ∧ InvS env A w
  | .brk w => ∃ A, r.brk = some A ∧ InvS env A w
  | .raised w => ∃ A, r.raised = some A ∧ InvS env A w
  | .ended (some leaving) w => EndFine env leaving w
  | .ended none _ => True

theorem Inv_mono {m : Machine} {P Q : List Pair} {kt : Option Nat × Option Nat} (hpq : ∀ p ∈ P, p ∈ Q)
    (h : Inv m P kt) : Inv m Q kt := by
  obtain ⟨b, hb, h2⟩ := h
  exact ⟨b, hpq _ hb, h2⟩

theorem InvS_mono {env : Env} {A B : Abs} {w : World} (hle : LeA env A B) (h : InvS env A w) : InvS env B w :=
  fun i hi => Inv_mono (hle i hi) (h i hi)

theorem LeA_refl (env : Env) (A : Abs) : LeA env A A := fun _ _ _ hp => hp

theorem LeA_trans {env : Env} {a b c : Abs} (h1 : LeA env a b) (h2 : LeA env b c) : LeA env a c :=
  fun i hi p hp => h2 i hi p (h1 i hi p hp)

theorem mem_unionL_left {a b : List Pair} {p : Pair} (h : p ∈ a) : p ∈ unionL a b :=
  List.mem_append.2 (Or.inl h)

theorem mem_unionL_right {a b : List Pair} {p : Pair} (h : p ∈ b) : p ∈ unionL a b := by
  by_cases ha : p ∈ a
  · exact mem_unionL_left ha
  · exact List.mem_append.2 (Or.inr (List.mem_filter.2 ⟨h, by simpa [List.contains_iff_mem] using ha⟩))

theorem mem_unionL {a b : List Pair} {p : Pair} (h : p ∈ unionL a b) : p ∈ a ∨ p ∈ b := by
  rcases List.mem_append.1 h with h | h
  · exact Or.inl h
  · exact Or.inr (List.mem_filter.1 h).1

theorem LeA_unionA_left (env : Env) (a b : Abs) : LeA env a (unionA a b) := fun _ _ _ hp => mem_unionL_left hp
theorem LeA_unionA_right (env : Env) (a b : Abs) : LeA env b (unionA a b) := fun _ _ _ hp => mem_unionL_right hp

theorem LeO_joinO_left (env : Env) (a b : Option Abs) : LeO env a (joinO a b) := by
  intro A ha
  subst ha
  cases b with
  | none => exact ⟨A, rfl, LeA_refl env A⟩
  | some B => exact ⟨unionA A B, rfl, LeA_unionA_left env A B⟩

theorem LeO_joinO_right (env : Env) (a b : Option Abs) : LeO env b (joinO a b) := by
  intro B hb
  subst hb
  cases a with
  | none => exact ⟨B, rfl, LeA_refl env B⟩
  | some A => exact ⟨unionA A B, rfl, LeA_unionA_right env A B⟩

theorem LeO_refl (env : Env) (a : Option Abs) : LeO env a a := fun A h => ⟨A, h, LeA_refl env A⟩

theorem LeO_trans {env : Env} {a b c : Option Abs} (h1 : LeO env a b) (h2 : LeO env b c) : LeO env a c := by
  intro A ha
  obtain ⟨B, hb, hab⟩ := h1 A ha
  obtain ⟨C, hc, hbc⟩ := h2 B hb
  exact ⟨C, hc, LeA_trans hab hbc⟩

theorem LeO_none (env : Env) (a : Option Abs) : LeO env none a := fun _ h => by cases h

theorem Res.le_refl (env : Env) (r : Res) : r.le env r := ⟨LeO_refl env _, LeO_refl env _, LeO_refl env _⟩

theorem Res.le_join_left (env : Env) (r s : Res) : r.le env (r.join s) :=
  ⟨LeO_joinO_left env _ _, LeO_joinO_left env _ _, LeO_joinO_left env _ _⟩

theorem Res.le_join_right (env : Env) (r s : Res) : s.le env (r.join s) :=
  ⟨LeO_joinO_right env _ _, LeO_joinO_right env _ _, LeO_joinO_right env _ _⟩

theorem Res.le_trans {env : Env} {r s u : Res} (h1 : r.le env s) (h2 : s.le env u) : r.le env u :=
  ⟨LeO_trans h1.1 h2.1, LeO_trans h1.2.1 h2.2.1, LeO_trans h1.2.2 h2.2.2⟩

theorem OutOk_mono {env : Env} {r s : Res} {o : Out} (hle : r.le env s) (h : OutOk env r o) : OutOk env s o := by
  cases o with
  | normal w =>
    obtain ⟨A, hA, hi⟩ := h
    obtain ⟨B, hB, hab⟩ := hle.1 A hA
    exact ⟨B, hB, InvS_mono hab hi⟩
  | brk w =>
    obtain ⟨A, hA, hi⟩ := h
    obtain ⟨B, hB, hab⟩ := hle.2.1 A hA
    exact ⟨B, hB, InvS_mono hab hi⟩
  | raised w =>
    obtain ⟨A, hA, hi⟩ := h
    obtain ⟨B, hB, hab⟩ := hle.2.2 A hA
    exact ⟨B, hB, InvS_mono hab hi⟩
  | ended l w =>
    cases l with
    | none => trivial
    | some L => exact h

theorem subA_le {env : Env} {a b : Abs} (h : subA env.n a b = true) : LeA env a b := by
  intro i hi p hp
  simp only [subA, List.all_eq_true, List.mem_range] at h
  have := h i hi
  simp only [subL, List.all_eq_true, List.contains_iff_mem] at this
  exact this p hp

/-! ## External events, and the ends -/

theorem ExtW_inv {env : Env} {A : Abs} {w w1 : World} (h : InvS env A w) (hx : ExtW env w w1) : InvS env A w1 := by
  intro i hi
  obtain ⟨b, hb, hst⟩ := h i hi
  obtain ⟨hk, hmove⟩ := hx i
  refine ⟨b, hk ▸ hb, ?_⟩
  rcases hmove with ⟨h0, h1⟩ | ⟨t, t', ht, ht', hr⟩
  · rcases hst with ⟨hb0, _⟩ | ⟨b0, t0, _, hs, _⟩
    · exact Or.inl ⟨hb0, h1⟩
    · rw [h0] at hs; cases hs
  · rcases hst with ⟨_, hs⟩ | ⟨b0, t0, hb0, hs, hreach⟩
    · rw [ht] at hs; cases hs
    · rw [ht] at hs
      cases hs
      exact Or.inr ⟨b0, t', hb0, ht', Reach.trans hreach hr⟩

theorem endOk_sound {env : Env} {A : Abs} {w : World} {leaving : List Nat} (h : InvS env A w)
    (hok : endOk env A leaving = true) : EndFine env leaving w := by
  intro c hc hl t ht t' hr
  simp only [endOk, List.all_eq_true, List.mem_range, Bool.or_eq_true, List.contains_iff_mem] at hok
  rcases hok c hc with hin | hall
  · exact absurd hin hl
  · obtain ⟨b, hb, hst⟩ := h c hc
    rcases hst with ⟨_, hs⟩ | ⟨b0, t0, hb0, hs, hreach⟩
    · rw [ht] at hs; cases hs
    · rw [ht] at hs
      cases hs
      subst hb0
      have hp := hall _ hb
      simp only at hp
      split at hp
      · next cls hcl =>
        simp only [List.all_eq_true, List.contains_iff_mem] at hp
        exact hp _ (closure_complete hcl (Reach.trans hreach hr))
      · cases hp

/-! ## A call -/

theorem InvS_set {env : Env} {A : Abs} {w : World} {c : Nat} {P : List Pair} {v : Option Nat × Option Nat}
    (h : InvS env A w) (hc : Inv (env.ms c) P v) : InvS env (setAbs A c P) (setW w c v) := by
  intro i hi
  by_cases hic : i = c
  · subst hic; simpa [setAbs, setW] using hc
  · simpa [setAbs, setW, hic] using h i hi

theorem InvS_widen {env : Env} {A : Abs} {w : World} {c : Nat} {P : List Pair}
    (h : InvS env A w) (hsub : ∀ p ∈ A c, p ∈ P) : InvS env (setAbs A c P) w := by
  intro i hi
  by_cases hic : i = c
  · subst hic; simpa [setAbs] using Inv_mono hsub (h i hi)
  · simpa [setAbs, hic] using h i hi

theorem inv_started {m : Machine} {P : List Pair} {kt : Option Nat × Option Nat} {s : Nat} (h : Inv m P kt)
    (hs : kt.2 = some s) : ∃ b0, (kt.1, some b0) ∈ P ∧ Reach m.ext b0 s := by
  obtain ⟨b, hb, h2⟩ := h
  rcases h2 with ⟨_, h0⟩ | ⟨b0, t, hb0, ht, hr⟩
  · rw [hs] at h0; cases h0
  · subst hb0; rw [hs] at ht; cases ht; exact ⟨b0, hb, hr⟩

theorem inv_unstarted {m : Machine} {P : List Pair} {kt : Option Nat × Option Nat} (h : Inv m P kt)
    (hs : kt.2 = none) : (kt.1, none) ∈ P := by
  obtain ⟨b, hb, h2⟩ := h
  rcases h2 with ⟨hb0, _⟩ | ⟨b0, t, hb0, ht, hr⟩
  · subst hb0; exact hb
  · rw [hs] at ht; cases ht

theorem cl_complete {m : Machine} {P : List Pair} {k : Option Nat} {b0 t : Nat} (hok : closuresOk m P = true)
    (hp : (k, some b0) ∈ P) (hr : Reach m.ext b0 t) : t ∈ cl m b0 := by
  simp only [closuresOk, List.all_eq_true] at hok
  have := hok _ hp
  simp only at this
  cases hc : closure m.ext b0 with
  | none => rw [hc] at this; cases this
  | some cls => simp only [cl, hc, Option.getD_some]; exact closure_complete hc hr

theorem plain_step {env : Env} {tgt : Option Target} {callee : Callee} {w : World} {o : CallOut}
    (hc : caseCall env tgt callee = none) (hs : CallStep env tgt callee w o) : o = .ok w ∨ ∃ k, o = .err k w := by
  cases hs <;> simp_all

theorem caseCall_eq {env : Env} {c t : Nat} {eff : Effect}
    (heff : ((env.flow.tasks[t]?).map Task.effect : Option Effect) = some eff) :
    caseCall env (some (.case c)) (.task t) = some (c, eff) := by
  obtain ⟨tk, htk, he⟩ := Option.map_eq_some_iff.1 heff
  simp [caseCall, htk, he]

/-- **What the check makes of a call describes every way it can come out.** -/
theorem callAbs_sound {env : Env} {tgt : Option Target} {callee : Callee} {A aok aerr : Abs} {w1 : World}
    (hca : callAbs env tgt callee A = some (aok, aerr)) (h : InvS env A w1) :
    (∀ w2, CallStep env tgt callee w1 (.ok w2) → InvS env aok w2) ∧
    (∀ k w2, CallStep env tgt callee w1 (.err k w2) → InvS env aerr w2) := by
  have plain : caseCall env tgt callee = none → aok = A → aerr = A →
      (∀ w2, CallStep env tgt callee w1 (.ok w2) → InvS env aok w2) ∧
      (∀ k w2, CallStep env tgt callee w1 (.err k w2) → InvS env aerr w2) := by
    intro hcc h1 h2
    subst h1; subst h2
    constructor
    · intro w2 hs
      rcases plain_step hcc hs with he | ⟨k, he⟩
      · cases he; exact h
      · cases he
    · intro k w2 hs
      rcases plain_step hcc hs with he | ⟨k', he⟩
      · cases he
      · cases he; exact h
  cases tgt with
  | none =>
    simp only [callAbs, Option.some.injEq, Prod.mk.injEq] at hca
    exact plain rfl hca.1.symm hca.2.symm
  | some tg =>
    cases tg with
    | letVar x =>
      simp only [callAbs, Option.some.injEq, Prod.mk.injEq] at hca
      exact plain rfl hca.1.symm hca.2.symm
    | case c =>
      cases callee with
      | rule r =>
        simp only [callAbs, Option.some.injEq, Prod.mk.injEq] at hca
        exact plain rfl hca.1.symm hca.2.symm
      | task t =>
        simp only [callAbs] at hca
        split at hca
        · next hcn =>
          have hinv := h c hcn
          split at hca
          · next e heff =>
            have hcc := caseCall_eq (c := c) heff
            split at hca
            · next hcond =>
              simp only [Bool.and_eq_true] at hcond
              obtain ⟨_, hclo⟩ := hcond
              simp only [Option.some.injEq, Prod.mk.injEq] at hca
              obtain ⟨h1, h2⟩ := hca
              subst h1; subst h2
              constructor
              · intro w2 hs
                cases hs with
                | plainOk h0 => rw [hcc] at h0; cases h0
                | sendOk h0 hs hs' =>
                  rw [hcc] at h0
                  simp only [Option.some.injEq, Prod.mk.injEq, Effect.sends.injEq] at h0
                  obtain ⟨h1, h2⟩ := h0
                  subst h1; subst h2
                  apply InvS_set h
                  obtain ⟨b0, hb0, hr⟩ := inv_started hinv hs
                  refine ⟨some _, ?_, Or.inr ⟨_, _, rfl, rfl, Reach.refl _⟩⟩
                  simp only [sendOk, List.mem_flatMap]
                  refine ⟨_, hb0, ?_⟩
                  simp only [List.mem_flatMap, List.mem_map]
                  exact ⟨_, cl_complete hclo hb0 hr, _, hs', rfl⟩
                | startOk h0 _ _ => rw [hcc] at h0; simp at h0
                | observeOk h0 _ => rw [hcc] at h0; simp at h0
                | observeFound h0 _ _ => rw [hcc] at h0; simp at h0
              · intro k w2 hs
                have hsub : ∀ p ∈ A c, p ∈ sendErr (env.ms c) e (A c) := fun p hp => List.mem_append.2 (Or.inl hp)
                cases hs with
                | plainErr h0 => rw [hcc] at h0; cases h0
                | sendRefused h0 _ _ => exact InvS_widen h hsub
                | sendFailed h0 => exact InvS_widen h hsub
                | sendFailedAfter h0 hs hs' =>
                  rw [hcc] at h0
                  simp only [Option.some.injEq, Prod.mk.injEq, Effect.sends.injEq] at h0
                  obtain ⟨h1, h2⟩ := h0
                  subst h1; subst h2
                  apply InvS_set h
                  obtain ⟨b0, hb0, hr⟩ := inv_started hinv hs
                  refine ⟨some _, ?_, Or.inr ⟨_, _, rfl, rfl, Reach.refl _⟩⟩
                  apply List.mem_append.2 (Or.inr _)
                  simp only [List.mem_flatMap]
                  refine ⟨_, hb0, ?_⟩
                  simp only [List.mem_flatMap, List.mem_map]
                  exact ⟨_, cl_complete hclo hb0 hr, _, hs', rfl⟩
                | startFailed h0 => rw [hcc] at h0; simp at h0
                | startFailedAfter h0 _ _ => rw [hcc] at h0; simp at h0
                | observeFailed h0 => rw [hcc] at h0; simp at h0
            · cases hca
          · next es heff =>
            have hcc := caseCall_eq (c := c) heff
            split at hca
            · simp only [Option.some.injEq, Prod.mk.injEq] at hca
              obtain ⟨h1, h2⟩ := hca
              subst h1; subst h2
              constructor
              · intro w2 hs
                cases hs with
                | plainOk h0 => rw [hcc] at h0; cases h0
                | sendOk h0 _ _ => rw [hcc] at h0; simp at h0
                | startOk h0 hnone hs =>
                  rw [hcc] at h0
                  simp only [Option.some.injEq, Prod.mk.injEq, Effect.starts.injEq] at h0
                  obtain ⟨h1, h2⟩ := h0
                  subst h1; subst h2
                  apply InvS_set h
                  exact ⟨some _, List.mem_map.2 ⟨_, hs, rfl⟩, Or.inr ⟨_, _, rfl, rfl, Reach.refl _⟩⟩
                | observeOk h0 _ => rw [hcc] at h0; simp at h0
                | observeFound h0 _ _ => rw [hcc] at h0; simp at h0
              · intro k w2 hs
                have hsub : ∀ p ∈ A c, p ∈ startErr (env.ms c) es (A c) := fun p hp => List.mem_append.2 (Or.inl hp)
                cases hs with
                | plainErr h0 => rw [hcc] at h0; cases h0
                | sendRefused h0 _ _ => rw [hcc] at h0; simp at h0
                | sendFailed h0 => rw [hcc] at h0; simp at h0
                | sendFailedAfter h0 _ _ => rw [hcc] at h0; simp at h0
                | startFailed h0 => exact InvS_widen h hsub
                | startFailedAfter h0 hnone hs =>
                  rw [hcc] at h0
                  simp only [Option.some.injEq, Prod.mk.injEq, Effect.starts.injEq] at h0
                  obtain ⟨h1, h2⟩ := h0
                  subst h1; subst h2
                  apply InvS_set h
                  have hm := inv_unstarted hinv hnone
                  refine ⟨some _, ?_, Or.inr ⟨_, _, rfl, rfl, Reach.refl _⟩⟩
                  apply List.mem_append.2 (Or.inr _)
                  simp only [List.mem_flatMap, List.mem_map]
                  exact ⟨_, hm, _, hs, rfl⟩
                | observeFailed h0 => rw [hcc] at h0; simp at h0
            · cases hca
          · next heff =>
            have hcc := caseCall_eq (c := c) heff
            split at hca
            · next hcond =>
              simp only [Bool.and_eq_true, Bool.or_eq_true] at hcond
              obtain ⟨hclo, hany⟩ := hcond
              simp only [Option.some.injEq, Prod.mk.injEq] at hca
              obtain ⟨h1, h2⟩ := hca
              subst h1; subst h2
              constructor
              · intro w2 hs
                cases hs with
                | plainOk h0 => rw [hcc] at h0; cases h0
                | sendOk h0 _ _ => rw [hcc] at h0; simp at h0
                | startOk h0 _ _ => rw [hcc] at h0; simp at h0
                | observeOk h0 ht =>
                  rw [hcc] at h0
                  simp only [Option.some.injEq, Prod.mk.injEq] at h0
                  obtain ⟨h1, _⟩ := h0
                  subst h1
                  apply InvS_set h
                  obtain ⟨b0, hb0, hr⟩ := inv_started hinv ht
                  refine ⟨some _, ?_, Or.inr ⟨_, _, rfl, rfl, Reach.refl _⟩⟩
                  simp only [observeOk, List.mem_flatMap]
                  refine ⟨_, hb0, ?_⟩
                  simp only [List.mem_map]
                  exact ⟨_, cl_complete hclo hb0 hr, rfl⟩
                | observeFound h0 hnone hr =>
                  rw [hcc] at h0
                  simp only [Option.some.injEq, Prod.mk.injEq] at h0
                  obtain ⟨h1, _⟩ := h0
                  subst h1
                  apply InvS_set h
                  have hm := inv_unstarted hinv hnone
                  have hclAny : ∃ cls, closure (env.ms c).any (env.ms c).initial = some cls := by
                    rcases hany with hall | hsome
                    · simp only [allStarted, List.all_eq_true] at hall
                      have := hall _ hm
                      simp at this
                    · cases hcl : closure (env.ms c).any (env.ms c).initial with
                      | none => rw [hcl] at hsome; cases hsome
                      | some cls => exact ⟨cls, rfl⟩
                  obtain ⟨cls, hcls⟩ := hclAny
                  refine ⟨some _, ?_, Or.inr ⟨_, _, rfl, rfl, Reach.refl _⟩⟩
                  simp only [observeOk, List.mem_flatMap]
                  refine ⟨_, hm, ?_⟩
                  simp only [List.mem_map, clAny, hcls, Option.getD_some]
                  exact ⟨_, closure_complete hcls hr, rfl⟩
              · intro k w2 hs
                cases hs with
                | plainErr h0 => rw [hcc] at h0; cases h0
                | sendRefused h0 _ _ => rw [hcc] at h0; simp at h0
                | sendFailed h0 => rw [hcc] at h0; simp at h0
                | sendFailedAfter h0 _ _ => rw [hcc] at h0; simp at h0
                | startFailed h0 => rw [hcc] at h0; simp at h0
                | startFailedAfter h0 _ _ => rw [hcc] at h0; simp at h0
                | observeFailed h0 => exact h
            · cases hca
          · cases hca
        · cases hca

/-! ## A match, the handlers, the arms -/

theorem narrow_sound {env : Env} {e : Expr} {arm : Arm} {A : Abs} {w : World}
    (_hn : ∀ c, caseOf env e = some c → c < env.n) (h : InvS env A w) (hm : ArmMay env e arm w) :
    InvS env (narrow env e arm A) w := by
  unfold narrow
  unfold ArmMay at hm
  split
  · next c hc =>
    rw [hc] at hm
    simp only at hm
    intro i hi
    by_cases hic : i = c
    · subst hic
      simp only [setAbs]
      obtain ⟨b, hb, h2⟩ := h i hi
      exact ⟨b, List.mem_filter.2 ⟨hb, hm⟩, h2⟩
    · simpa [setAbs, hic] using h i hi
  · exact h

theorem chkHandlers_mem (env : Env) {rec : List Stmt → Abs → Option Res} {A : Abs} :
    ∀ {hs : List Handler} {R : Res}, chkHandlers rec A hs = some R →
      ∀ h ∈ hs, ∃ r, rec h.body A = some r ∧ r.le env R
  | [], _, _, h, hh => by cases hh
  | (.mk errs body) :: rest, R, hc, h, hh => by
    simp only [chkHandlers] at hc
    split at hc
    · next r rs hr hrs =>
      cases hc
      rcases List.mem_cons.1 hh with he | he
      · subst he
        exact ⟨r, hr, Res.le_join_left env r rs⟩
      · obtain ⟨r', hr', hle⟩ := chkHandlers_mem env hrs h he
        exact ⟨r', hr', Res.le_trans hle (Res.le_join_right env r rs)⟩
    · cases hc

theorem chkArms_mem (env : Env) {rec : List Stmt → Abs → Option Res} {e : Expr} {A : Abs} :
    ∀ {arms : List Arm} {R : Res}, chkArms env rec e A arms = some R →
      ∀ arm ∈ arms, ∃ r, rec arm.body (narrow env e arm A) = some r ∧ r.le env R
  | [], _, _, arm, ha => by cases ha
  | (.mk vs isNone someName body) :: rest, R, hc, arm, ha => by
    simp only [chkArms] at hc
    split at hc
    · next r rs hr hrs =>
      cases hc
      rcases List.mem_cons.1 ha with he | he
      · subst he
        exact ⟨r, hr, Res.le_join_left env r rs⟩
      · obtain ⟨r', hr', hle⟩ := chkArms_mem env hrs arm he
        exact ⟨r', hr', Res.le_trans hle (Res.le_join_right env r rs)⟩
    · cases hc

theorem catchesAll_false {k : String} : ∀ {hs : List Handler}, (∀ h ∈ hs, ¬ h.takes k) → catchesAll hs = false
  | [], _ => rfl
  | (.mk errs body) :: rest, hno => by
    simp only [catchesAll, Bool.or_eq_false_iff]
    constructor
    · rw [Bool.eq_false_iff]
      intro hany
      apply hno (.mk errs body) List.mem_cons_self
      simp only [Handler.takes, List.any_eq_true] at hany ⊢
      obtain ⟨x, hx, hf⟩ := hany
      refine ⟨x, hx, ?_⟩
      cases x <;> simp_all [HErr.hits]
    · exact catchesAll_false (fun h hh => hno h (List.mem_cons_of_mem _ hh))

/-! ## Loops -/

/-- What `fix` finds: a head that holds where the loop starts, and that a round ends inside. -/
theorem fix_spec {env : Env} {bd : Abs → Option Res} :
    ∀ (k : Nat) (A H : Abs), fix env.n bd k A = some H →
      LeA env A H ∧ ∃ rb, bd H = some rb ∧ (rb.normal = none ∨ ∃ N, rb.normal = some N ∧ subA env.n N H = true)
  | 0, _, _, h => by simp [fix] at h
  | k + 1, A, H, h => by
    simp only [fix] at h
    split at h
    · cases h
    · next rb hrb =>
      split at h
      · next hn =>
        cases h
        exact ⟨LeA_refl env A, rb, hrb, Or.inl hn⟩
      · next N hN =>
        split at h
        · next hsub =>
          cases h
          exact ⟨LeA_refl env A, rb, hrb, Or.inr ⟨N, hN, hsub⟩⟩
        · obtain ⟨hle, rest⟩ := fix_spec k _ H h
          exact ⟨LeA_trans (LeA_unionA_left env A N) hle, rest⟩

theorem fix_stable {env : Env} {bd : Abs → Option Res} {H : Abs} {rb : Res} (hb : bd H = some rb)
    (hs : rb.normal = none ∨ ∃ N, rb.normal = some N ∧ subA env.n N H = true) : fix env.n bd 64 H = some H := by
  rw [show (64 : Nat) = 63 + 1 from rfl, fix, hb]
  rcases hs with hn | ⟨N, hN, hsub⟩
  · simp only [hn]
  · simp only [hN, hsub, if_true]

theorem loopRes_spec {env : Env} {bd : Abs → Option Res} {A : Abs} {r : Res} (h : loopRes env.n bd A = some r) :
    ∃ H rb, bd H = some rb ∧ LeA env A H ∧
      (rb.normal = none ∨ ∃ N, rb.normal = some N ∧ subA env.n N H = true) ∧
      r = { normal := joinO (some H) rb.brk, brk := none, raised := rb.raised } ∧
      loopRes env.n bd H = some r := by
  unfold loopRes at h
  split at h
  · cases h
  · next H hH =>
    obtain ⟨hle, rb, hrb, hst⟩ := fix_spec 64 A H hH
    rw [hrb] at h
    simp only [Option.some.injEq] at h
    subst h
    refine ⟨H, rb, hrb, hle, hst, rfl, ?_⟩
    unfold loopRes
    simp only [fix_stable hrb hst, hrb]

/-- A round of a loop that ends inside the head starts the next round inside it. -/
theorem round_in_head {env : Env} {H : Abs} {rb : Res} {w : World}
    (hst : rb.normal = none ∨ ∃ N, rb.normal = some N ∧ subA env.n N H = true)
    (hok : OutOk env rb (.normal w)) : InvS env H w := by
  obtain ⟨N, hN, hi⟩ := hok
  rcases hst with hn | ⟨N', hN', hsub⟩
  · rw [hn] at hN; cases hN
  · rw [hN'] at hN
    cases hN
    exact InvS_mono (subA_le hsub) hi

/-- Where a loop goes on: its head, and where a `break` left a round. -/
theorem loop_normal {env : Env} {A H : Abs} {rb : Res} (hle : LeA env A H) :
    LeO env (some A) (joinO (some H) rb.brk) := by
  intro A' hA'
  cases hA'
  obtain ⟨B, hB, hHB⟩ := LeO_joinO_left env (some H) rb.brk H rfl
  exact ⟨B, hB, LeA_trans hle hHB⟩

/-! ## Blocks -/

theorem chk_cons_eq {env : Env} {f : Nat} {s : Stmt} {rest : List Stmt} {A : Abs} {r : Res}
    (h : chk env (f + 1) (s :: rest) A = some r) :
    ∃ r1, chkHead env (chk env f) s A = some r1 ∧
      ((r1.normal = none ∧ r = { r1 with normal := none }) ∨
       ∃ N r2, r1.normal = some N ∧ chk env f rest N = some r2 ∧
         r = { normal := r2.normal, brk := joinO r1.brk r2.brk, raised := joinO r1.raised r2.raised }) := by
  simp only [chk] at h
  split at h
  · cases h
  · next r1 hr1 =>
    refine ⟨r1, hr1, ?_⟩
    split at h
    · next hn => cases h; exact Or.inl ⟨hn, rfl⟩
    · next N hN =>
      split at h
      · cases h
      · next r2 hr2 => cases h; exact Or.inr ⟨N, r2, hN, hr2, rfl⟩

/-- What the rest of a block comes to, the block comes to. -/
theorem lift_rest {env : Env} {r1 r2 : Res} {o : Out} (h : OutOk env r2 o) :
    OutOk env { normal := r2.normal, brk := joinO r1.brk r2.brk, raised := joinO r1.raised r2.raised } o := by
  have hle : r2.le env { normal := r2.normal, brk := joinO r1.brk r2.brk, raised := joinO r1.raised r2.raised } :=
    ⟨LeO_refl env r2.normal, LeO_joinO_right env r1.brk r2.brk, LeO_joinO_right env r1.raised r2.raised⟩
  exact OutOk_mono hle h

/-- What a block's first statement stops with, the block stops with. -/
theorem lift_head {env : Env} {f : Nat} {rest : List Stmt} {r1 r : Res} {o : Out} (hstop : o.stops)
    (hr : (r1.normal = none ∧ r = { r1 with normal := none }) ∨
      ∃ N r2, r1.normal = some N ∧ chk env f rest N = some r2 ∧
        r = { normal := r2.normal, brk := joinO r1.brk r2.brk, raised := joinO r1.raised r2.raised })
    (h : OutOk env r1 o) : OutOk env r o := by
  cases o with
  | normal w => exact absurd hstop (by simp [Out.stops])
  | brk w =>
    obtain ⟨A, hA, hi⟩ := h
    rcases hr with ⟨_, rfl⟩ | ⟨N, r2, _, _, rfl⟩
    · exact ⟨A, hA, hi⟩
    · obtain ⟨B, hB, hab⟩ := LeO_joinO_left env r1.brk r2.brk A hA
      exact ⟨B, hB, InvS_mono hab hi⟩
  | raised w =>
    obtain ⟨A, hA, hi⟩ := h
    rcases hr with ⟨_, rfl⟩ | ⟨N, r2, _, _, rfl⟩
    · exact ⟨A, hA, hi⟩
    · obtain ⟨B, hB, hab⟩ := LeO_joinO_left env r1.raised r2.raised A hA
      exact ⟨B, hB, InvS_mono hab hi⟩
  | ended l w =>
    cases l with
    | none => trivial
    | some L => exact h

/-- A block's first statement ends inside what the check said; the rest comes to what the block
    does. -/
theorem go_on {env : Env} {f : Nat} {rest : List Stmt} {r1 r : Res} {A1 : Abs} {w1 : World} {o : Out}
    (hr : (r1.normal = none ∧ r = { r1 with normal := none }) ∨
      ∃ N r2, r1.normal = some N ∧ chk env f rest N = some r2 ∧
        r = { normal := r2.normal, brk := joinO r1.brk r2.brk, raised := joinO r1.raised r2.raised })
    (hA1 : LeO env (some A1) r1.normal) (hi : InvS env A1 w1)
    (ih : ∀ f' A' r', chk env f' rest A' = some r' → InvS env A' w1 → OutOk env r' o) : OutOk env r o := by
  obtain ⟨B, hB, hab⟩ := hA1 A1 rfl
  rcases hr with ⟨hn, _⟩ | ⟨N, r2, hN, hr2, rfl⟩
  · rw [hn] at hB; cases hB
  · rw [hN] at hB
    simp only [Option.some.injEq] at hB
    subst hB
    exact lift_rest (ih f _ r2 hr2 (InvS_mono hab hi))

theorem chkHead_call {env : Env} {rec : List Stmt → Abs → Option Res} {site line : Nat} {tgt : Option Target}
    {callee : Callee} {hs : List Handler} {A : Abs} {r1 : Res}
    (hh : chkHead env rec (.call site line tgt callee hs) A = some r1) :
    ∃ aok aerr rh, callAbs env tgt callee A = some (aok, aerr) ∧ chkHandlers rec aerr hs = some rh ∧
      r1 = { normal := joinO (some aok) rh.normal, brk := rh.brk,
             raised := joinO (if catchesAll hs then none else some aerr) rh.raised } := by
  simp only [chkHead] at hh
  split at hh
  · cases hh
  · next aok aerr hca =>
    split at hh
    · cases hh
    · next rh hrh => cases hh; exact ⟨aok, aerr, rh, hca, hrh, rfl⟩

theorem chkHead_repeat {env : Env} {rec : List Stmt → Abs → Option Res} {site t : Nat} {body : List Stmt}
    {A : Abs} {r1 : Res} (hh : chkHead env rec (.repeat site t body) A = some r1) :
    loopRes env.n (rec body) A = some r1 := by
  simpa [chkHead] using hh

theorem chkHead_for {env : Env} {rec : List Stmt → Abs → Option Res} {site line : Nat} {x : String} {list : Expr}
    {max : Nat} {body : List Stmt} {result : Option (String × Expr)} {par : Option (List String)}
    {A : Abs} {r1 : Res} (hh : chkHead env rec (.forEach site line x list max body result par) A = some r1) :
    par = none ∧ loopRes env.n (rec body) A = some r1 := by
  simp only [chkHead] at hh
  split at hh
  · cases hh
  · next hp => exact ⟨by simpa using hp, hh⟩

theorem chkHead_match {env : Env} {rec : List Stmt → Abs → Option Res} {site line : Nat} {e : Expr} {shown : String}
    {arms : List Arm} {A : Abs} {r1 : Res} (hh : chkHead env rec (.matchOn site line e shown arms) A = some r1) :
    chkArms env rec e A arms = some r1 ∧ ∀ c, caseOf env e = some c → c < env.n := by
  simp only [chkHead] at hh
  split at hh
  · next c hc =>
    split at hh
    · next hcn => exact ⟨hh, fun c2 h2 => by rw [hc] at h2; cases h2; exact hcn⟩
    · cases hh
  · next hc => exact ⟨hh, fun c2 h2 => by rw [hc] at h2; cases h2⟩

/-- A loop checked from its head comes to what it came to from where it started. -/
theorem chk_again {env : Env} {f : Nat} {s : Stmt} {rest : List Stmt} {A H : Abs} {r1 : Res}
    (hA : chkHead env (chk env f) s A = some r1) (hH : chkHead env (chk env f) s H = some r1) :
    chk env (f + 1) (s :: rest) H = chk env (f + 1) (s :: rest) A := by
  simp only [chk, hA, hH]

/-- **Every statement keeps the invariant**: every way a run of a block comes out agrees with what
    the check said of it, from any point the check describes. -/
theorem exec_sound {env : Env} {ss : List Stmt} {w : World} {o : Out} (h : Exec env ss w o) :
    ∀ f A r, chk env f ss A = some r → InvS env A w → OutOk env r o := by
  induction h with
  | nil =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f => simp only [chk, Option.some.injEq] at hc; subst hc; exact ⟨A, rfl, hi⟩
  | pass _ ih | assign _ ih | wait _ ih =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      simp only [chkHead, Option.some.injEq] at hh
      subst hh
      exact go_on hr (LeO_refl _ _) hi ih
  | brk =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      simp only [chkHead, Option.some.injEq] at hh
      subst hh
      exact lift_head (by simp [Out.stops]) hr ⟨A, rfl, hi⟩
  | succeed =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      simp only [chkHead] at hh
      split at hh
      · next hok => exact endOk_sound hi hok
      · cases hh
  | fail =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      simp only [chkHead] at hh
      split at hh
      · next hok => exact endOk_sound hi hok
      · cases hh
  | matchArm harm hmay _ _ ihb ihr =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨hR, hn⟩ := chkHead_match hh
      obtain ⟨ra, hra, hle⟩ := chkArms_mem env hR _ harm
      obtain ⟨A1, hA1, hi1⟩ := ihb f _ ra hra (narrow_sound hn hi hmay)
      exact go_on hr (fun A' hA' => by cases hA'; exact hle.1 A1 hA1) hi1 ihr
  | matchArmStop harm hmay _ hstop ihb =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨hR, hn⟩ := chkHead_match hh
      obtain ⟨ra, hra, hle⟩ := chkArms_mem env hR _ harm
      exact lift_head hstop hr (OutOk_mono hle (ihb f _ ra hra (narrow_sound hn hi hmay)))
  | matchNone => intro _ _ _ _ _; trivial
  | repeatExit _ ih =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨H, rb, hrb, hle, hst, rfl, _⟩ := loopRes_spec (chkHead_repeat hh)
      exact go_on hr (loop_normal hle) hi ih
  | repeatRound _ _ ihb ihl =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      have hloopA := chkHead_repeat hh
      obtain ⟨H, rb, hrb, hle, hst, hr1, hloopH⟩ := loopRes_spec hloopA
      have hiH : InvS env H _ := InvS_mono hle hi
      have hi1 := round_in_head hst (ihb f H rb hrb hiH)
      apply ihl (f + 1) H r _ hi1
      rw [chk_again (rest := _) hh (by simp only [chkHead]; exact hloopH)]
      exact hc
  | repeatBreak _ _ ihb ihr =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨H, rb, hrb, hle, hst, hr1, _⟩ := loopRes_spec (chkHead_repeat hh)
      obtain ⟨B, hB, hiB⟩ := ihb f H rb hrb (InvS_mono hle hi)
      subst hr1
      exact go_on hr (fun A' hA' => by cases hA'; exact LeO_joinO_right env (some H) rb.brk B hB) hiB ihr
  | @repeatStop site t body rest w o' _ hhalt ihb =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨H, rb, hrb, hle, hst, hr1, _⟩ := loopRes_spec (chkHead_repeat hh)
      have hok := ihb f H rb hrb (InvS_mono hle hi)
      subst hr1
      cases o' with
      | normal _ => exact absurd hhalt (by simp [Out.halts])
      | brk _ => exact absurd hhalt (by simp [Out.halts])
      | raised w' => exact lift_head (by simp [Out.stops]) hr hok
      | ended l w' =>
        cases l with
        | none => trivial
        | some L => exact lift_head (by simp [Out.stops]) hr (show EndFine env L w' from hok)
  | forExit _ ih =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨_, hloop⟩ := chkHead_for hh
      obtain ⟨H, rb, hrb, hle, hst, rfl, _⟩ := loopRes_spec hloop
      exact go_on hr (loop_normal hle) hi ih
  | forRound _ _ ihb ihl =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨hpar, hloopA⟩ := chkHead_for hh
      obtain ⟨H, rb, hrb, hle, hst, hr1, hloopH⟩ := loopRes_spec hloopA
      have hi1 := round_in_head hst (ihb f H rb hrb (InvS_mono hle hi))
      apply ihl (f + 1) H r _ hi1
      rw [chk_again (rest := _) hh (by simp only [chkHead, hpar]; exact hloopH)]
      exact hc
  | forBreak _ _ ihb ihr =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨_, hloop⟩ := chkHead_for hh
      obtain ⟨H, rb, hrb, hle, hst, hr1, _⟩ := loopRes_spec hloop
      obtain ⟨B, hB, hiB⟩ := ihb f H rb hrb (InvS_mono hle hi)
      subst hr1
      exact go_on hr (fun A' hA' => by cases hA'; exact LeO_joinO_right env (some H) rb.brk B hB) hiB ihr
  | @forStop site line x list max body result par rest w o' _ hhalt ihb =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨_, hloop⟩ := chkHead_for hh
      obtain ⟨H, rb, hrb, hle, hst, hr1, _⟩ := loopRes_spec hloop
      have hok := ihb f H rb hrb (InvS_mono hle hi)
      subst hr1
      cases o' with
      | normal _ => exact absurd hhalt (by simp [Out.halts])
      | brk _ => exact absurd hhalt (by simp [Out.halts])
      | raised w' => exact lift_head (by simp [Out.stops]) hr hok
      | ended l w' =>
        cases l with
        | none => trivial
        | some L => exact lift_head (by simp [Out.stops]) hr (show EndFine env L w' from hok)
  | forTooMany => intro _ _ _ _ _; trivial
  | callOk hx hst _ ih =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨aok, aerr, rh, hca, hrh, rfl⟩ := chkHead_call hh
      have hi2 := (callAbs_sound hca (ExtW_inv hi hx)).1 _ hst
      exact go_on hr (LeO_joinO_left env (some aok) rh.normal) hi2 ih
  | callHandled hx hst hmem _ _ _ ihb ihr =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨aok, aerr, rh, hca, hrh, rfl⟩ := chkHead_call hh
      have hi2 := (callAbs_sound hca (ExtW_inv hi hx)).2 _ _ hst
      obtain ⟨rhh, hrhh, hle⟩ := chkHandlers_mem env hrh _ hmem
      obtain ⟨A3, hA3, hi3⟩ := ihb f aerr rhh hrhh hi2
      exact go_on hr (LeO_trans (fun A' hA' => by cases hA'; exact hle.1 A3 hA3) (LeO_joinO_right env (some aok) rh.normal)) hi3 ihr
  | callHandledStop hx hst hmem _ _ hstop ihb =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨aok, aerr, rh, hca, hrh, rfl⟩ := chkHead_call hh
      have hi2 := (callAbs_sound hca (ExtW_inv hi hx)).2 _ _ hst
      obtain ⟨rhh, hrhh, hle⟩ := chkHandlers_mem env hrh _ hmem
      have hok := OutOk_mono hle (ihb f aerr rhh hrhh hi2)
      apply lift_head hstop hr
      exact OutOk_mono ⟨LeO_joinO_right _ _ _, LeO_refl _ _, LeO_joinO_right _ _ _⟩ hok
  | callRaised hx hst hno =>
    intro f A r hc hi
    cases f with
    | zero => simp [chk] at hc
    | succ f =>
      obtain ⟨r1, hh, hr⟩ := chk_cons_eq hc
      obtain ⟨aok, aerr, rh, hca, hrh, rfl⟩ := chkHead_call hh
      have hi2 := (callAbs_sound hca (ExtW_inv hi hx)).2 _ _ hst
      apply lift_head (by simp [Out.stops]) hr
      rw [catchesAll_false hno]
      obtain ⟨B, hB, hab⟩ := LeO_joinO_left env (some aerr) rh.raised aerr rfl
      exact ⟨B, hB, InvS_mono hab hi2⟩

theorem start_inv (env : Env) : InvS env start World.start := by
  intro i _
  exact ⟨none, List.mem_singleton.2 rfl, Or.inl ⟨rfl, rfl⟩⟩

/-- **A flow the check passes leaves no case unfinished where it ends.** In every run that ends
    where the check looks — the end of the flow, a `succeed` or a `fail`, the end of `on failure` —
    every case the run does not hand over with `leaving` is either not started, or in a final state
    of its machine, and every state the events on the other side can take it to from there is final
    too. -/
theorem chkFlow_sound {env : Env} {fuel : Nat} (hc : chkFlow env fuel = true) {leaving : List Nat} {w : World}
    (he : Ends env leaving w) : EndFine env leaving w := by
  unfold chkFlow at hc
  split at hc
  · cases hc
  · next r hr =>
    simp only [Bool.and_eq_true] at hc
    obtain ⟨hend, hfail⟩ := hc
    cases he with
    | flowEnd hx =>
      obtain ⟨A, hA, hi⟩ := exec_sound hx fuel start r hr (start_inv env)
      simp only [endOkO, hA] at hend
      exact endOk_sound hi hend
    | stopped hx => exact exec_sound hx fuel start r hr (start_inv env)
    | onFailureEnd hx hblk hx2 =>
      obtain ⟨Ar, hAr, hi⟩ := exec_sound hx fuel start r hr (start_inv env)
      rw [hAr, hblk] at hfail
      simp only at hfail
      split at hfail
      · cases hfail
      · next r2 hr2 =>
        obtain ⟨A2, hA2, hi2⟩ := exec_sound hx2 fuel Ar r2 hr2 hi
        simp only [endOkO, hA2] at hfail
        exact endOk_sound hi2 hfail
    | onFailureStopped hx hblk hx2 =>
      obtain ⟨Ar, hAr, hi⟩ := exec_sound hx fuel start r hr (start_inv env)
      rw [hAr, hblk] at hfail
      simp only at hfail
      split at hfail
      · cases hfail
      · next r2 hr2 => exact exec_sound hx2 fuel Ar r2 hr2 hi

end DandoriCore
