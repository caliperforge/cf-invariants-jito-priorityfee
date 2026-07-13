# Multi-seed reachability certification (cf-invariants-jito-priorityfee)

## Status

**Uncovered: 0 of 1 planted class certified today.** Framework files
land in this commit; the deterministic regression bin the planted
crate needs is queued as a follow-up spike per crypto-contributor
`T-reachability-solana-jito-regression-bins-spike-2026-07-13`.

The base planted CI continues to run the Crucible fuzz leg per commit.
Reachability adds an orthogonal deterministic-regression receipt on
top; it does not replace the fuzz leg.

## What Shape A is

Shape A per crypto-contributor design proposal
`D-solana-reachability-leg-shape-2026-07-13` (Director-ratified):
each planted crate ships a `src/bin/regression.rs` that reads
`REACHABILITY_SEED`, drives the fixture actions via `StdRng`, and
prints the class-specific `INVARIANT VIOLATED <name>` marker on
divergence. `ci/reachability_leg.sh` iterates the 16-seed canonical
set (`ci/reachability_seeds.txt`, byte-identical to sibling repos)
and requires rc!=0 + marker on all 16
(fail-on-any-clean-seed does-not-merge).

Reference implementation:
`caliperforge/solana-invariant-atlas:references/collateral_mint_ref_planted/fuzz/collateral_mint_ref/src/bin/regression.rs`.

## Planted classes in scope (need regression bins)

| planted crate | class | regression bin status |
| --- | --- | --- |
| `jito_pfd_ref_planted_transfer_increments_total` | balance_conservation (transfer_increments_total) | absent (spike required) |

The single planted crate today does not ship `src/bin/regression.rs`
under `references/<name>/fuzz/<name>/src/bin/`.

## Spike scope (queued)

For this planted class the spike adds:

1. `Cargo.toml` `[[bin]]` entry for `regression`.
2. `Cargo.toml` `[dependencies]` addition: `rand = "0.8"`.
3. `src/bin/regression.rs`: `parse_seed_env()` +
   `keypair_from_rng(rng)` scaffolding (copy verbatim from the
   Solana atlas reference implementation), followed by the
   class-specific deterministic sequence:
   - transfer_increments_total: initialize priority-fee vault →
     transfer(amount) → assert Δ vault_total == amount.
4. When `REACHABILITY_SEED` is absent, fallback to fixed values so
   normal `cargo run --release --bin regression` remains
   developer-friendly.

Correctness requires reading the fixture's `#[fuzz_fixture]` block
and the underlying Jito priority-fee-distribution program to pick
the right instruction order + account setup.

## What lands today

- `ci/reachability_seeds.txt` — canonical 16-seed set (byte-identical
  to sibling repos).
- `docs/reachability.md` — this file.

No workflow changes; no README verdict block.

## Merge-gate rule (target)

Once the regression bin exists + the `reachability` job lands, no
new planted twin merges to `main` unless the leg exits green
(fail-on-all-N). k/N certification number moves into the top-level
README verdict block at that point.

## Reuse

Canonical scripts: `caliperforge/crypto-contributor:scripts/reachability/`.
Sibling ecosystem references:
`caliperforge/soroban-invariant-atlas:ci/reachability_leg.sh`
and
`caliperforge/solana-invariant-atlas:ci/reachability_leg.sh`
(closest per-ecosystem fit).
