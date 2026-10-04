/-
  What the business-day operations give (koyomi's DESIGN 1.8, 2.2), whatever the calendar says.

  - `roll following` gives the first business day on or after the day, and `roll preceding` the
    last one on or before it (`seek_following`, `seek_preceding`).
  - A day rolled by any of the four conventions is a business day (`roll_open`).
  - A day reached by counting business days is a business day, `+ 0` and `- 0` included
    (`addBusiness_open`): DESIGN 1.8 keeps "a day counted in business days is a business day" when
    an integer input brings a 0.
  - `if closed` touches only a closed day (`ifClosed_open`).

  The calendar is any `Cal`, so these hold for every table of holidays there is.
-/
import KoyomiModel.Ops

namespace KoyomiModel

theorem plus_ok {z n w : Int} (h : plus z n = .ok w) : w = z + n := by
  unfold plus at h
  by_cases hc : z + n < minDay ∨ z + n > maxDay
  · simp [hc] at h
  · simp [hc] at h
    exact h.symm

/-- What `seek` finds is a business day. -/
theorem seek_open {cal : Cal} {fwd : Bool} : ∀ {n : Nat} {z x : Int}, seek cal fwd n z = .ok x → cal x = .ok true := by
  intro n
  induction n with
  | zero => intro z x h; simp [seek] at h
  | succ n ih =>
    intro z x h
    unfold seek at h
    cases hc : cal z with
    | error e => rw [hc] at h; cases h
    | ok b =>
      rw [hc] at h
      cases b with
      | true => cases h; exact hc
      | false =>
        simp only at h
        cases hp : plus z (step fwd) with
        | error e => rw [hp] at h; cases h
        | ok w => rw [hp] at h; exact ih h

/-- **`roll following`**: the day it gives is on or after `z`, a business day, and every day from
    `z` up to it is closed — the first business day on or after `z`. -/
theorem seek_following {cal : Cal} : ∀ {n : Nat} {z x : Int}, seek cal true n z = .ok x →
    z ≤ x ∧ cal x = .ok true ∧ ∀ y, z ≤ y → y < x → cal y = .ok false := by
  intro n
  induction n with
  | zero => intro z x h; simp [seek] at h
  | succ n ih =>
    intro z x h
    have hopen := seek_open h
    unfold seek at h
    cases hc : cal z with
    | error e => rw [hc] at h; cases h
    | ok b =>
      rw [hc] at h
      cases b with
      | true =>
        cases h
        exact ⟨Int.le_refl _, hc, fun y h1 h2 => absurd h2 (by omega)⟩
      | false =>
        simp only at h
        cases hp : plus z (step true) with
        | error e => rw [hp] at h; cases h
        | ok w =>
          rw [hp] at h
          have hw : w = z + 1 := by rw [plus_ok hp]; rfl
          obtain ⟨h1, _, h3⟩ := ih h
          refine ⟨by omega, hopen, ?_⟩
          intro y hy1 hy2
          rcases Int.lt_or_le y w with hlt | hge
          · have : y = z := by omega
            rw [this]; exact hc
          · exact h3 y hge hy2

/-- **`roll preceding`**: the day it gives is on or before `z`, a business day, and every day
    after it up to `z` is closed — the last business day on or before `z`. -/
theorem seek_preceding {cal : Cal} : ∀ {n : Nat} {z x : Int}, seek cal false n z = .ok x →
    x ≤ z ∧ cal x = .ok true ∧ ∀ y, x < y → y ≤ z → cal y = .ok false := by
  intro n
  induction n with
  | zero => intro z x h; simp [seek] at h
  | succ n ih =>
    intro z x h
    have hopen := seek_open h
    unfold seek at h
    cases hc : cal z with
    | error e => rw [hc] at h; cases h
    | ok b =>
      rw [hc] at h
      cases b with
      | true =>
        cases h
        exact ⟨Int.le_refl _, hc, fun y h1 h2 => absurd h2 (by omega)⟩
      | false =>
        simp only at h
        cases hp : plus z (step false) with
        | error e => rw [hp] at h; cases h
        | ok w =>
          rw [hp] at h
          have hw : w = z - 1 := by rw [plus_ok hp]; simp [step]; omega
          obtain ⟨h1, _, h3⟩ := ih h
          refine ⟨by omega, hopen, ?_⟩
          intro y hy1 hy2
          rcases Int.lt_or_le w y with hlt | hge
          · have : y = z := by omega
            rw [this]; exact hc
          · exact h3 y hy1 hge

/-- **Every convention gives a business day.** -/
theorem roll_open {cal : Cal} {z x : Int} (c : Conv) (h : roll cal z c = .ok x) : cal x = .ok true := by
  cases c with
  | following => exact seek_open h
  | preceding => exact seek_open h
  | modifiedFollowing =>
    unfold roll at h
    cases hf : seek cal true fuel z with
    | error e => rw [hf] at h; cases h
    | ok f =>
      rw [hf] at h
      simp only at h
      split at h
      · cases h; exact seek_open hf
      · exact seek_open h
  | modifiedPreceding =>
    unfold roll at h
    cases hp : seek cal false fuel z with
    | error e => rw [hp] at h; cases h
    | ok p =>
      rw [hp] at h
      simp only at h
      split at h
      · cases h; exact seek_open hp
      · exact seek_open h

theorem count_open {cal : Cal} {fwd : Bool} {n : Nat} : ∀ {f : Nat} {z x : Int} {k : Nat},
    count cal fwd n f z k = .ok x → cal x = .ok true := by
  intro f
  induction f with
  | zero => intro z x k h; simp [count] at h
  | succ f ih =>
    intro z x k h
    unfold count at h
    cases hp : plus z (step fwd) with
    | error e => rw [hp] at h; cases h
    | ok w =>
      rw [hp] at h
      simp only at h
      cases hc : cal w with
      | error e => rw [hc] at h; cases h
      | ok b =>
        rw [hc] at h
        cases b with
        | true =>
          simp only at h
          split at h
          · cases h; exact hc
          · exact ih h
        | false => exact ih h

/-- **A day counted in business days is a business day**, `± 0 business days` included. -/
theorem addBusiness_open {cal : Cal} {z x : Int} {n : Nat} {fwd : Bool}
    (h : addBusiness cal z n fwd = .ok x) : cal x = .ok true := by
  unfold addBusiness at h
  split at h
  · exact seek_open h
  · exact count_open h

/-- **`if closed` leaves a business day as it is.** -/
theorem ifClosed_open {cal : Cal} {vals : Array Int} {inner : Op} {z : Int} (ho : cal z = .ok true) :
    (Op.ifClosed inner).apply cal vals z = .ok z := by
  simp [Op.apply, ho]

end KoyomiModel
