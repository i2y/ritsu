/-
  Self approval, on the side of the organization (ringi's DESIGN 5.5, 8.5, 17 item 5). Nobody the
  machine chooses to answer a stage is left out: not by any way to choose (`solve_kept`), nor where
  a stage with nobody goes up to (`up_from_kept`). With self approval not allowed, being left out
  is being the requester or the one who made the request for them (`left_out_iff`), so neither of
  them is chosen (`solve_not_self`, `up_from_not_self`); with self approval allowed, nobody is left
  out (`left_out_allowed`).
-/
import RingiCore.Org

namespace RingiCore

-- ── Who is left out ──────────────────────────────────────────────────────────

/-- With self approval allowed (DESIGN 5.5), nobody is left out. -/
theorem left_out_allowed {spec : Spec} (hs : spec.self_approval = true) (who : Who) (x : String) :
    left_out spec who x = false := by
  simp [left_out, hs]

/-- With self approval not allowed (DESIGN 5.5), a person is kept exactly when they are neither the
    requester nor the one who made the request for them. -/
theorem left_out_iff {spec : Spec} (hs : spec.self_approval = false) (who : Who) (x : String) :
    left_out spec who x = false ↔ x ≠ who.requester ∧ who.made_by ≠ some x := by
  simp only [left_out, hs, Bool.not_false, Bool.true_and, Bool.or_eq_false_iff, beq_eq_false_iff_ne,
    ne_eq]
  constructor
  · rintro ⟨h1, h2⟩
    exact ⟨fun h => h1 h.symm, h2⟩
  · rintro ⟨h1, h2⟩
    exact ⟨fun h => h1 h.symm, h2⟩

-- ── The ways to choose ───────────────────────────────────────────────────────

/-- A way that finds nobody (DESIGN 5.4) answers no people. -/
private theorem nobody_ne (why : Why) (up : Up) (ps : List String) (tm : Option String) :
    nobody why up ≠ .people ps tm := by
  simp [nobody]

/-- A way the organization cannot decide (DESIGN 5.4, 8.5) answers no people. -/
private theorem cannot_ne (note : String) (ps : List String) (tm : Option String) :
    cannot note ≠ .people ps tm := by
  simp [cannot]

/-- The head `head_for` keeps is not left out (DESIGN 5.5, 8.5). -/
private theorem head_for_kept {spec : Spec} {who : Who} {t : Team} {x : String}
    (h : head_for spec who t = some x) : left_out spec who x = false := by
  unfold head_for at h
  split at h
  · cases h
  · split at h
    · cases h
    · next hc =>
      cases h
      simp only [Bool.or_eq_true, not_or, Bool.not_eq_true] at hc
      exact hc.2

/-- The requester's manager is not left out (DESIGN 5.5, 8.5). -/
theorem manager_of_kept {spec : Spec} {who : Who} {org : Org} {ps : List String}
    {tm : Option String} (h : manager_of spec who org = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  unfold manager_of at h
  split at h
  · exact absurd h (nobody_ne _ _ _ _)
  · split at h
    · exact absurd h (nobody_ne _ _ _ _)
    · next y hy =>
      cases h
      rw [List.head?_filterMap, List.findSome?_eq_some_iff] at hy
      obtain ⟨_, t, _, _, ht, _⟩ := hy
      intro x hx
      rw [List.mem_singleton] at hx
      subst hx
      exact head_for_kept ht

/-- The head of the team of a kind is not left out (DESIGN 5.5, 8.5). -/
theorem head_of_kind_kept {spec : Spec} {who : Who} {org : Org} {kind : String}
    {ps : List String} {tm : Option String} (h : head_of_kind spec who org kind = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  unfold head_of_kind at h
  split at h
  · exact absurd h (nobody_ne _ _ _ _)
  · split at h
    · exact absurd h (nobody_ne _ _ _ _)
    · split at h
      · exact absurd h (nobody_ne _ _ _ _)
      · split at h
        · exact absurd h (nobody_ne _ _ _ _)
        · next hl =>
          cases h
          intro x hx
          rw [List.mem_singleton] at hx
          subst hx
          simpa using hl

/-- The nearest head of a position is not left out (DESIGN 5.5, 8.5). -/
theorem nearest_way_kept {spec : Spec} {who : Who} {org : Org} {position written : String}
    {ps : List String} {tm : Option String}
    (h : nearest_way spec who org position written = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  unfold nearest_way at h
  split at h
  · exact absurd h (nobody_ne _ _ _ _)
  · split at h
    · exact absurd h (cannot_ne _ _ _)
    · next want _ =>
      split at h
      · exact absurd h (nobody_ne _ _ _ _)
      · next y hy =>
        cases h
        rw [List.head?_filterMap, List.findSome?_eq_some_iff] at hy
        obtain ⟨_, t, _, _, ht, _⟩ := hy
        intro x hx
        rw [List.mem_singleton] at hx
        subst hx
        unfold senior at ht
        split at ht
        · cases ht
        · next z hz =>
          split at ht
          · cases ht
          · split at ht
            · cases ht
            · split at ht
              · cases ht
                exact head_for_kept hz
              · cases ht

/-- The people of a team are those of its members not left out (DESIGN 5.5, 8.5). -/
theorem team_way_kept {spec : Spec} {who : Who} {org : Org} {id : String} {ps : List String}
    {tm : Option String} (h : team_way spec who org id = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  simp only [team_way] at h
  split at h
  · exact absurd h (nobody_ne _ _ _ _)
  · split at h
    · exact absurd h (nobody_ne _ _ _ _)
    · split at h
      · exact absurd h (nobody_ne _ _ _ _)
      · cases h
        intro x hx
        simp only [List.mem_filter, Bool.not_eq_true'] at hx
        exact hx.2

/-- A person named is chosen only when they are not left out (DESIGN 5.5, 8.5). -/
theorem person_way_kept {spec : Spec} {who : Who} {org : Org} {id : String} {ps : List String}
    {tm : Option String} (h : person_way spec who org id = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  unfold person_way at h
  split at h
  · split at h
    · split at h
      · exact absurd h (nobody_ne _ _ _ _)
      · exact absurd h (nobody_ne _ _ _ _)
    · next hl =>
      cases h
      intro x hx
      rw [List.mem_singleton] at hx
      subst hx
      simpa using hl
  · exact absurd h (nobody_ne _ _ _ _)

/-- The person a master's row names is chosen only when they are not left out (DESIGN 5.5, 8.5). -/
theorem lookup_way_kept {spec : Spec} {who : Who} {vals : List Arg} {org : Org}
    {master field by_ shown column by_name : String} {ps : List String} {tm : Option String}
    (h : lookup_way spec who vals org master field by_ shown column by_name = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  unfold lookup_way at h
  split at h
  · exact absurd h (cannot_ne _ _ _)
  · split at h
    · exact absurd h (cannot_ne _ _ _)
    · split at h
      · exact absurd h (cannot_ne _ _ _)
      · exact person_way_kept h

/-- Every way to choose (DESIGN 5.2, 5.5, 8.5): nobody it chooses is left out. -/
theorem solve_way_kept {spec : Spec} {who : Who} {vals : List Arg} {org : Org} {w : Way}
    {ps : List String} {tm : Option String} (h : solve_way spec who vals org w = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  unfold solve_way at h
  split at h
  · exact manager_of_kept h
  · exact head_of_kind_kept h
  · exact nearest_way_kept h
  · exact team_way_kept h
  · exact person_way_kept h
  · exact lookup_way_kept h
  · split at h
    · exact absurd h (cannot_ne _ _ _)
    · exact person_way_kept h
  · exact absurd h (cannot_ne _ _ _)

/-- The way to choose of a stage, solved in the organization (DESIGN 5.5, 8.5, 17 item 5): nobody
    it chooses is left out, the way after `else` of an agent's way among them. -/
theorem solve_kept {spec : Spec} {who : Who} {vals : List Arg} {org : Org} {way : String}
    {ps : List String} {tm : Option String} (h : solve spec who vals org way = .people ps tm) :
    ∀ x ∈ ps, left_out spec who x = false := by
  unfold solve at h
  split at h
  · exact absurd h (cannot_ne _ _ _)
  · split at h
    · split at h
      · exact absurd h (cannot_ne _ _ _)
      · exact solve_way_kept h
    · exact solve_way_kept h

/-- Self approval not allowed (DESIGN 5.5, 17 item 5): nobody a way to choose chooses is the
    requester, or the one who made the request for them. -/
theorem solve_not_self {spec : Spec} {who : Who} {vals : List Arg} {org : Org} {way : String}
    {ps : List String} {tm : Option String} (hs : spec.self_approval = false)
    (h : solve spec who vals org way = .people ps tm) :
    ∀ x ∈ ps, x ≠ who.requester ∧ who.made_by ≠ some x :=
  fun x hx => (left_out_iff hs who x).mp (solve_kept h x hx)

-- ── Going up ─────────────────────────────────────────────────────────────────

/-- Where a stage with nobody goes up to (DESIGN 5.4, 5.5, 8.5): the head found is not left out. -/
theorem up_from_kept {spec : Spec} {who : Who} {org : Org} {origin : Up} {x : String}
    (h : up_from spec who org origin = some x) : left_out spec who x = false := by
  have kept : ∀ l : List Team, (l.filterMap (fun t => head_kept spec who t)).head? = some x →
      left_out spec who x = false := by
    intro l hl
    rw [List.head?_filterMap, List.findSome?_eq_some_iff] at hl
    obtain ⟨_, t, _, _, ht, _⟩ := hl
    unfold head_kept at ht
    split at ht
    · cases ht
    · split at ht
      · cases ht
      · next hc =>
        cases ht
        simpa using hc
  cases origin with
  | at_ team => exact kept _ h
  | above team => exact kept _ h
  | cannot => cases h

/-- Self approval not allowed (DESIGN 5.4, 5.5, 17 item 5): where a stage with nobody goes up to is
    neither the requester nor the one who made the request for them. -/
theorem up_from_not_self {spec : Spec} {who : Who} {org : Org} {origin : Up} {x : String}
    (hs : spec.self_approval = false) (h : up_from spec who org origin = some x) :
    x ≠ who.requester ∧ who.made_by ≠ some x :=
  (left_out_iff hs who x).mp (up_from_kept h)

end RingiCore
