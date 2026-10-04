/-
  What a book's bounds guarantee (chobo's DESIGN 0.1, 2.2): no call, and no time passing, takes an
  account across a bound in the direction the bound forbids.

  A bound is checked against a pessimistic count. Taking out of an account with a lower bound is
  checked against what is posted less what is held going out (`low`); putting into an account with
  an upper bound, against what is posted plus what is held coming in (`high`). The theorems say:

  - `apply_kept`: after any call that goes through, every account with a lower bound has its `low`
    at or above the bound, or no lower than before; every account with an upper bound has its
    `high` at or below the bound, or no higher than before. An account that starts below its lower
    bound (a bound above 0 starts there) can only come up.
  - `pass_kept`: time passing only gives back what holds held, so `low` never goes down and `high`
    never goes up.
  - `within_stays`: so an account within its bounds stays within them, whatever is called.
  - `refused_keeps_balances`, `doneBefore_keeps_state`: a refusal changes no balance — every move or
    none — and a call already done changes nothing.
  - `again_doneBefore`: the same `do` or `hold` a second time is `done_before`, and changes nothing.

  `State.Good` — every hold moves amounts of at least 0 — is what `fits` lets through and every
  call keeps (`apply_kept`, `pass_kept`).
-/
import ChoboModel.Ledger

namespace ChoboModel

/-- What is left in an account counting what is held going out as gone. -/
def low (b : Bal) : Int := b.posted - b.heldOut
/-- What is in an account counting what is held coming in as there. -/
def high (b : Bal) : Int := b.posted + b.heldIn

/-- An account kept its bounds from `before` to `after`. -/
def Kept (bk : Book) (a : AccountId) (before after : Bal) : Prop :=
  (∀ l, lowerOf bk a = some l → l.value ≤ low after ∨ low before ≤ low after) ∧
  (∀ u, upperOf bk a = some u → high after ≤ u.value ∨ high after ≤ high before)

/-- Every hold moves amounts of at least 0. -/
def State.Good (s : State) : Prop := ∀ e ∈ s.holds, ∀ m ∈ e.2.moves, 0 ≤ m.amount

/-! ## Association lists -/

theorem find?_cons {α β : Type} [DecidableEq α] {k a : α} {v : β} {l : List (α × β)} :
    find? ((k, v) :: l) a = if k = a then some v else find? l a := rfl

theorem find?_append {α β : Type} [DecidableEq α] {a : α} :
    ∀ {l1 l2 : List (α × β)}, find? (l1 ++ l2) a = (find? l1 a).or (find? l2 a)
  | [], _ => rfl
  | (k, v) :: rest, l2 => by
    simp only [List.cons_append, find?_cons]
    split
    · rfl
    · exact find?_append

theorem mem_of_find? {α β : Type} [DecidableEq α] {a : α} {v : β} :
    ∀ {l : List (α × β)}, find? l a = some v → (a, v) ∈ l
  | [], h => by simp [find?] at h
  | (k, w) :: rest, h => by
    rw [find?_cons] at h
    split at h
    · next hk => cases h; subst hk; exact List.mem_cons_self
    · exact List.mem_cons_of_mem _ (mem_of_find? h)

theorem scratchBal_cons {scratch : List (AccountId × Bal)} {s : State} {k a : AccountId} {v : Bal} :
    scratchBal ((k, v) :: scratch) s a = if k = a then v else scratchBal scratch s a := by
  simp only [scratchBal, find?_cons]
  split <;> rfl

theorem bal_cons {accts : List (AccountId × Bal)} {k a : AccountId} {v : Bal} :
    bal ((k, v) :: accts) a = if k = a then v else bal accts a := by
  simp only [bal, find?_cons]
  split <;> rfl

theorem bal_append {scratch : List (AccountId × Bal)} {s : State} {a : AccountId} :
    bal (scratch ++ s.accounts) a = scratchBal scratch s a := by
  simp only [bal, scratchBal, State.balance, find?_append]
  cases find? scratch a <;> rfl

/-! ## One move -/

theorem low_takeOut (hold : Bool) (x : Bal) (n : Int) : low (takeOut hold x n) = low x - n := by
  cases hold <;> simp [takeOut, low] <;> omega

theorem high_takeOut (hold : Bool) (x : Bal) {n : Int} (hn : 0 ≤ n) : high (takeOut hold x n) ≤ high x := by
  cases hold <;> simp [takeOut, high] <;> omega

theorem low_putIn (hold : Bool) (x : Bal) {n : Int} (hn : 0 ≤ n) : low x ≤ low (putIn hold x n) := by
  cases hold <;> simp [putIn, low] <;> omega

theorem high_putIn (hold : Bool) (x : Bal) (n : Int) : high (putIn hold x n) = high x + n := by
  cases hold <;> simp [putIn, high] <;> omega

theorem lowRefuses_none {bk : Book} {x : Bal} {m : HeldMove} (h : lowRefuses bk x m = none) :
    ∀ l, lowerOf bk m.src = some l → l.value ≤ low x - m.amount := by
  intro l hl
  unfold lowRefuses at h
  rw [hl] at h
  simp only [ite_eq_right_iff, reduceCtorEq, imp_false, Int.not_lt] at h
  unfold low
  omega

theorem highRefuses_none {bk : Book} {x : Bal} {m : HeldMove} (h : highRefuses bk x m = none) :
    ∀ u, upperOf bk m.dst = some u → high x + m.amount ≤ u.value := by
  intro u hu
  unfold highRefuses at h
  rw [hu] at h
  simp only [gt_iff_lt, ite_eq_right_iff, reduceCtorEq, imp_false, Int.not_lt] at h
  unfold high
  omega

/-- **One move that passes its bounds keeps every account's bounds.** -/
theorem step_kept {bk : Book} {hold : Bool} {s : State} {scratch : List (AccountId × Bal)} {m : HeldMove}
    (ham : 0 ≤ m.amount)
    (hlo : lowRefuses bk (scratchBal scratch s m.src) m = none)
    (hhi : highRefuses bk (scratchBal scratch s m.dst) m = none)
    (hk : ∀ a, Kept bk a (s.balance a) (scratchBal scratch s a)) (a : AccountId) :
    Kept bk a (s.balance a)
      (scratchBal ((m.dst, putIn hold (scratchBal ((m.src, takeOut hold (scratchBal scratch s m.src) m.amount) :: scratch) s m.dst) m.amount) ::
        (m.src, takeOut hold (scratchBal scratch s m.src) m.amount) :: scratch) s a) := by
  generalize hs1 : (m.src, takeOut hold (scratchBal scratch s m.src) m.amount) :: scratch = scratch1
  have hx1 : scratchBal scratch1 s a =
      if m.src = a then takeOut hold (scratchBal scratch s m.src) m.amount else scratchBal scratch s a := by
    rw [← hs1]; exact scratchBal_cons
  rw [scratchBal_cons]
  have f2 : high (scratchBal scratch1 s a) ≤ high (scratchBal scratch s a) := by
    rw [hx1]
    split
    · next he => subst he; exact high_takeOut _ _ ham
    · exact Int.le_refl _
  obtain ⟨klo, khi⟩ := hk a
  constructor
  · intro l hl
    generalize hg : (if m.dst = a then putIn hold (scratchBal scratch1 s m.dst) m.amount else scratchBal scratch1 s a) = x2
    have f1 : low (scratchBal scratch1 s a) ≤ low x2 := by
      rw [← hg]
      split
      · next he => subst he; exact low_putIn _ _ ham
      · exact Int.le_refl _
    by_cases hs : m.src = a
    · subst hs
      left
      have hc := lowRefuses_none hlo l hl
      have e : low (scratchBal scratch1 s m.src) = low (scratchBal scratch s m.src) - m.amount := by
        rw [hx1]; simp only [ite_true]; exact low_takeOut _ _ _
      omega
    · have e : scratchBal scratch1 s a = scratchBal scratch s a := by rw [hx1]; simp [hs]
      rw [e] at f1
      rcases klo l hl with h1 | h1
      · left; omega
      · right; omega
  · intro u hu
    by_cases hd : m.dst = a
    · subst hd
      left
      simp only [ite_true]
      rw [high_putIn]
      have hc := highRefuses_none hhi u hu
      omega
    · simp only [hd, ite_false]
      rcases khi u hu with h1 | h1
      · left; omega
      · right; omega

/-- **The moves of a `do` or a `hold`, one after another.** -/
theorem runMoves_kept {bk : Book} {hold : Bool} {s : State} :
    ∀ (ms : List HeldMove) (scratch out : List (AccountId × Bal)),
      (∀ m ∈ ms, 0 ≤ m.amount) →
      runMoves bk hold s ms scratch = .moved out →
      (∀ a, Kept bk a (s.balance a) (scratchBal scratch s a)) →
      ∀ a, Kept bk a (s.balance a) (scratchBal out s a)
  | [], scratch, out, _, h, hk => by
    simp only [runMoves, Moved.moved.injEq] at h
    subst h; exact hk
  | m :: ms, scratch, out, hm, h, hk => by
    simp only [runMoves] at h
    cases hlo : lowRefuses bk (scratchBal scratch s m.src) m with
    | some r => rw [hlo] at h; cases h
    | none =>
      rw [hlo] at h
      simp only at h
      cases hhi : highRefuses bk (scratchBal scratch s m.dst) m with
      | some r => rw [hhi] at h; cases h
      | none =>
        rw [hhi] at h
        simp only at h
        exact runMoves_kept ms _ out (fun x hx => hm x (List.mem_cons_of_mem _ hx)) h
          (step_kept (hm m List.mem_cons_self) hlo hhi hk)

/-! ## A hold ending -/

/-- **Settling what a hold holds** — posting part or all of it, voiding it, letting it expire —
    never takes `low` down nor `high` up, as long as no move posts more than it held. -/
theorem settle_monotone :
    ∀ (L : List (HeldMove × Int)) (accts : List (AccountId × Bal)),
      (∀ x ∈ L, 0 ≤ x.2 ∧ x.2 ≤ x.1.amount) →
      ∀ a, low (bal accts a) ≤ low (bal (settle L accts) a) ∧ high (bal (settle L accts) a) ≤ high (bal accts a)
  | [], accts, _, a => by simp only [settle]; exact ⟨Int.le_refl _, Int.le_refl _⟩
  | (m, p) :: rest, accts, hL, a => by
    obtain ⟨hp0, hpm⟩ := hL (m, p) List.mem_cons_self
    simp only at hp0 hpm
    simp only [settle]
    generalize ha1 : (m.src, settleOut m p (bal accts m.src)) :: accts = a1
    generalize ha2 : (m.dst, settleIn m p (bal a1 m.dst)) :: a1 = a2
    obtain ⟨ih1, ih2⟩ := settle_monotone rest a2 (fun x hx => hL x (List.mem_cons_of_mem _ hx)) a
    have e1 : bal a1 a = if m.src = a then settleOut m p (bal accts m.src) else bal accts a := by
      rw [← ha1]; exact bal_cons
    have e2 : bal a2 a = if m.dst = a then settleIn m p (bal a1 m.dst) else bal a1 a := by
      rw [← ha2]; exact bal_cons
    have s1 : low (bal accts a) ≤ low (bal a1 a) ∧ high (bal a1 a) ≤ high (bal accts a) := by
      rw [e1]
      split
      · next he => subst he; simp only [low, high, settleOut]; constructor <;> omega
      · exact ⟨Int.le_refl _, Int.le_refl _⟩
    have s2 : low (bal a1 a) ≤ low (bal a2 a) ∧ high (bal a2 a) ≤ high (bal a1 a) := by
      rw [e2]
      split
      · next he => subst he; simp only [low, high, settleIn]; constructor <;> omega
      · exact ⟨Int.le_refl _, Int.le_refl _⟩
    exact ⟨Int.le_trans (Int.le_trans s1.1 s2.1) ih1, Int.le_trans ih2 (Int.le_trans s2.2 s1.2)⟩

theorem release_fits {h : Hold} (hm : ∀ m ∈ h.moves, 0 ≤ m.amount) :
    ∀ x ∈ release h, 0 ≤ x.2 ∧ x.2 ≤ x.1.amount := by
  intro x hx
  simp only [release, List.mem_map] at hx
  obtain ⟨m, hm', rfl⟩ := hx
  exact ⟨Int.le_refl _, hm m hm'⟩

/-! ## What a call lets through -/

theorem kept_refl (bk : Book) (a : AccountId) (x : Bal) : Kept bk a x x :=
  ⟨fun _ _ => Or.inr (Int.le_refl _), fun _ _ => Or.inr (Int.le_refl _)⟩

theorem kept_of_monotone {bk : Book} {a : AccountId} {x y : Bal} (h1 : low x ≤ low y) (h2 : high y ≤ high x) :
    Kept bk a x y :=
  ⟨fun _ _ => Or.inr h1, fun _ _ => Or.inr h2⟩

theorem fits_moves {t : TransferKind} {c : Call} (h : fits t c = true) : ∀ m ∈ c.moves t, 0 ≤ m.amount := by
  unfold fits at h
  simp only [Bool.and_eq_true] at h
  intro m hm
  simp only [Call.moves, List.mem_map] at hm
  obtain ⟨mv, hmv, rfl⟩ := hm
  have := List.all_eq_true.1 h.2 mv hmv
  simpa using this

theorem fits_amounts {t : TransferKind} {c : Call} {a : List (Nat × Int)} (h : fits t c = true) (ha : c.amounts = some a) :
    ∀ p ∈ a, 0 ≤ p.2 := by
  unfold fits at h
  simp only [Bool.and_eq_true] at h
  have h2 := h.1.2
  rw [ha] at h2
  simp only [Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at h2
  intro p hp
  exact (h2.2 p hp).1

theorem fits_lit {t : TransferKind} {c : Call} (h : fits t c = true) {mv : Move} (hmv : mv ∈ t.moves) :
    0 ≤ c.amount mv := by
  unfold fits at h
  simp only [Bool.and_eq_true] at h
  have := List.all_eq_true.1 h.2 mv hmv
  simpa using this

/-- What a `post` posts is at least 0 on every move. -/
theorem postAmounts_nonneg {t : TransferKind} {c : Call} {h : Hold} (hf : fits t c = true)
    (hm : ∀ m ∈ h.moves, 0 ≤ m.amount) : ∀ p ∈ postAmounts t c h, 0 ≤ p := by
  unfold postAmounts
  cases ha : c.amounts with
  | none =>
    intro p hp
    simp only [List.mem_map] at hp
    obtain ⟨m, hm', rfl⟩ := hp
    exact hm m hm'
  | some a =>
    intro p hp
    simp only [List.mem_map] at hp
    obtain ⟨mv, hmv, rfl⟩ := hp
    cases hamt : mv.amount with
    | param i =>
      simp only
      cases hfi : find? a i with
      | none => simp
      | some v =>
        simp only [Option.getD_some]
        exact fits_amounts hf ha (i, v) (mem_of_find? hfi)
    | lit v =>
      simp only
      have h0 := fits_lit hf hmv
      simp only [Call.amount, hamt] at h0
      exact h0

theorem overHold_false {amounts : List Int} {moves : List HeldMove} (h : overHold amounts moves = false) :
    ∀ x ∈ moves.zip amounts, x.2 ≤ x.1.amount := by
  intro x hx
  unfold overHold at h
  have := List.any_eq_false.1 h _ hx
  simp only [gt_iff_lt, decide_eq_true_eq, Int.not_lt] at this
  exact this

theorem zip_nonneg {moves : List HeldMove} {amounts : List Int} (ha : ∀ p ∈ amounts, 0 ≤ p) :
    ∀ x ∈ moves.zip amounts, 0 ≤ x.2 := by
  intro x hx
  exact ha x.2 (List.of_mem_zip hx).2

/-! ## The theorems -/

/-- **A `post` keeps the bounds**, and the holds good. -/
theorem post_kept {bk : Book} {s s' : State} {t : TransferKind} {c : Call} {o : Outcome}
    (hg : s.Good) (hf : fits t c = true) (h : s.post t c = (o, s')) :
    s'.Good ∧ ∀ a, Kept bk a (s.balance a) (s'.balance a) := by
  unfold State.post at h
  simp only at h
  split at h
  · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
  · next hd hh =>
    have hm : ∀ m ∈ hd.moves, 0 ≤ m.amount := hg _ (mem_of_find? hh)
    split at h
    · split at h
      · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
      · split at h
        · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
        · next hover =>
          cases h
          have hover' : overHold (postAmounts t c hd) hd.moves = false := by simpa using hover
          refine ⟨?_, fun a => ?_⟩
          · intro e he
            simp only [List.mem_cons] at he
            rcases he with rfl | he
            · exact hm
            · exact hg e he
          · have := settle_monotone (hd.moves.zip (postAmounts t c hd)) s.accounts
              (fun x hx => ⟨zip_nonneg (postAmounts_nonneg hf hm) x hx, overHold_false hover' x hx⟩) a
            exact kept_of_monotone this.1 this.2
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩

/-- **A `void` keeps the bounds**, and the holds good. -/
theorem void_kept {bk : Book} {s s' : State} {t : TransferKind} {c : Call} {o : Outcome}
    (hg : s.Good) (h : s.void t c = (o, s')) :
    s'.Good ∧ ∀ a, Kept bk a (s.balance a) (s'.balance a) := by
  unfold State.void at h
  simp only at h
  split at h
  · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
  · next hd hh =>
    have hm : ∀ m ∈ hd.moves, 0 ≤ m.amount := hg _ (mem_of_find? hh)
    split at h
    · split at h
      · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
      · cases h
        refine ⟨?_, fun a => ?_⟩
        · intro e he
          simp only [List.mem_cons] at he
          rcases he with rfl | he
          · exact hm
          · exact hg e he
        · have := settle_monotone (release hd) s.accounts (release_fits hm) a
          exact kept_of_monotone this.1 this.2
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩

/-- **A `do` or a `hold` keeps the bounds**, and the holds good. -/
theorem moveOrHold_kept {bk : Book} {s s' : State} {t : TransferKind} {c : Call} {o : Outcome}
    (hg : s.Good) (hf : fits t c = true) (h : s.moveOrHold bk t c = .ok (o, s')) :
    s'.Good ∧ ∀ a, Kept bk a (s.balance a) (s'.balance a) := by
  unfold State.moveOrHold at h
  simp only at h
  split at h
  · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
  · split at h
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
    · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
    · split at h
      · cases h; exact ⟨hg, fun a => kept_refl bk a _⟩
      · next scratch hrun =>
        split at h
        · cases h
          have hk := runMoves_kept (c.moves t) [] scratch (fits_moves hf) hrun
            (fun a => by simp only [scratchBal, find?, Option.getD_none]; exact kept_refl bk a _)
          by_cases hh : (c.op == Op.hold) = true
          · rw [if_pos hh]
            refine ⟨?_, fun a => ?_⟩
            · intro e he
              simp only [List.mem_cons] at he
              rcases he with rfl | he
              · exact fits_moves hf
              · exact hg e he
            · show Kept bk a (s.balance a) (bal (scratch ++ s.accounts) a)
              rw [bal_append]; exact hk a
          · rw [if_neg hh]
            refine ⟨hg, fun a => ?_⟩
            show Kept bk a (s.balance a) (bal (scratch ++ s.accounts) a)
            rw [bal_append]; exact hk a
        · cases h

/-- **No call takes an account across a bound in the direction the bound forbids.** -/
theorem apply_kept {bk : Book} {s s' : State} {c : Call} {o : Outcome} (hg : s.Good)
    (h : s.apply bk c = .ok (o, s')) : s'.Good ∧ ∀ a, Kept bk a (s.balance a) (s'.balance a) := by
  unfold State.apply at h
  split at h
  · cases h
  · split at h
    · next hf =>
      split at h
      · exact moveOrHold_kept hg hf h
      · exact moveOrHold_kept hg hf h
      · simp only [Except.ok.injEq] at h; exact post_kept hg hf h
      · simp only [Except.ok.injEq] at h; exact void_kept hg h
    · cases h

theorem mem_latest {e : HoldKey × Hold} : ∀ {l : List (HoldKey × Hold)}, e ∈ latest l → e ∈ l
  | [], h => by simp [latest] at h
  | (k, v) :: rest, h => by
    simp only [latest, List.mem_cons, List.mem_filter] at h
    rcases h with h | ⟨h, _⟩
    · exact h ▸ List.mem_cons_self
    · exact List.mem_cons_of_mem _ (mem_latest h)

theorem foldl_expire (s : State) (hg : s.Good) :
    ∀ (L : List (HoldKey × Hold)) (st : State), (∀ e ∈ L, e ∈ s.holds) → st.Good →
      (∀ a, low (s.balance a) ≤ low (st.balance a) ∧ high (st.balance a) ≤ high (s.balance a)) →
      (L.foldl State.expire st).Good ∧
      ∀ a, low (s.balance a) ≤ low ((L.foldl State.expire st).balance a) ∧
        high ((L.foldl State.expire st).balance a) ≤ high (s.balance a)
  | [], st, _, hgst, hst => ⟨hgst, hst⟩
  | e :: rest, st, hL, hgst, hst => by
    simp only [List.foldl_cons]
    have hm : ∀ m ∈ e.2.moves, 0 ≤ m.amount := hg e (hL e List.mem_cons_self)
    apply foldl_expire s hg rest _ (fun x hx => hL x (List.mem_cons_of_mem _ hx))
    · intro x hx
      simp only [State.expire, List.mem_cons] at hx
      rcases hx with rfl | hx
      · exact hm
      · exact hgst x hx
    · intro a
      have := settle_monotone (release e.2) st.accounts (release_fits hm) a
      obtain ⟨h1, h2⟩ := hst a
      exact ⟨Int.le_trans h1 this.1, Int.le_trans this.2 h2⟩

/-- **Time passing only gives back.** -/
theorem pass_kept {s : State} (hg : s.Good) (secs : Nat) :
    (s.pass secs).Good ∧ ∀ a, low (s.balance a) ≤ low ((s.pass secs).balance a) ∧
      high ((s.pass secs).balance a) ≤ high (s.balance a) := by
  unfold State.pass
  simp only
  refine foldl_expire s hg _ { s with now := s.now + secs } ?_ hg (fun a => ⟨Int.le_refl _, Int.le_refl _⟩)
  intro e he
  exact mem_latest (List.mem_filter.1 he).1

/-- **Within its bounds stays within them**: a lower bound an account meets, it meets after any
    call; so does an upper bound. -/
theorem within_stays {bk : Book} {s s' : State} {c : Call} {o : Outcome} (hg : s.Good)
    (h : s.apply bk c = .ok (o, s')) (a : AccountId) :
    (∀ l, lowerOf bk a = some l → l.value ≤ low (s.balance a) → l.value ≤ low (s'.balance a)) ∧
    (∀ u, upperOf bk a = some u → high (s.balance a) ≤ u.value → high (s'.balance a) ≤ u.value) := by
  obtain ⟨klo, khi⟩ := (apply_kept hg h).2 a
  constructor
  · intro l hl hw
    rcases klo l hl with h1 | h1
    · exact h1
    · exact Int.le_trans hw h1
  · intro u hu hw
    rcases khi u hu with h1 | h1
    · exact h1
    · exact Int.le_trans h1 hw

/-! ## Every move or none, and the key -/

/-- What a `do` or a `hold` can come to: done, or the balances and the holds as they were — and
    for `done_before`, the whole state as it was. -/
theorem moveOrHold_shape {bk : Book} {s s' : State} {t : TransferKind} {c : Call} {o : Outcome}
    (h : s.moveOrHold bk t c = .ok (o, s')) :
    o = .done ∨ (s'.accounts = s.accounts ∧ s'.holds = s.holds ∧ (o = .doneBefore → s' = s)) := by
  unfold State.moveOrHold at h
  simp only at h
  split at h
  · simp only [Except.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨rfl, rfl⟩ := h
    exact Or.inr ⟨rfl, rfl, fun e => by cases e⟩
  · split at h
    · simp only [Except.ok.injEq, Prod.mk.injEq] at h
      obtain ⟨rfl, rfl⟩ := h
      exact Or.inr ⟨rfl, rfl, fun e => by cases e⟩
    · simp only [Except.ok.injEq, Prod.mk.injEq] at h
      obtain ⟨_, rfl⟩ := h
      exact Or.inr ⟨rfl, rfl, fun _ => rfl⟩
    · split at h
      · simp only [Except.ok.injEq, Prod.mk.injEq] at h
        obtain ⟨rfl, rfl⟩ := h
        exact Or.inr ⟨rfl, rfl, fun e => by cases e⟩
      · split at h
        · simp only [Except.ok.injEq, Prod.mk.injEq] at h
          exact Or.inl h.1.symm
        · cases h

/-- What a `post` can come to: done, or the state as it was. -/
theorem post_shape {s s' : State} {t : TransferKind} {c : Call} {o : Outcome} (h : s.post t c = (o, s')) :
    o = .done ∨ s' = s := by
  unfold State.post at h
  simp only at h
  split at h
  · cases h; exact Or.inr rfl
  · split at h
    · split at h
      · cases h; exact Or.inr rfl
      · split at h
        · cases h; exact Or.inr rfl
        · cases h; exact Or.inl rfl
    · cases h; exact Or.inr rfl
    · cases h; exact Or.inr rfl
    · cases h; exact Or.inr rfl

/-- What a `void` can come to: done, or the state as it was. -/
theorem void_shape {s s' : State} {t : TransferKind} {c : Call} {o : Outcome} (h : s.void t c = (o, s')) :
    o = .done ∨ s' = s := by
  unfold State.void at h
  simp only at h
  split at h
  · cases h; exact Or.inr rfl
  · split at h
    · split at h
      · cases h; exact Or.inr rfl
      · cases h; exact Or.inl rfl
    · cases h; exact Or.inr rfl
    · cases h; exact Or.inr rfl
    · cases h; exact Or.inr rfl

/-- **A refusal changes no balance and no hold**: every move, or none. -/
theorem refused_keeps_balances {bk : Book} {s s' : State} {c : Call} {r : String}
    (h : s.apply bk c = .ok (.refused r, s')) : s'.accounts = s.accounts ∧ s'.holds = s.holds := by
  unfold State.apply at h
  split at h
  · cases h
  · split at h
    · split at h
      · rcases moveOrHold_shape h with e | ⟨h1, h2, _⟩
        · cases e
        · exact ⟨h1, h2⟩
      · rcases moveOrHold_shape h with e | ⟨h1, h2, _⟩
        · cases e
        · exact ⟨h1, h2⟩
      · simp only [Except.ok.injEq] at h
        rcases post_shape h with e | e
        · cases e
        · subst e; exact ⟨rfl, rfl⟩
      · simp only [Except.ok.injEq] at h
        rcases void_shape h with e | e
        · cases e
        · subst e; exact ⟨rfl, rfl⟩
    · cases h

/-- **A call already done changes nothing.** -/
theorem doneBefore_keeps_state {bk : Book} {s s' : State} {c : Call}
    (h : s.apply bk c = .ok (.doneBefore, s')) : s' = s := by
  unfold State.apply at h
  split at h
  · cases h
  · split at h
    · split at h
      · rcases moveOrHold_shape h with e | ⟨_, _, h3⟩
        · cases e
        · exact h3 rfl
      · rcases moveOrHold_shape h with e | ⟨_, _, h3⟩
        · cases e
        · exact h3 rfl
      · simp only [Except.ok.injEq] at h
        rcases post_shape h with e | e
        · cases e
        · exact e
      · simp only [Except.ok.injEq] at h
        rcases void_shape h with e | e
        · cases e
        · exact e
    · cases h

theorem moveOrHold_again {bk : Book} {s s1 : State} {t : TransferKind} {c : Call}
    (h : s.moveOrHold bk t c = .ok (.done, s1)) : s1.moveOrHold bk t c = .ok (.doneBefore, s1) := by
  have key : find? s1.keys (c.kind, c.op, c.key t) = some (.done c.args) ∧
      ¬ ((c.moves t).any (fun m => decide (m.src = m.dst)) = true) := by
    unfold State.moveOrHold at h
    simp only at h
    split at h
    · simp at h
    · next hsame =>
      split at h
      · simp at h
      · split at h <;> simp at h
      · split at h
        · simp at h
        · split at h
          · simp only [Except.ok.injEq, Prod.mk.injEq, true_and] at h
            subst h
            exact ⟨by split <;> simp [find?_cons], hsame⟩
          · cases h
  unfold State.moveOrHold
  simp only
  rw [if_neg key.2, key.1]
  simp

/-- **The same `do` or `hold` a second time is `done_before`**, and changes nothing: what a retry
    of a call that went through comes to. -/
theorem again_doneBefore {bk : Book} {s s1 : State} {c : Call}
    (hop : c.op = .«do» ∨ c.op = .hold) (h : s.apply bk c = .ok (.done, s1)) :
    s1.apply bk c = .ok (.doneBefore, s1) := by
  unfold State.apply at h ⊢
  split at h
  · cases h
  · next t ht =>
    split at h
    · next hf =>
      rw [if_pos hf]
      rcases hop with hop | hop
      · rw [hop] at h ⊢; exact moveOrHold_again h
      · rw [hop] at h ⊢; exact moveOrHold_again h
    · cases h

end ChoboModel
