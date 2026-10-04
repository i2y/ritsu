/-
  The certificate's checks imply the rule's claims (DESIGN §15.97).

  Each theorem here has the same shape: *if* the corresponding function in `RulecCert.Check`
  returns `true` on what the certificate carries, *then* the matching `Prop` in
  `RulecCert.Semantics` holds of the table. Nothing is assumed about how the certificate was
  produced — `rulec` does not appear in the statements — so the theorems say what a reader
  of the document may conclude from it, and no more.

  What each theorem needs beyond the check is written as a hypothesis, and there are only
  three kinds: the rows are shaped like the axes, a box the cover calls impossible really is
  unreachable (`RulecCert.Sieve` discharges that), and the pairs the tool could not settle
  are named. The last one is why `undecided` exists in the document at all.
-/
import RulecCert.Check

namespace RulecCert

/-! ## Two rows never fire at once -/

/-- A list whose elements pairwise cannot both pass `P` has at most one that does. -/
theorem length_filter_le_one {α : Type _} {P : α → Bool} :
    ∀ {l : List α}, l.Pairwise (fun a b => ¬(P a = true ∧ P b = true)) →
      (l.filter P).length ≤ 1
  | [], _ => by simp
  | a :: l, h => by
    rw [List.pairwise_cons] at h
    by_cases ha : P a = true
    · have : l.filter P = [] := by
        refine List.filter_eq_nil_iff.2 ?_
        intro b hb hpb
        exact absurd ⟨ha, hpb⟩ (h.1 b hb)
      simp [ha, this]
    · simp only [List.filter_cons, ha, Bool.false_eq_true, ite_false]
      exact length_filter_le_one h.2

/-- A pair proved apart on an axis cannot both take one point. -/
theorem not_both_of_partsOn {a b : Box} {axis : Nat} {p : Point}
    (h : partsOn a b axis = true) (ha : inBox a p = true) (hb : inBox b p = true) : False := by
  unfold partsOn at h
  cases hx : a[axis]? with
  | none => rw [hx] at h; simp at h
  | some xs =>
    cases hy : b[axis]? with
    | none => rw [hx, hy] at h; simp at h
    | some ys =>
      rw [hx, hy] at h
      obtain ⟨c, hc, hcx⟩ := inBox_getElem ha hx
      obtain ⟨d, hd, hdy⟩ := inBox_getElem hb hy
      have : c = d := by rw [hc] at hd; exact Option.some.inj hd
      subst this
      have := List.all_eq_true.1 h c (by simpa using hcx)
      exact absurd (by simpa using hdy) (by simpa using this)

/-- **E105 is settled by the pairs.** With every pair either proved apart or named as one
    the tool could not settle — and here there are none of those — no point is taken by two
    rows at once. -/
theorem disjoint_of_pairsPart {rows : List Row} {told : Nat → Nat → Option Nat}
    {undecided : Nat → Nat → Bool} (hshape : rowsDistinct rows = true)
    (hnone : ∀ a b, undecided a b = false)
    (h : pairsPart rows told undecided = true) :
    ∀ p, ((rows.filter (fun r => inBox r.box p)).length ≤ 1) := by
  intro p
  refine length_filter_le_one ?_
  have hdis : rows.Pairwise (fun r q => r.index ≠ q.index) := of_decide_eq_true hshape
  have hall := List.all_eq_true.1 h
  refine hdis.imp_of_mem ?_
  intro r q hr hq hne
  rintro ⟨hrp, hqp⟩
  -- One of the two is the earlier; the pair it makes is the one the certificate names.
  have key : ∀ x y : Row, x ∈ rows → y ∈ rows → x.index < y.index →
      inBox x.box p = true → inBox y.box p = true → False := by
    intro x y hx hy hlt hxp hyp
    have hxy := List.all_eq_true.1 (hall x hx) y hy
    rw [ite_eq_left_of_eq_true _ _ (by simp [hlt])] at hxy
    cases hax : told x.index y.index with
    | none => rw [hax] at hxy; rw [hnone] at hxy; exact absurd hxy (by simp)
    | some axis => rw [hax] at hxy; exact not_both_of_partsOn hxy hxp hyp
  rcases Nat.lt_or_ge r.index q.index with hlt | hge
  · exact key r q hr hq hlt hrp hqp
  · exact key q r hq hr (Nat.lt_of_le_of_ne hge (Ne.symm hne)) hqp hrp

/-! ## Every row answers somewhere -/

/-- **E102 is settled by the witnesses.** Each row is handed a point the rule really is
    asked about, inside that row's box and — under `first` — ahead of no earlier row. -/
theorem holdsFor_of_reachOk {t : Table} {claimed : List Row} {P : Point → Prop}
    {askedOk : Point → List Rat → Bool} {witness : Nat → Option (Point × List Rat)}
    (hp : ∀ p v, askedOk p v = true → P p)
    (h : reachOk t.arities claimed t.rows t.policy askedOk witness = true) :
    t.holdsFor P claimed := by
  intro r hr
  have hrow := List.all_eq_true.1 h r hr
  cases hw : witness r.index with
  | none => rw [hw] at hrow; simp at hrow
  | some pv =>
    obtain ⟨p, v⟩ := pv
    rw [hw] at hrow
    simp only [Bool.and_eq_true] at hrow
    obtain ⟨⟨⟨hs, hpo⟩, hb⟩, hfirst⟩ := hrow
    refine ⟨p, hs, hp p v hpo, hb, ?_⟩
    intro hpol q hq hlt
    rw [hpol] at hfirst
    simp only [bne_self_eq_false, Bool.false_or] at hfirst
    have := List.all_eq_true.1 hfirst q hq
    simp only [Bool.or_eq_true, decide_eq_true_eq, Bool.not_eq_true'] at this
    rcases this with hle | hnb
    · omega
    · exact hnb

/-! ## The cover tiles the space

The walk is structural but the tree is not a list, so the induction below runs on a size
bound. These four lemmas are what lets it step from a node to the child the point picks. -/

theorem Kids.get?_lt_size : ∀ {ks : Kids} {c : Nat} {k : Cover},
    ks.get? c = some k → k.size < ks.size
  | .nil, _, _, hg => by simp [Kids.get?] at hg
  | .cons k' ks, 0, k, hg => by
      simp only [Kids.get?, Option.some.injEq] at hg
      subst hg; simp only [Kids.size]; omega
  | .cons k' ks, c + 1, k, hg => by
      simp only [Kids.get?] at hg
      have := Kids.get?_lt_size hg
      simp only [Kids.size]; omega

theorem Kids.leans_get : ∀ {ks : Kids} {c : Nat} {k : Cover},
    ks.get? c = some k → ks.leansOnUpstream = false → k.leansOnUpstream = false
  | .nil, _, _, hg, _ => by simp [Kids.get?] at hg
  | .cons k' ks, 0, k, hg, hu => by
      simp only [Kids.get?, Option.some.injEq] at hg
      subst hg
      simp only [Kids.leansOnUpstream, Bool.or_eq_false_iff] at hu
      exact hu.1
  | .cons k' ks, c + 1, k, hg, hu => by
      simp only [Kids.get?] at hg
      simp only [Kids.leansOnUpstream, Bool.or_eq_false_iff] at hu
      exact Kids.leans_get hg hu.2

theorem kidsOk_get {arities : List Arity} {rows : List Row} {ruledOut : Point → Bool} :
    ∀ (ks : Kids) (path : Point) (base left c : Nat),
      kidsOk arities rows ruledOut ks path base left = true → c < left →
        ∃ k, ks.get? c = some k ∧
          coverOk arities rows ruledOut k (path ++ [base + c]) = true
  | .nil, _, _, left, c, hok, hc => by
      simp only [kidsOk, beq_iff_eq] at hok; omega
  | .cons k' ks, path, base, left, 0, hok, _ => by
      simp only [kidsOk, Bool.and_eq_true] at hok
      exact ⟨k', rfl, by simpa using hok.1.2⟩
  | .cons k' ks, path, base, left, c + 1, hok, hc => by
      simp only [kidsOk, Bool.and_eq_true, bne_iff_ne, ne_eq] at hok
      obtain ⟨k, hg, hco⟩ := kidsOk_get ks path (base + 1) (left - 1) c hok.2 (by omega)
      refine ⟨k, hg, ?_⟩
      have e : base + 1 + c = base + (c + 1) := by omega
      rwa [e] at hco

/-- What one axis of a path-taking box says. -/
theorem takesPath_get {box : Box} {path : Point} (h : takesPath box path = true)
    {i : Nat} (hi : i < path.length) :
    ∃ xs c, box[i]? = some xs ∧ path[i]? = some c ∧ xs.contains c = true := by
  have hb := List.all_eq_true.1 h i (List.mem_range.2 hi)
  revert hb
  cases hbx : box[i]? with
  | none => intro hb; simp at hb
  | some xs =>
    cases hpx : path[i]? with
    | none => intro hb; simp at hb
    | some c => intro hb; exact ⟨xs, c, rfl, rfl, by simpa using hb⟩

/-- What one axis below the path says: the box takes every coordinate the axis has. -/
theorem spansRest_get {box : Box} {arities : List Arity} {depth i : Nat}
    (h : spansRest box arities depth = true) (hd : depth ≤ i) (hi : i < arities.length) :
    ∃ xs n, box[i]? = some xs ∧ arities[i]? = some n ∧ ∀ c, c < n → xs.contains c = true := by
  have hmem : (i - depth) ∈ List.range (arities.length - depth) := List.mem_range.2 (by omega)
  have hb := List.all_eq_true.1 h _ hmem
  have e : depth + (i - depth) = i := by omega
  rw [e] at hb
  revert hb
  cases hbx : box[i]? with
  | none => intro hb; simp at hb
  | some xs =>
    cases hax : arities[i]? with
    | none => intro hb; simp at hb
    | some n =>
      intro hb
      refine ⟨xs, n, rfl, rfl, ?_⟩
      intro c hc
      exact List.all_eq_true.1 (by simpa using hb) c (List.mem_range.2 hc)

/-- A prefix reads the same as the list it is a prefix of. -/
theorem prefix_getElem? {path p : Point} (h : path <+: p) {i : Nat} (hi : i < path.length) :
    p[i]? = path[i]? := by
  obtain ⟨tl, rfl⟩ := h
  simp [List.getElem?_append_left hi]

/-- One more coordinate, read off the point itself, extends the prefix. -/
theorem prefix_extend {path p : Point} {c : Nat} (h : path <+: p)
    (hc : p[path.length]? = some c) : (path ++ [c]) <+: p := by
  obtain ⟨tl, rfl⟩ := h
  cases tl with
  | nil => rw [List.append_nil] at hc; simp at hc
  | cons d ds =>
    have : d = c := by simpa using hc
    subst this
    exact ⟨ds, by simp⟩

/-- **The walk of the cover, by induction on its size.** Every point the rule is asked
    about arrives at a `row` leaf, because the other two leaves cannot hold it: an
    `impossible` leaf would say the point is never asked about, and an `upstream` leaf is
    excluded by hypothesis. -/
theorem coverOk_sound {t : Table} {ruledOut : Point → Bool}
    (hshape : rowsShaped t.arities t.rows = true)
    (hr : ∀ path p, ruledOut path = true → path <+: p → inSpace t.arities p = true →
      ¬ t.asked p) :
    ∀ (n : Nat) (cv : Cover) (path p : Point), cv.size ≤ n →
      coverOk t.arities t.rows ruledOut cv path = true →
      path <+: p → inSpace t.arities p = true → t.asked p →
      ∃ r ∈ t.rows, inBox r.box p = true := by
  intro n
  induction n with
  | zero =>
    intro cv _ _ hsz _ _ _ _
    cases cv <;> simp [Cover.size] at hsz
  | succ n ih =>
    intro cv path p hsz hok hpre hsp hask
    cases cv with
    | upstream => simp [coverOk] at hok
    | impossible =>
      exact absurd hask (hr path p (by simpa [coverOk] using hok) hpre hsp)
    | row i =>
      simp only [coverOk] at hok
      revert hok
      cases hf : t.rows.find? (fun r => r.index == i) with
      | none => intro hok; simp at hok
      | some r =>
        intro hok
        simp only [Bool.and_eq_true] at hok
        obtain ⟨htp, hsr⟩ := hok
        have hmem : r ∈ t.rows := List.mem_of_find?_eq_some hf
        refine ⟨r, hmem, ?_⟩
        have hplen : p.length = t.arities.length := inSpace_length hsp
        have hblen : r.box.length = t.arities.length := by
          have := List.all_eq_true.1 hshape r hmem
          simpa using this
        refine inBox_of_getElem (by rw [hblen, hplen]) ?_
        intro i' xs c' hbx hpx
        by_cases hlt : i' < path.length
        · obtain ⟨ys, d, hby, hpd, hyd⟩ := takesPath_get htp hlt
          have : p[i']? = some d := by rw [prefix_getElem? hpre hlt]; exact hpd
          have hc : c' = d := by rw [this] at hpx; exact Option.some.inj hpx.symm
          have hx : xs = ys := by rw [hby] at hbx; exact Option.some.inj hbx.symm
          subst hc; subst hx; exact hyd
        · have hi' : i' < t.arities.length := by
            have : i' < p.length := by
              rcases Nat.lt_or_ge i' p.length with h' | h'
              · exact h'
              · rw [List.getElem?_eq_none h'] at hpx; simp at hpx
            omega
          obtain ⟨ys, m, hby, ham, hall⟩ := spansRest_get hsr (by omega) hi'
          obtain ⟨d, hpd, hdm⟩ := inSpace_getElem hsp ham
          have hc : c' = d := by rw [hpd] at hpx; exact Option.some.inj hpx.symm
          have hx : xs = ys := by rw [hby] at hbx; exact Option.some.inj hbx.symm
          subst hc; subst hx; exact hall _ hdm
    | split kids =>
      simp only [coverOk] at hok
      revert hok
      cases ha : t.arities[path.length]? with
      | none => intro hok; simp at hok
      | some m =>
        intro hok
        obtain ⟨c, hpc, hcm⟩ := inSpace_getElem hsp ha
        obtain ⟨k, hk, hchild⟩ := kidsOk_get kids path 0 m c hok hcm
        rw [Nat.zero_add] at hchild
        have hksz : k.size < kids.size := Kids.get?_lt_size hk
        have : Cover.size (.split kids) = kids.size + 1 := rfl
        exact ih k (path ++ [c]) p (by omega) hchild (prefix_extend hpre hpc) hsp hask

/-- **E101 is settled by the cover.** -/
theorem complete_of_coverOk {t : Table} {ruledOut : Point → Bool} {tree : Cover}
    (hshape : rowsShaped t.arities t.rows = true)
    (hr : ∀ path p, ruledOut path = true → path <+: p → inSpace t.arities p = true →
      ¬ t.asked p)
    (h : coverOk t.arities t.rows ruledOut tree [] = true) : t.completeHolds := by
  intro p hsp hask
  obtain ⟨r, hmem, hb⟩ :=
    coverOk_sound hshape hr tree.size tree [] p (Nat.le_refl _) h List.nil_prefix hsp hask
  exact List.ne_nil_of_mem (List.mem_filter.2 ⟨hmem, hb⟩)

/-! ## The two together

`unique` claims one row, not "at least one" and not "at most one". The certificate states
the two halves separately because they fail separately — a gap is E101 and an overlap is
E105 — and this is where they are put back together. -/

theorem unique_of_checks {t : Table} {ruledOut : Point → Bool} {tree : Cover}
    {told : Nat → Nat → Option Nat}
    (hshape : rowsShaped t.arities t.rows = true)
    (hdist : rowsDistinct t.rows = true)
    (hr : ∀ path p, ruledOut path = true → path <+: p → inSpace t.arities p = true →
      ¬ t.asked p)
    (hcover : coverOk t.arities t.rows ruledOut tree [] = true)
    (hpairs : pairsPart t.rows told (fun _ _ => false) = true) : t.uniqueHolds := by
  intro p hsp hask
  have h1 : t.firing p ≠ [] := complete_of_coverOk hshape hr hcover p hsp hask
  have h2 : (t.firing p).length ≤ 1 :=
    disjoint_of_pairsPart hdist (fun _ _ => rfl) hpairs p
  have h3 : (t.firing p).length ≠ 0 := fun h => h1 (List.eq_nil_of_length_eq_zero h)
  omega

/-- The strong reading: the values behind each point are there and check out. -/
theorem reached_of_reachOk {t : Table} {claimed : List Row}
    {askedOk : Point → List Rat → Bool} {witness : Nat → Option (Point × List Rat)}
    (hp : ∀ p v, askedOk p v = true → t.asked p)
    (h : reachOk t.arities claimed t.rows t.policy askedOk witness = true) :
    t.reachedHoldsFor claimed := holdsFor_of_reachOk hp h

/-- The weak reading: the point is one the sieve does not exclude, and no more is claimed. -/
theorem notExcluded_of_reachOk {t : Table} {claimed : List Row} {excluded : Point → Bool}
    {witness : Nat → Option (Point × List Rat)}
    (h : reachOk t.arities claimed t.rows t.policy (fun p _ => !excluded p) witness = true) :
    t.notExcludedFor excluded claimed :=
  holdsFor_of_reachOk (fun p _ hw => by simpa using hw) h

end RulecCert
