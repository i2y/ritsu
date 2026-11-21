### sekisho diff: gate `claims` v1 → v2

- Old: `tests/diff/claims.gate`
- New: `tests/diff/claims_v2.gate`
- Lines under the workflows the two versions write differently (no answer of the gate reads them; `ritsu check` holds the flows' calls to them):
  - workflow pay, resource Claim from claim_id: amount = amount → amount = asked
  - workflow pay, resource Claim from claim_id: rush = rush (only in the new version)
  - workflow pay, resource Claim from claim_id: checked = checked (only in the old version)

#### `pay_claim`

2 combinations walked over both versions.

For every combination, every action is decided the same.
