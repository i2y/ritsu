/-
  The places where the evaluation can fail (kaname's DESIGN 5.4, 15.2.3): what each one takes, the
  values that do not keep it (`FailsAt`), and that a place the analysis shows takes none of those
  among the values it is given (`shown_sound`).

  Where the evaluation fails with one of the five kinds of the effects, the analysis kept a place
  that is not shown, or the failure is one of a function it calls, called with values its
  parameters take (`Wit`). A place kept stays kept to the end of the function with every value it
  held (`Grows`), so the evidence reaches the end of the analysis.
-/
import KanameCore.Narrow

namespace KanameCore

/-- A value the place does not let through, as the evaluation holds it there. What a check across
    the languages lets through is the other language's to say: a place that waits for its verdict,
    or whose verdict does not hold, can fail for any value; one whose verdict holds, for none (what
    the verdict says, which `total_with_sound` takes as given). -/
def FailsAt : What → Val → Prop
  | .ranged r, c => ∃ p, c.point = some p ∧ outRange r p = true
  | .param s, c => s.outside true c = true
  | .slot s, c => s.outside false c = true
  | .isqrt, c => ∃ r, c = .num r ∧ r.num < 0
  | .overflow, c => ∃ r, c = .num r ∧ fits r = false
  | .zero, c => c = .num 0
  | .pending, _ => True
  | .across holds, _ => holds = false
  | .oneOf vs, c => ∃ x, c = .enum x ∧ vs.contains x = false

/-- Values a function's parameters take: as many as it has, each among the values of its type and
    in the range it declares. -/
def Valid (fn : Fn) (args : List Val) : Prop :=
  args.length = fn.params.length ∧
    ∀ p v, (p, v) ∈ fn.params.zip args → Mem p.abs v ∧ p.slot.outside true v = false

/-- A function that fails with none of the five kinds wherever it is called with values its
    parameters take. -/
def NoFail (C : Ctx) (f : Nat) : Prop :=
  ∀ fn, C.M.fns[f]? = some fn → ∀ n dep args, Valid fn args →
    ∀ k l, callFn C n dep f args = .error (.failed k l) → k.ofEffect = false

/-- **What the analysis kept says the evaluation can fail**: a place that a value it holds does
    not keep (or one for int64 that is not shown), or a function called that can fail. -/
def Wit (C : Ctx) (o : Out) : Prop :=
  (∃ p ∈ o.places, (∃ c, Mem p.value c ∧ FailsAt p.what c) ∨ (p.what = .overflow ∧ okNum p.value = false)) ∨
    (∃ g ∈ o.calls, ¬ NoFail C g)

theorem Wit.grow {C : Ctx} {o o' : Out} (h : Wit C o) (g : Grows o o') : Wit C o' := by
  rcases h with ⟨p, hp, hw⟩ | ⟨f, hf, hn⟩
  · obtain ⟨p', hp', hk, hwh, hm, hok⟩ := g.places p hp
    refine Or.inl ⟨p', hp', ?_⟩
    rcases hw with ⟨c, hc, hfa⟩ | ⟨ho, hn⟩
    · exact Or.inl ⟨c, hm c hc, hwh ▸ hfa⟩
    · refine Or.inr ⟨hwh.trans ho, ?_⟩
      cases h' : okNum p'.value
      · rfl
      · exact absurd (hok h') (by rw [hn]; simp)
  · exact Or.inr ⟨f, g.calls f hf, hn⟩

/-- A place given a value it fails for. -/
theorem wit_place {C : Ctx} {o : Out} {e : Nat} {key : String} {kind : Kind} {what : What} {v : Abs} {c : Val}
    (hm : Mem v c) (hf : FailsAt what c) : Wit C (o.place e key kind what v) := by
  obtain ⟨p, hp, _, hw, hmem, _⟩ := place_holds o e key kind what v
  exact Or.inl ⟨p, hp, Or.inl ⟨c, hmem c hm, hw ▸ hf⟩⟩

/-- A place for int64 given values that are not shown to keep it. -/
theorem wit_overflow {C : Ctx} {o : Out} {e : Nat} {key : String} {kind : Kind} {v : Abs}
    (hv : okNum v = false) : Wit C (o.place e key kind .overflow v) := by
  obtain ⟨p, hp, _, hw, _, hok⟩ := place_holds o e key kind .overflow v
  refine Or.inl ⟨p, hp, Or.inr ⟨hw, ?_⟩⟩
  cases h' : okNum p.value
  · rfl
  · exact absurd (hok h') (by rw [hv]; simp)

-- ── Shown places take no value they fail for ───────────────────────────────

theorem not_outRange {r : Range} {p : Rat} (h1 : EndLe r.min p) (h2 : EndGe r.max p) : outRange r p = false := by
  unfold outRange
  rcases hmin : r.min with _ | l <;> rcases hmax : r.max with _ | u
  · rfl
  · rw [hmax] at h2; simp only [EndGe] at h2; simp [Rat.not_lt.mpr h2]
  · rw [hmin] at h1; simp only [EndLe] at h1; simp [Rat.not_lt.mpr h1]
  · rw [hmin] at h1; rw [hmax] at h2; simp only [EndLe, EndGe] at h1 h2
    simp [Rat.not_lt.mpr h1, Rat.not_lt.mpr h2]

theorem Mem_stripOpt {a : Abs} {v : Val} (h : Mem a v) (hnn : v ≠ .none) : Mem (stripOpt a) v := by
  cases h with
  | optNone => exact absurd rfl hnn
  | optSome _ hx => exact hx
  | any => exact Mem.any _
  | num hi => exact Mem.num hi
  | date hi => exact Mem.date hi
  | enumAll => exact Mem.enumAll
  | boolAll => exact Mem.boolAll
  | enum hx => exact Mem.enum hx
  | bool hx => exact Mem.bool hx
  | str hc => exact Mem.str hc
  | list hc he => exact Mem.list hc he
  | record hf hr => exact Mem.record hf hr
  | transfer hf => exact Mem.transfer hf
  | variantEnum h => exact Mem.variantEnum h
  | variant h1 h2 h3 h4 => exact Mem.variant h1 h2 h3 h4
  | dict hc hk hv hnd => exact Mem.dict hc hk hv hnd
  | members hc he hnd => exact Mem.members hc he hnd
  | closAny => exact Mem.closAny
  | clos h1 h2 h3 => exact Mem.clos h1 h2 h3

theorem point_in {a : Abs} {c : Val} {p : Rat} {i : Iv} (hm : Mem a c) (hp : c.point = some p) (hi : a.iv = some i) :
    i.In p := by
  cases c with
  | num r => simp [Val.point] at hp; subst hp; exact Mem_iv_num hm hi
  | date d => simp [Val.point] at hp; subst hp; exact Mem_iv_date hm hi
  | _ => simp [Val.point] at hp

/-- The range check of one value of a place. -/
theorem range_ok {elem : Abs} {x : Val} {r : Option Range} (hm : Mem elem x) (hin : rangeOK r elem = true) :
    pointOut r x = false := by
  unfold pointOut
  split
  · rename_i rr p hp
    simp only [rangeOK] at hin
    rcases hiv : elem.iv with _ | i <;> rw [hiv] at hin <;> simp at hin
    have := within_sound (point_in hm hp hiv) hin
    exact not_outRange this.1 this.2
  · rfl

/-- A string whose count of code points `strOK` finds within what a place declares is not outside it. -/
theorem str_ok {elem : Abs} {x : Val} {list : Bool} {count : Option Range} (hm : Mem elem x)
    (hin : strOK list count elem = true) : strOut list count x = false := by
  cases x with
  | str s =>
    cases list with
    | true => simp [strOut]
    | false =>
      cases count with
      | none => simp [strOut]
      | some c =>
        simp only [strOK] at hin
        split at hin
        · rename_i i
          cases hm with
          | str hi =>
            have := within_sound hi hin
            simp only [strOut, Bool.not_false, Bool.true_and, Option.any_some]
            exact not_outRange this.1 this.2
        · simp at hin
  | _ => simp [strOut]

/-- A count within what a place declares is not outside it. -/
theorem counted_ok {count : Option Range} {cnt : Iv} {e elem : Abs} {n : Nat} {ys : List Val} (hcl : cnt.In (n : Rat))
    (he : countedAbs count cnt e = some elem) : countedIn count n ys = some ys ∧ elem = stripOpt e := by
  unfold countedAbs at he
  split at he
  · simp at he
  · rename_i hcnt
    simp only [Option.some.injEq] at he
    refine ⟨?_, he.symm⟩
    unfold countedIn
    split
    · rename_i cr
      simp only [Option.any_some, Bool.not_eq_true', Bool.not_eq_false] at hcnt
      have hw : cnt.within cr.min cr.max = true := by simpa using hcnt
      have := within_sound hcl hw
      rw [not_outRange this.1 this.2]; rfl
    · rfl

/-- The values of a place where its count is not outside, each among what an element can be: a
    list's elements, a set's members, a dict's values. -/
theorem elems_ok {list : Bool} {count : Option Range} {a : Abs} {c : Val} {elem : Abs}
    (hm : Mem a c) (hc : c ≠ .none) (he : elemAbs list count a = some elem) :
    ∃ xs, elemsOf list count c = some xs ∧ ∀ x ∈ xs, x ≠ .none → Mem elem x := by
  unfold elemAbs at he
  cases list with
  | false =>
    simp at he; subst he
    exact ⟨[c], by simp [elemsOf], fun x hx _ => by simp at hx; subst hx; exact hm⟩
  | true =>
    simp only [ite_true] at he
    split at he
    · cases hm with
      | @list _ _ ys hcl hys =>
        obtain ⟨h1, rfl⟩ := counted_ok (ys := ys) hcl he
        exact ⟨ys, by simp only [elemsOf]; exact h1, fun x hx hxn => Mem_stripOpt (hys x hx) hxn⟩
    · cases hm with
      | @members _ _ ys hcl hys _ =>
        obtain ⟨h1, rfl⟩ := counted_ok (ys := ys) hcl he
        exact ⟨ys, by simp only [elemsOf]; exact h1, fun x hx hxn => Mem_stripOpt (hys x hx) hxn⟩
    · cases hm with
      | @dict _ _ _ ps hcl _ hv _ =>
        obtain ⟨h1, rfl⟩ := counted_ok (ys := ps.map (·.2)) hcl he
        refine ⟨ps.map (·.2), by simp only [elemsOf]; exact h1, fun x hx hxn => ?_⟩
        obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hx
        exact Mem_stripOpt (hv p hp) hxn
    · simp at he

/-- The fields of a value of an enum a place reaches keep what they declare, where `keeps` says they
    do: the field of the value it is, among what that field can be. -/
theorem variant_fields_ok {elem : Abs} {x : Val} {values : List (String × List (String × Slot))}
    {out : Slot → Val → Bool} {keep : Abs → Slot → Bool}
    (ih : ∀ a s c, keep a s = true → Mem a c → out s c = false)
    (hm : Mem elem x) (hall : variantsKeep keep elem values = true) :
    variantOutside out values x = false := by
  cases x with
  | variant t fs =>
    simp only [variantOutside]
    rcases hf : values.find? (·.1 == t) with _ | ⟨t', fss⟩
    · rw [hf]; rfl
    · rw [hf]
      simp only [Option.map_some]
      have hmem := List.mem_of_find?_eq_some hf
      have ht' : t' = t := by have := List.find?_some hf; simpa using this
      subst ht'
      have hv := List.all_eq_true.mp hall _ hmem
      rw [List.any_eq_false]
      intro y hy
      split
      · rename_i fv hfv
        have hfvV : fieldV fs y.1 = some fv := by
          unfold fieldV; rw [hfv]; rfl
        cases elem with
        | variants vs =>
          cases hm with
          | @variant _ _ r _ hr _ hsome hmem' =>
            simp only [hr] at hv
            have hk := List.all_eq_true.mp hv _ hy
            obtain ⟨a, ha⟩ := Option.isSome_iff_exists.mp (hsome y.1 fv hfvV)
            rw [ha] at hk
            simp only [Option.getD_some] at hk
            rw [ih a y.2 fv hk (hmem' y.1 fv hfvV a ha)]
            simp
        | _ =>
          have hk := List.all_eq_true.mp hv _ hy
          rw [ih .any y.2 fv hk (Mem.any fv)]
          simp
      · simp
  | _ => rfl

/-- The keys of a dict a place reaches keep what their place declares, where `keysOK` says they do. -/
theorem keys_ok {keep : Abs → Slot → Bool} {out : Slot → Val → Bool} {keys : Option Slot} {sa : Abs} {c : Val}
    (ih : ∀ a s c, keep a s = true → Mem a c → out s c = false) (hm : Mem sa c) (hk : keysOK keep keys sa = true) :
    keysOut out keys c = false := by
  cases c with
  | dict ps =>
    simp only [keysOut]
    cases keys with
    | none => rfl
    | some ks =>
      simp only
      cases hm with
      | @dict _ ka _ _ _ hk' _ _ =>
        simp only [keysOK] at hk
        rw [List.any_eq_false]
        intro p hp
        simp [ih ka ks p.1 hk (hk' p hp)]
      | _ => simp [keysOK] at hk
  | _ => simp [keysOut]

/-- **What `keeps` says is so**: a value among those of a place that keeps what it declares is not
    outside it. -/
theorem keeps_sound : ∀ (k d : Nat) (a : Abs) (s : Slot) (reached : Bool) (c : Val),
    keepsD k a s reached = true → Mem a c → s.outsideD d reached c = false
  | 0, _, _, _, _, _, h, _ => by simp [keepsD] at h
  | _, 0, _, _, _, _, _, _ => by simp [Slot.outsideD]
  | k + 1, d + 1, a, .mk list count range unit01 record fields values keys _, reached, c, hk, hm => by
    simp only [Slot.outsideD]
    split
    · rfl
    · rename_i hcn
      have hm' := Mem_stripOpt hm hcn
      simp only [keepsD] at hk
      split at hk
      · rename_i hnv; rw [hnv] at hm'; exact absurd hm' Mem.not_never
      · rw [Bool.and_eq_true] at hk
        obtain ⟨hkeys, hk⟩ := hk
        -- the keys of a dict, where a parameter reaches them
        have hko : (reached && keysOut (fun s v => Slot.outsideD d s true v) keys c) = false := by
          cases reached with
          | false => rfl
          | true =>
            simp only [Bool.not_true, Bool.false_or] at hkeys
            simp only [Bool.true_and]
            exact keys_ok (fun a s c h1 h2 => keeps_sound k d a s true c h1 h2) hm' hkeys
        rw [hko, Bool.false_or]
        split at hk
        · simp at hk
        · rename_i hel
          obtain ⟨xs, hxs, hmx⟩ := elems_ok hm' hcn hel
          rw [hxs]
          simp only
          rw [List.any_eq_false]
          intro x hx
          split
          · simp
          · rename_i hxn; exact absurd (hmx x hx hxn) Mem.not_never
        · rename_i elem hnn hel
          obtain ⟨xs, hxs, hmx⟩ := elems_ok hm' hcn hel
          rw [hxs]
          simp only
          simp only [Bool.and_eq_true, Bool.or_eq_true, Bool.not_eq_true'] at hk
          obtain ⟨⟨⟨hin, hstr⟩, hrec⟩, hval⟩ := hk
          rw [List.any_eq_false]
          intro x hx
          split
          · simp
          · rename_i hxn
            have hmx' := hmx x hx hxn
            simp only [Bool.not_eq_true, Bool.or_eq_false_iff, Bool.and_eq_false_imp]
            refine ⟨⟨⟨range_ok hmx' hin, str_ok hmx' hstr⟩, ?_⟩, ?_⟩
            · intro hrr
              split
              · rename_i fs
                rcases hrec with hrec | hrec
                · exact absurd hrr (by simp_all)
                · rw [List.any_eq_false]
                  intro ⟨f, s'⟩ hf
                  split
                  · rename_i fv hfv
                    have hall := List.all_eq_true.mp hrec ⟨f, s'⟩ hf
                    simp only at hall
                    have hfield : (Val.record fs).field? f = some fv := by
                      simp only [Val.field?, hfv, Option.map_some]
                    have := keeps_sound k d _ s' true fv hall (Mem_field hmx' hfield)
                    simp only [this]; simp
                  · simp
              · simp
            · intro hrr
              rcases hval with hval | hval
              · exact absurd hrr (by simp_all)
              · exact variant_fields_ok (fun a s c h1 h2 => keeps_sound k d a s true c h1 h2) hmx' hval

/-- A value of an enum among those an abstract value `enumWithin` finds within `vs` is one of them. -/
theorem enumWithin_sound {vs : List String} {a : Abs} {x : String} (h : enumWithin vs a = true) (hm : Mem a (.enum x)) :
    vs.contains x = true := by
  cases a with
  | never => exact absurd hm Mem.not_never
  | set s =>
    cases s with
    | none => simp [enumWithin] at h
    | some s =>
      cases hm with
      | enum hx =>
        simp only [enumWithin, List.all_eq_true] at h
        exact h x hx
  | _ => simp [enumWithin] at h

/-- **A place the analysis shows takes no value it fails for.** -/
theorem shown_sound {p : Place} (hs : p.shown = true) {c : Val} (hm : Mem p.value c) : ¬ FailsAt p.what c := by
  unfold Place.shown at hs
  intro hf
  split at hs
  · rename_i r hw
    rw [hw] at hf
    obtain ⟨q, hq, hout⟩ := hf
    have hcn : c ≠ .none := by intro e; subst e; simp [Val.point] at hq
    have hmp := Mem_present hm hcn
    simp only [Bool.or_eq_true] at hs
    rcases hs with hs | hs
    · rcases hiv : p.value.present.iv with _ | i <;> rw [hiv] at hs <;> simp at hs
      have := within_sound (point_in hmp hq hiv) hs
      rw [not_outRange this.1 this.2] at hout; exact absurd hout (by simp)
    · split at hs
      · rename_i hnv; rw [hnv] at hmp; exact Mem.not_never hmp
      · simp at hs
  · rename_i s hw
    rw [hw] at hf
    simp only [FailsAt] at hf
    have := keeps_sound KEEPS D p.value s true c hs hm
    unfold Slot.outside at hf; rw [this] at hf; exact absurd hf (by simp)
  · rename_i s hw
    rw [hw] at hf
    simp only [FailsAt] at hf
    have := keeps_sound KEEPS D p.value s false c hs hm
    unfold Slot.outside at hf; rw [this] at hf; exact absurd hf (by simp)
  · rename_i hw
    rw [hw] at hf
    obtain ⟨r, rfl, hr⟩ := hf
    simp only [Bool.or_eq_true] at hs
    rcases hs with hs | hs
    · split at hs
      · rename_i hnv; rw [hnv] at hm; exact Mem.not_never hm
      · simp at hs
    · rcases hiv : p.value.iv with _ | i <;> rw [hiv] at hs <;> simp at hs
      have hin := Mem_iv_num hm hiv
      rcases hlo : i.lo with _ | l <;> rw [hlo] at hs <;> simp at hs
      have h1 := hin.lo; rw [hlo] at h1
      have : (0 : Rat) ≤ l := hs
      have : (0 : Rat) ≤ r := Rat.le_trans this h1
      have := (@Rat.num_nonneg r).mpr this
      omega
  · rename_i hw
    rw [hw] at hf
    obtain ⟨r, rfl, hr⟩ := hf
    rw [okNum_sound hm hs] at hr; exact absurd hr (by simp)
  · rename_i hw
    rw [hw] at hf
    simp only [FailsAt] at hf
    subst hf
    simp only [Bool.or_eq_true] at hs
    rcases hs with hs | hs
    · split at hs
      · rename_i hnv; rw [hnv] at hm; exact Mem.not_never hm
      · simp at hs
    · rcases hiv : p.value.iv with _ | i <;> rw [hiv] at hs <;> simp at hs
      exact hasZero_sound (Mem_iv_num hm hiv) hs rfl
  · simp at hs
  · rename_i holds hw
    rw [hw] at hf
    simp only [FailsAt] at hf
    rw [hf] at hs; exact absurd hs (by simp)
  · rename_i vs hw
    rw [hw] at hf
    obtain ⟨x, rfl, hout⟩ := hf
    have hmp := Mem_present hm (by intro h; cases h)
    rw [enumWithin_sound hs hmp] at hout
    exact absurd hout (by simp)

/-- **Where every place is shown and every function called does not fail, the evidence of a
    failure is none.** -/
theorem no_wit {C : Ctx} {o : Out} (hs : ∀ p ∈ o.places, p.shown = true) (hc : ∀ g ∈ o.calls, NoFail C g) :
    ¬ Wit C o := by
  rintro (⟨p, hp, (⟨c, hm, hf⟩ | ⟨ho, hn⟩)⟩ | ⟨g, hg, hn⟩)
  · exact shown_sound (hs p hp) hm hf
  · have := hs p hp
    unfold Place.shown at this
    rw [ho] at this
    rw [hn] at this; exact absurd this (by simp)
  · exact hn (hc g hg)

end KanameCore
