import Kaname.refund_split

namespace Proofs.refund_split

open Kaname Kaname.refund_split

/-- The shares, each the refund up to its line's end less the refund before it, telescope to the
refund of the whole. -/
theorem shares_add_up : Kaname.refund_split.shares_add_up := by
  intro lines refund hlen _ hwithin _ _
  obtain ⟨S, hS⟩ : ∃ S, S = (lines.map fun l => l.left).sum := ⟨_, rfl⟩
  have hT : 0 < S := hS ▸ sum_pos hlen fun l hl => (hwithin l hl).1
  have key : (split lines refund).map (fun s => s.share)
      = (runningTotals lines (fun l => l.left)).map (fun r => allocate refund r.upto S - allocate refund r.before S) := by
    unfold split
    rw [List.map_map]
    apply List.map_congr_left
    intro r hr
    simp only [Function.comp, runningTotals_total _ _ r hr, hS]
  rw [key, runningTotals_telescope lines (fun l => l.left) (fun x => allocate refund x S), ← hS]
  rw [allocate_self refund hT, allocate_zero]
  omega

/-- A share is the refund up to its line's end less the refund before it, and `allocate` does not go
down as its share grows. -/
theorem no_share_is_negative : Kaname.refund_split.no_share_is_negative := by
  intro lines refund hlen _ hwithin hr0 _ s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  rw [List.mem_map] at hs
  obtain ⟨r, hr, rfl⟩ := hs
  have hw : ∀ l ∈ lines, 0 ≤ (fun (l : Line) => l.left) l := fun l hl => by
    have := (hwithin l hl).1
    show 0 ≤ l.left
    omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hmono := allocate_mono (t := refund) hr0 (htot ▸ hT) (show r.before ≤ r.upto by
    have hitem : r.item ∈ lines := by
      have := List.mem_map_of_mem (f := (·.item)) hr
      rwa [runningTotals_items] at this
    have := (hwithin r.item hitem).1
    omega)
  show allocate refund r.upto r.total - allocate refund r.before r.total ≥ 0
  omega

/-- What one line gets is at most its width of the whole, when the refund is no more than the whole. -/
theorem no_share_is_more_than_its_left : Kaname.refund_split.no_share_is_more_than_its_left := by
  intro lines refund hlen _ hwithin hr0 _ hle s hs
  have hT : 0 < (lines.map fun l => l.left).sum := sum_pos hlen fun l hl => (hwithin l hl).1
  unfold split at hs
  rw [List.mem_map] at hs
  obtain ⟨r, hr, rfl⟩ := hs
  have hw : ∀ l ∈ lines, 0 ≤ (fun (l : Line) => l.left) l := fun l hl => by
    have := (hwithin l hl).1
    show 0 ≤ l.left
    omega
  have ⟨hwide, _, _⟩ := mem_runningTotals hw r hr
  have htot := runningTotals_total _ _ r hr
  have hitem : r.item ∈ lines := by
    have := List.mem_map_of_mem (f := (·.item)) hr
    rwa [runningTotals_items] at this
  have := (hwithin r.item hitem).1
  have hsub := allocate_sub_le (t := refund) hr0 (htot ▸ hle) (htot ▸ hT) (show r.before ≤ r.upto by omega)
  show allocate refund r.upto r.total - allocate refund r.before r.total ≤ r.item.left
  omega

end Proofs.refund_split
