/-
  Climbing the organization (ringi's DESIGN 8.5, 17 item 6). `chain` is the team and the teams
  above it, found by going from each team to the team its parent names, at most 30 of them
  (`chain_eq_ancestors`, `chain_climbs`, `chain_length_le`). Every head the machine finds by
  climbing is the head of a team in that chain and is not left out: the requester's manager
  (`manager_of_people`), the head of the team of a kind (`head_of_kind_people`), the nearest head
  of a position or higher (`nearest_way_people`), and the head a stage with nobody goes up to
  (`up_from_at_found`, `up_from_above_found`). The nearest head of a position also holds, in the
  team they head, a position at the place of the one asked for or later among the organization's
  positions, which are listed lowest first (`rank_eq_some`).

  The `foldl` over `numbers 1 30` is read through `ancestors`, a climb of any length `n`
  (`foldl_climb`, by induction on the list); the 30 comes in once, from `numbers_one_thirty`.
-/
import RingiCore.Org

namespace RingiCore

-- ── Lists ────────────────────────────────────────────────────────────────────

/-- The first element `f` takes to a value, with the elements before it, which it takes to none
    (the climbs of DESIGN 8.5 take the first team that answers). -/
private theorem head?_filterMap_eq_some {α β} {f : α → Option β} {l : List α} {b : β}
    (h : (l.filterMap f).head? = some b) :
    ∃ pre a post, l = pre ++ a :: post ∧ f a = some b ∧ ∀ x ∈ pre, f x = none := by
  rw [List.head?_filterMap] at h
  exact List.findSome?_eq_some_iff.mp h

-- ── Climbing ─────────────────────────────────────────────────────────────────

/-- The teams met going up from the team `at_` names, nearest first, at most `n` of them: the team
    `at_` names in the organization, then the team its parent names, and so on, until a team has
    no parent, a parent names no team of the organization, or `n` teams are met. -/
def ancestors (org : Org) : Nat → Option String → List Team
  | 0, _ => []
  | _ + 1, none => []
  | n + 1, some id =>
    match team_of org id with
    | none => []
    | some t => t :: ancestors org n t.parent

/-- `Climbs org at_ ts`: the teams `ts` are met going up from the team `at_` names, nearest first.
    The first is the team `at_` names (`team_of`), and each next one is the team the one before
    names as its parent. The climb may stop at any team. -/
inductive Climbs (org : Org) : Option String → List Team → Prop
  | stop (at_ : Option String) : Climbs org at_ []
  | up {id : String} {t : Team} {ts : List Team} :
      team_of org id = some t → Climbs org t.parent ts → Climbs org (some id) (t :: ts)

/-- A climb from no team meets no team (DESIGN 8.5). -/
theorem ancestors_none (org : Org) (n : Nat) : ancestors org n none = [] := by
  cases n <;> rfl

/-- A climb of as many steps as the list is long (DESIGN 8.5): the teams it adds are those met
    going up from where it is, at most that many. -/
theorem foldl_climb (org : Org) (l : List Int) (c : Climb) :
    (l.foldl (fun c _ => climb org c) c).teams = c.teams ++ ancestors org l.length c.at_ := by
  induction l generalizing c with
  | nil => simp [ancestors]
  | cons x l ih =>
    rw [List.foldl_cons, ih]
    rcases c with ⟨a, ts⟩
    cases a with
    | none => simp [climb, ancestors_none]
    | some id =>
      cases ht : team_of org id with
      | none => simp [climb, ancestors, ancestors_none, ht]
      | some t => simp [climb, ancestors, ht]

/-- `chain` climbs 30 times (DESIGN 8.5: the depth of the teams is at most 30). -/
theorem numbers_one_thirty : (numbers 1 30).length = 30 := by decide

/-- `chain` (DESIGN 8.5): the teams met going up from the team, at most 30 of them. -/
theorem chain_eq_ancestors (org : Org) (team : Option String) :
    chain org team = ancestors org 30 team := by
  unfold chain
  rw [foldl_climb, numbers_one_thirty]
  rfl

/-- A climb of `n` steps meets at most `n` teams (DESIGN 8.5). -/
theorem ancestors_length_le (org : Org) (n : Nat) (at_ : Option String) :
    (ancestors org n at_).length ≤ n := by
  induction n generalizing at_ with
  | zero => simp [ancestors]
  | succ n ih =>
    cases at_ with
    | none => simp [ancestors]
    | some id =>
      unfold ancestors
      split
      · simp
      · simp only [List.length_cons]
        exact Nat.succ_le_succ (ih _)

/-- The depth of a climb has a bound (DESIGN 8.5, 17 item 6): `chain` meets at most 30 teams. -/
theorem chain_length_le (org : Org) (team : Option String) : (chain org team).length ≤ 30 := by
  rw [chain_eq_ancestors]
  exact ancestors_length_le org 30 team

/-- A climb of `n` steps goes up the parents (DESIGN 8.5): each team it meets after the first is
    the team the one before names as its parent. -/
theorem ancestors_climbs (org : Org) (n : Nat) (at_ : Option String) :
    Climbs org at_ (ancestors org n at_) := by
  induction n generalizing at_ with
  | zero => exact Climbs.stop _
  | succ n ih =>
    cases at_ with
    | none => exact Climbs.stop _
    | some id =>
      unfold ancestors
      split
      · exact Climbs.stop _
      · next t ht => exact Climbs.up ht (ih _)

/-- `chain` goes up the parents (DESIGN 8.5, 17 item 6): its first team is the team it starts
    from, and each next one is the team the one before names as its parent. -/
theorem chain_climbs (org : Org) (team : Option String) : Climbs org team (chain org team) := by
  rw [chain_eq_ancestors]
  exact ancestors_climbs org 30 team

/-- A chain from no team is empty (DESIGN 8.5). -/
theorem chain_none (org : Org) : chain org none = [] := by
  rw [chain_eq_ancestors]
  rfl

/-- A chain from a team of the organization (DESIGN 8.5) starts with that team, and goes on with
    the teams met going up from its parent, at most 29 of them. -/
theorem chain_some {org : Org} {id : String} {t : Team} (ht : team_of org id = some t) :
    chain org (some id) = t :: ancestors org 29 t.parent := by
  rw [chain_eq_ancestors]
  simp [ancestors, ht]

/-- A chain from an ID that names no team of the organization is empty (DESIGN 8.5). -/
theorem chain_some_of_none {org : Org} {id : String} (ht : team_of org id = none) :
    chain org (some id) = [] := by
  rw [chain_eq_ancestors]
  simp [ancestors, ht]

/-- Every team a climb meets is a team of the organization (DESIGN 8.5). -/
theorem mem_ancestors {org : Org} {n : Nat} {at_ : Option String} {t : Team}
    (h : t ∈ ancestors org n at_) : t ∈ org.teams := by
  induction n generalizing at_ with
  | zero => simp [ancestors] at h
  | succ n ih =>
    cases at_ with
    | none => simp [ancestors] at h
    | some id =>
      unfold ancestors at h
      split at h
      · simp at h
      · next u hu =>
        rcases List.mem_cons.mp h with rfl | h
        · exact List.mem_of_find?_eq_some hu
        · exact ih h

/-- Every team in a chain is a team of the organization (DESIGN 8.5). -/
theorem mem_chain {org : Org} {team : Option String} {t : Team} (h : t ∈ chain org team) :
    t ∈ org.teams := by
  rw [chain_eq_ancestors] at h
  exact mem_ancestors h

/-- The person's primary team (DESIGN 5.2, 8.5): the team of a row of theirs in `org.members`
    marked primary. -/
theorem primary_eq_some {org : Org} {person p : String} (h : primary org person = some p) :
    ∃ m ∈ org.members, m.person = person ∧ m.primary = true ∧ m.team = p := by
  unfold primary first? at h
  split at h
  · cases h
  · next m hm =>
    cases h
    have hp := List.find?_some hm
    simp only [Bool.and_eq_true, beq_iff_eq] at hp
    exact ⟨m, List.mem_of_find?_eq_some hm, hp.1, hp.2, rfl⟩

-- ── The places of positions ──────────────────────────────────────────────────

/-- The place of the first element equal to `x`, in a list numbered from `k` (the numbering of
    `indexed`, which `rank` reads; DESIGN 8.1). -/
private theorem find?_zipIdx_snd (l : List String) (x : String) (k : Nat) :
    ((l.zipIdx k).find? (fun q => q.1 == x)).map Prod.snd = (l.findIdx? (· == x)).map (· + k) := by
  induction l generalizing k with
  | nil => rfl
  | cons a l ih =>
    rw [List.zipIdx_cons, List.find?_cons, List.findIdx?_cons]
    by_cases ha : (a == x) = true
    · simp [ha]
    · simp only [ha]
      simp only [Bool.false_eq_true, ite_false]
      rw [ih (k + 1), Option.map_map]
      congr 1
      funext i
      simp only [Function.comp]
      omega

/-- `rank` is the first place of the position in the organization's positions, counted from 0. -/
theorem rank_eq_findIdx? (org : Org) (x : String) :
    rank org x = (org.positions.findIdx? (· == x)).map (fun i : Nat => (i : Int)) := by
  have h := find?_zipIdx_snd org.positions x 0
  unfold rank first? indexed
  rw [List.find?_map]
  have hp : ((fun y : Indexed String => y.item == x) ∘
      (fun q : String × Nat => match q with | (x, i) => ({ index := ↑i, item := x } : Indexed String))) =
      (fun q : String × Nat => q.1 == x) := by
    funext q
    rcases q with ⟨y, i⟩
    rfl
  rw [hp]
  cases hf : List.find? (fun q : String × Nat => q.1 == x) org.positions.zipIdx with
  | none =>
    rw [hf] at h
    simp only [Option.map_none] at h ⊢
    cases hi : List.findIdx? (· == x) org.positions with
    | none => rfl
    | some i => rw [hi] at h; cases h
  | some q =>
    rw [hf] at h
    rcases q with ⟨y, i⟩
    simp only [Option.map_some] at h ⊢
    cases hi : List.findIdx? (· == x) org.positions with
    | none => rw [hi] at h; cases h
    | some j =>
      rw [hi] at h
      simp only [Option.map_some, Option.some.injEq] at h
      simp only [Option.map_some, Option.some.injEq]
      omega

/-- The place of a position (DESIGN 8.1, 8.5): `rank org x = some r` says `r` is the first place
    of `x` in `org.positions`, which lists the positions lowest first. -/
theorem rank_eq_some_iff {org : Org} {x : String} {r : Int} :
    rank org x = some r ↔ ∃ i : Nat, r = i ∧ org.positions[i]? = some x ∧
      ∀ j < i, org.positions[j]? ≠ some x := by
  rw [rank_eq_findIdx?]
  constructor
  · intro h
    rcases ho : List.findIdx? (· == x) org.positions with _ | i
    · rw [ho] at h; cases h
    · rw [ho] at h
      cases h
      obtain ⟨hlt, hi, hj⟩ := List.findIdx?_eq_some_iff_getElem.mp ho
      refine ⟨i, rfl, ?_, ?_⟩
      · rw [List.getElem?_eq_some_iff]
        exact ⟨hlt, by simpa using hi⟩
      · intro j hji hj'
        rw [List.getElem?_eq_some_iff] at hj'
        obtain ⟨hjl, hjx⟩ := hj'
        exact hj j hji (by simpa using hjx)
  · rintro ⟨i, rfl, hi, hj⟩
    rw [List.getElem?_eq_some_iff] at hi
    obtain ⟨hlt, hix⟩ := hi
    have : List.findIdx? (· == x) org.positions = some i := by
      rw [List.findIdx?_eq_some_iff_getElem]
      refine ⟨hlt, by simpa using hix, ?_⟩
      intro j hji hjx
      exact hj j hji (List.getElem?_eq_some_iff.mpr ⟨_, by simpa using hjx⟩)
    rw [this]
    rfl

/-- The place of a position, as the stages read it (DESIGN 8.1, 8.5, 17 item 6): a place `r` that
    `rank` answers is not negative, holds the position in `org.positions`, and no place before it
    does. -/
theorem rank_eq_some {org : Org} {x : String} {r : Int} (h : rank org x = some r) :
    0 ≤ r ∧ org.positions[r.toNat]? = some x ∧ ∀ j < r.toNat, org.positions[j]? ≠ some x := by
  obtain ⟨i, rfl, hi, hj⟩ := rank_eq_some_iff.mp h
  simp only [Int.toNat_natCast]
  exact ⟨Int.natCast_nonneg i, hi, hj⟩

/-- A position has a place exactly when the organization declares it (DESIGN 8.1). -/
theorem rank_eq_none_iff {org : Org} {x : String} : rank org x = none ↔ x ∉ org.positions := by
  rw [rank_eq_findIdx?, Option.map_eq_none_iff, List.findIdx?_eq_none_iff]
  constructor
  · intro h hx
    have := h x hx
    simp at this
  · intro h y hy
    simp only [beq_eq_false_iff_ne, ne_eq]
    rintro rfl
    exact h hy

-- ── The heads a climb keeps ──────────────────────────────────────────────────

/-- The head `head_for` keeps (DESIGN 5.5, 8.5): the team's head, neither the requester nor left
    out. -/
theorem head_for_some {spec : Spec} {who : Who} {t : Team} {x : String}
    (h : head_for spec who t = some x) :
    t.head = some x ∧ x ≠ who.requester ∧ left_out spec who x = false := by
  unfold head_for at h
  split at h
  · cases h
  · next y hy =>
    split at h
    · cases h
    · next hc =>
      cases h
      simp only [Bool.or_eq_true, beq_iff_eq, not_or, Bool.not_eq_true] at hc
      exact ⟨hy, hc.1, hc.2⟩

/-- A team `head_for` passes over (DESIGN 5.5, 8.5): it has no head, or its head is the requester
    or left out. -/
theorem head_for_eq_none {spec : Spec} {who : Who} {t : Team} :
    head_for spec who t = none ↔ ∀ y, t.head = some y → y = who.requester ∨ left_out spec who y = true := by
  unfold head_for
  split
  · next hy => simp [hy]
  · next y hy =>
    simp only [hy, Option.some.injEq, forall_eq']
    split
    · next hc => simpa [Bool.or_eq_true, beq_iff_eq] using hc
    · next hc => simpa [Bool.or_eq_true, beq_iff_eq] using hc

/-- The head `head_kept` keeps (DESIGN 5.4): the team's head, not left out. -/
theorem head_kept_some {spec : Spec} {who : Who} {t : Team} {x : String}
    (h : head_kept spec who t = some x) : t.head = some x ∧ left_out spec who x = false := by
  unfold head_kept at h
  split at h
  · cases h
  · next y hy =>
    split at h
    · cases h
    · next hc =>
      cases h
      exact ⟨hy, by simpa using hc⟩

/-- A team `head_kept` passes over (DESIGN 5.4): it has no head, or its head is left out. -/
theorem head_kept_eq_none {spec : Spec} {who : Who} {t : Team} :
    head_kept spec who t = none ↔ ∀ y, t.head = some y → left_out spec who y = true := by
  unfold head_kept
  split
  · next hy => simp [hy]
  · next y hy =>
    simp only [hy, Option.some.injEq, forall_eq']
    split
    · next hc => simpa using hc
    · next hc => simpa using hc

/-- The head `senior` keeps (DESIGN 8.5): the team's head, neither the requester nor left out, with
    a row in the team whose position is at the place `want` or later. -/
theorem senior_some {spec : Spec} {who : Who} {org : Org} {t : Team} {want : Int} {x : String}
    (h : senior spec who org t want = some x) :
    t.head = some x ∧ x ≠ who.requester ∧ left_out spec who x = false ∧
      ∃ m ∈ org.members, m.person = x ∧ m.team = t.id ∧
        ∃ r, rank org m.position = some r ∧ want ≤ r := by
  unfold senior at h
  split at h
  · cases h
  · next y hy =>
    obtain ⟨hh, hr, hl⟩ := head_for_some hy
    split at h
    · cases h
    · next m hm =>
      split at h
      · cases h
      · next r hrk =>
        split at h
        · next hge =>
          cases h
          have hp := List.find?_some hm
          simp only [Bool.and_eq_true, beq_iff_eq] at hp
          exact ⟨hh, hr, hl, m, List.mem_of_find?_eq_some hm, hp.1, hp.2, r, hrk, hge⟩
        · cases h

/-- A way that finds nobody (DESIGN 5.4) answers no people. -/
private theorem nobody_ne_people (why : Why) (up : Up) (ps : List String) (tm : Option String) :
    nobody why up ≠ .people ps tm := by
  simp [nobody]

/-- A way the organization cannot decide (DESIGN 5.4, 8.5) answers no people. -/
private theorem cannot_ne_people (note : String) (ps : List String) (tm : Option String) :
    cannot note ≠ .people ps tm := by
  simp [cannot]

-- ── The ways that climb ──────────────────────────────────────────────────────

/-- The nearest head of a position, as the machine finds them (DESIGN 5.2, 8.5): the head that
    `senior` keeps in the first team of the chain above the requester's primary team where it keeps
    one; the teams before it in the chain have none it keeps. -/
theorem nearest_way_first {spec : Spec} {who : Who} {org : Org} {position written : String}
    {ps : List String} {tm : Option String}
    (hw : nearest_way spec who org position written = .people ps tm) :
    ∃ p want x, primary org who.requester = some p ∧ rank org position = some want ∧
      ps = [x] ∧ tm = none ∧
      ∃ pre t post, chain org (some p) = pre ++ t :: post ∧ senior spec who org t want = some x ∧
        ∀ t' ∈ pre, senior spec who org t' want = none := by
  unfold nearest_way at hw
  split at hw
  · exact absurd hw (nobody_ne_people _ _ _ _)
  · next p hp =>
    split at hw
    · exact absurd hw (cannot_ne_people _ _ _)
    · next want hwant =>
      split at hw
      · exact absurd hw (nobody_ne_people _ _ _ _)
      · next x hx =>
        cases hw
        obtain ⟨pre, t, post, hc, ht, hpre⟩ := head?_filterMap_eq_some hx
        exact ⟨p, want, x, hp, hwant, rfl, rfl, pre, t, post, hc, ht, hpre⟩

/-- The nearest head of a position or higher (DESIGN 5.2, 8.5, 17 item 6): the one person found is
    the head of a team in the chain above the requester's primary team, is neither the requester
    nor left out, and holds in that team a position at the place of the one asked for or later. -/
theorem nearest_way_people {spec : Spec} {who : Who} {org : Org} {position written : String}
    {ps : List String} {tm : Option String}
    (hw : nearest_way spec who org position written = .people ps tm) :
    ∃ p x, ps = [x] ∧ tm = none ∧ primary org who.requester = some p ∧
      ∃ t ∈ chain org (some p), t.head = some x ∧ x ≠ who.requester ∧
        left_out spec who x = false ∧
        ∃ m ∈ org.members, m.person = x ∧ m.team = t.id ∧
          ∃ r want, rank org m.position = some r ∧ rank org position = some want ∧ want ≤ r := by
  obtain ⟨p, want, x, hp, hwant, rfl, rfl, pre, t, post, hc, ht, -⟩ := nearest_way_first hw
  obtain ⟨hh, hr, hl, m, hm, hmp, hmt, r, hrk, hge⟩ := senior_some ht
  refine ⟨p, x, rfl, rfl, hp, t, by simp [hc], hh, hr, hl, m, hm, hmp, hmt, r, want, hrk, hwant, hge⟩

/-- The requester's manager, as the machine finds them (DESIGN 5.2, 8.5): the head of the first
    team of the chain above the requester's primary team whose head is neither the requester nor
    left out; each team before it has no head, or the requester or one left out as its head. -/
theorem manager_of_first {spec : Spec} {who : Who} {org : Org} {ps : List String}
    {tm : Option String} (hw : manager_of spec who org = .people ps tm) :
    ∃ p x, primary org who.requester = some p ∧ ps = [x] ∧ tm = none ∧
      ∃ pre t post, chain org (some p) = pre ++ t :: post ∧
        t.head = some x ∧ x ≠ who.requester ∧ left_out spec who x = false ∧
        ∀ t' ∈ pre, ∀ y, t'.head = some y → y = who.requester ∨ left_out spec who y = true := by
  unfold manager_of at hw
  split at hw
  · exact absurd hw (nobody_ne_people _ _ _ _)
  · next p hp =>
    split at hw
    · exact absurd hw (nobody_ne_people _ _ _ _)
    · next x hx =>
      cases hw
      obtain ⟨pre, t, post, hc, ht, hpre⟩ := head?_filterMap_eq_some hx
      obtain ⟨hh, hr, hl⟩ := head_for_some ht
      exact ⟨p, x, hp, rfl, rfl, pre, t, post, hc, hh, hr, hl,
        fun t' h' => head_for_eq_none.mp (hpre t' h')⟩

/-- The requester's manager (DESIGN 5.2, 8.5, 17 item 6): the one person found is the head of a
    team in the chain above the requester's primary team, and is neither the requester nor left
    out. -/
theorem manager_of_people {spec : Spec} {who : Who} {org : Org} {ps : List String}
    {tm : Option String} (hw : manager_of spec who org = .people ps tm) :
    ∃ p x, ps = [x] ∧ tm = none ∧ primary org who.requester = some p ∧
      ∃ t ∈ chain org (some p), t.head = some x ∧ x ≠ who.requester ∧
        left_out spec who x = false := by
  obtain ⟨p, x, hp, rfl, rfl, pre, t, post, hc, hh, hr, hl, -⟩ := manager_of_first hw
  exact ⟨p, x, rfl, rfl, hp, t, by simp [hc], hh, hr, hl⟩

/-- The head of the team of a kind, as the machine finds them (DESIGN 5.2, 8.5): the first team of
    the kind in the chain above the requester's primary team, whose head is not left out; no team
    before it in the chain is of the kind. -/
theorem head_of_kind_first {spec : Spec} {who : Who} {org : Org} {kind : String}
    {ps : List String} {tm : Option String} (hw : head_of_kind spec who org kind = .people ps tm) :
    ∃ p x, primary org who.requester = some p ∧ ps = [x] ∧ tm = none ∧
      ∃ pre t post, chain org (some p) = pre ++ t :: post ∧ t.kind = kind ∧
        (∀ t' ∈ pre, t'.kind ≠ kind) ∧ t.head = some x ∧ left_out spec who x = false := by
  unfold head_of_kind at hw
  split at hw
  · exact absurd hw (nobody_ne_people _ _ _ _)
  · next p hp =>
    split at hw
    · exact absurd hw (nobody_ne_people _ _ _ _)
    · next t ht =>
      split at hw
      · exact absurd hw (nobody_ne_people _ _ _ _)
      · next x hx =>
        split at hw
        · exact absurd hw (nobody_ne_people _ _ _ _)
        · next hl =>
          cases hw
          obtain ⟨hk, pre, post, hc, hpre⟩ := List.find?_eq_some_iff_append.mp ht
          simp only [beq_iff_eq] at hk
          refine ⟨p, x, hp, rfl, rfl, pre, t, post, hc, hk, ?_, hx, by simpa using hl⟩
          intro t' h' hk'
          have := hpre t' h'
          simp [hk'] at this

/-- The head of the team of a kind (DESIGN 5.2, 8.5, 17 item 6): the one person found is the head
    of a team of the kind in the chain above the requester's primary team, and is not left out. -/
theorem head_of_kind_people {spec : Spec} {who : Who} {org : Org} {kind : String}
    {ps : List String} {tm : Option String} (hw : head_of_kind spec who org kind = .people ps tm) :
    ∃ p x, ps = [x] ∧ tm = none ∧ primary org who.requester = some p ∧
      ∃ t ∈ chain org (some p), t.kind = kind ∧ t.head = some x ∧ left_out spec who x = false := by
  obtain ⟨p, x, hp, rfl, rfl, pre, t, post, hc, hk, -, hh, hl⟩ := head_of_kind_first hw
  exact ⟨p, x, rfl, rfl, hp, t, by simp [hc], hk, hh, hl⟩

/-- Going up from a team (DESIGN 5.4, 8.5): where a stage with nobody goes up to from `.at_ team`
    is the head of the first team of the chain from that team whose head is not left out; each
    team before it has no head, or one left out. -/
theorem up_from_at_first {spec : Spec} {who : Who} {org : Org} {team x : String}
    (h : up_from spec who org (.at_ team) = some x) :
    ∃ pre t post, chain org (some team) = pre ++ t :: post ∧ t.head = some x ∧
      left_out spec who x = false ∧
      ∀ t' ∈ pre, ∀ y, t'.head = some y → left_out spec who y = true := by
  simp only [up_from] at h
  obtain ⟨pre, t, post, hc, ht, hpre⟩ := head?_filterMap_eq_some h
  obtain ⟨hh, hl⟩ := head_kept_some ht
  exact ⟨pre, t, post, hc, hh, hl, fun t' h' => head_kept_eq_none.mp (hpre t' h')⟩

/-- Going up from a team (DESIGN 5.4, 8.5, 17 item 6): the head found is the head of a team in the
    chain from that team, and is not left out. -/
theorem up_from_at_found {spec : Spec} {who : Who} {org : Org} {team x : String}
    (h : up_from spec who org (.at_ team) = some x) :
    ∃ t ∈ chain org (some team), t.head = some x ∧ left_out spec who x = false := by
  obtain ⟨pre, t, post, hc, hh, hl, -⟩ := up_from_at_first h
  exact ⟨t, by simp [hc], hh, hl⟩

/-- Going up from above a team (DESIGN 5.4, 8.5): where a stage with nobody goes up to from
    `.above team` is found as from `.at_ team`, past the team itself: the chain from the team is
    the team and the teams above it, and the head is that of the first of those above whose head is
    not left out. -/
theorem up_from_above_first {spec : Spec} {who : Who} {org : Org} {team x : String}
    (h : up_from spec who org (.above team) = some x) :
    ∃ t0 above, team_of org team = some t0 ∧ chain org (some team) = t0 :: above ∧
      ∃ pre t post, above = pre ++ t :: post ∧ t.head = some x ∧ left_out spec who x = false ∧
        ∀ t' ∈ pre, ∀ y, t'.head = some y → left_out spec who y = true := by
  simp only [up_from, dropI] at h
  cases ht0 : team_of org team with
  | none =>
    rw [chain_some_of_none ht0] at h
    simp at h
  | some t0 =>
    rw [chain_some ht0] at h
    simp only [Int.toNat_one, List.drop_succ_cons, List.drop_zero] at h
    obtain ⟨pre, t, post, hc, ht, hpre⟩ := head?_filterMap_eq_some h
    obtain ⟨hh, hl⟩ := head_kept_some ht
    exact ⟨t0, _, rfl, chain_some ht0, pre, t, post, hc, hh, hl,
      fun t' h' => head_kept_eq_none.mp (hpre t' h')⟩

/-- Going up from above a team (DESIGN 5.4, 8.5, 17 item 6): the head found is the head of a team
    in the chain from that team, after the team itself, and is not left out. -/
theorem up_from_above_found {spec : Spec} {who : Who} {org : Org} {team x : String}
    (h : up_from spec who org (.above team) = some x) :
    ∃ t ∈ (chain org (some team)).drop 1, t.head = some x ∧ left_out spec who x = false := by
  obtain ⟨t0, above, -, hc, pre, t, post, ha, hh, hl, -⟩ := up_from_above_first h
  exact ⟨t, by simp [hc, ha], hh, hl⟩

end RingiCore
