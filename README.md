# cf-invariants-jito-priorityfee

[![ci](https://github.com/caliperforge/cf-invariants-jito-priorityfee/actions/workflows/ci.yml/badge.svg)](https://github.com/caliperforge/cf-invariants-jito-priorityfee/actions/workflows/ci.yml)

**An invariant-fuzzing harness for the [Jito priority-fee-distribution program](https://github.com/jito-foundation/jito-programs/tree/master/mev-programs/programs/priority-fee-distribution), run on [Crucible](https://github.com/asymmetric-research/crucible).**

cf-invariants-jito-priorityfee is a focused harness, not a new fuzzer.
It ports the upstream Jito priority-fee-distribution program from
`anchor-lang` 0.31.1 to `anchor-lang` 1.0.1 so it can be driven by
Crucible v0.2.0 (LibAFL + LiteSVM), then runs an invariant class
against a clean reference and a single-site planted-bug twin. Every
push, CI rebuilds both program variants and asserts `clean = 0`
violations and `planted >= 1` violation.

This is a sibling artifact to
[cf-invariants-jito](https://github.com/caliperforge/cf-invariants-jito)
(Jito tip-distribution) and
[cf-invariants-jito-tippayment](https://github.com/caliperforge/cf-invariants-jito-tippayment)
(Jito tip-payment), shipped by the same operator. It is the *third*
real Jito program harnessed under the same anchor-lang 1.0.1 /
Crucible v0.2.0 / platform-tools v1.52 rails — proof that the rails
generalize beyond a single target.

---

## Scope — what Jito priority-fee-distribution is, what this harness covers

The Jito priority-fee-distribution program is the on-chain piece of
the [Jito](https://www.jito.network/) MEV-redistribution stack on
Solana that handles the *priority-fee* portion of validator-staker
distributions, distinct from the tip-distribution program that handles
the *tip* portion. Validators credit priority-fee lamports into a
per-epoch `PriorityFeeDistributionAccount` (PFDA) via
`transfer_priority_fee_tips`; once a merkle root is uploaded, stakers
claim against it.

The PFDA tracks a `total_lamports_transferred` field — a structural
running sum of every successful `transfer_priority_fee_tips` call's
`lamports` argument. This field exists on
`priority-fee-distribution`'s `PriorityFeeDistributionAccount` but
NOT on tip-distribution's `TipDistributionAccount`, making it
specific to this program's value-add.

The upstream code lives at
`jito-foundation/jito-programs/mev-programs/programs/priority-fee-distribution`
and is licensed Apache-2.0.

This harness does not modify the production program. It targets the
**invariant surface** of `transfer_priority_fee_tips` — the structural
property that must hold no matter what call sequence is fuzzed — and
proves the harness can confirm the property on the clean reference
and catch a deliberately planted regression.

## What it tests — one invariant class

| Class | Invariant under test | Planted-bug site |
|---|---|---|
| `transfer_increments_total_state_update` | `invariant_transfer_priority_fee_tips_increments_total` — after every successful `transfer_priority_fee_tips(lamports)` call, on-chain `PriorityFeeDistributionAccount.total_lamports_transferred` increases by exactly `lamports` (sum of all successful calls equals the fixture-side oracle). | `programs/priority-fee-distribution/src/lib.rs::transfer_priority_fee_tips` — the `ctx.accounts.priority_fee_distribution_account.increment_total_lamports_transferred(lamports)?;` call is dropped. The instruction's system-program lamport transfer still runs normally, so Solana's runtime balance check is not tripped; only the structural commit of the new total to the PFDA state never happens. |

**Why this invariant, not lamport conservation.** A natural first
pick for any payment program is "total lamports across `{from, pfda,
expired_funds_account}` are conserved by every successful call." It
does not work as a Crucible invariant because the Solana SVM runtime
enforces total-lamport conservation across every instruction
natively. Any program that fails to balance debits and credits has
its tx rejected by the runtime *before* the fixture's `read_account`
sees a discrepancy — so the invariant cannot meaningfully observe a
violation. A user-meaningful structural invariant for this program
has to live in the post-instruction state shape that the runtime does
NOT police. `PriorityFeeDistributionAccount.total_lamports_transferred`
— whether the on-chain running sum actually commits — is exactly that
shape, and it's *priority-fee-specific*, not shared with the
tip-distribution sibling.

CI result on the published commit: `clean = 0` violations and
`planted >= 1` violation. The CI badge is the source of truth — if
it is red, the harness is broken.

## Repository layout

```
.
├── programs/priority-fee-distribution/      # cf-invariants-jito-priorityfee port (anchor-lang 1.0.1)
├── programs/vote-state/                     # vendored jito-programs-vote-state (anchor-lang 1.0.1)
├── references/
│   ├── jito_pfd_ref/                        # clean baseline + Crucible fuzz fixture
│   │   ├── programs/priority-fee-distribution/   # ported program (== port above)
│   │   ├── programs/vote-state/                  # vendored helper
│   │   └── fuzz/jito_transfer_priority_fee_total/ # fuzz fixture
│   └── jito_pfd_ref_planted_transfer_increments_total/
│       ├── programs/priority-fee-distribution/   # planted variant (1-line drop)
│       ├── programs/vote-state/
│       └── fuzz/jito_transfer_priority_fee_total/ # synced fixture (same code as clean)
├── .github/workflows/ci.yml                 # CI: workspace check + build-sbf + harness matrix
├── Cargo.toml                               # workspace
├── LICENSE                                  # Apache-2.0 (CaliperForge)
├── NOTICE                                   # Jito attribution + modification log
└── README.md
```

The fuzz-fixture source for the invariant lives once under
`references/jito_pfd_ref/fuzz/jito_transfer_priority_fee_total/src/main.rs`;
CI copies the same source into the planted variant before the run, so
the only difference between a clean run and the planted run is the
`.so` binary loaded into LiteSVM.

## Pinned toolchain

These are the versions CI builds against on every push (see
[`.github/workflows/ci.yml`](./.github/workflows/ci.yml)). Pins are
inherited from the sister cf-invariants-jito / cf-invariants-jito-tippayment
projects' CI-green stack:

- Rust **stable**.
- `anchor-lang` **1.0.1** — matches Crucible v0.2.0's workspace.
- Upstream [Crucible](https://github.com/asymmetric-research/crucible) **v0.2.0** — built from source in CI (`cargo install --path crates/crucible-fuzz-cli`).
- Anza / Solana CLI **v2.1.21** for `cargo-build-sbf`.
- Solana platform-tools **v1.52** (passed as `--tools-version v1.52`;
  Crucible v0.2.0 deps require `edition2024` support, which earlier
  platform-tools' rustc cannot build).
- `solana-sha256-hasher` **3** (modular replacement for upstream's
  `solana_program::hash::hashv` — the only solana_program submodule
  the anchor-lang 1.0.1 shim no longer re-exports).
- `solana-sdk-ids` **3** (transitively, for the vote-state crate's
  `vote::id()` check).

The fuzz `Cargo.toml` references Crucible via path dep at
`../../../../../crucible/...`, i.e. a sibling directory to `port/`.
CI clones Crucible v0.2.0 to that sibling path before the harness
step. For local reproduction, do the same (or symlink an existing
checkout of Crucible v0.2.0 at that path).

## Reproduce from a fresh clone

CI runs exactly the steps below on every push. Local reproduction is
optional and requires the toolchain above installed and on `PATH`.

```sh
# 1. Clone this repo + Crucible v0.2.0 as a sibling.
git clone https://github.com/caliperforge/cf-invariants-jito-priorityfee.git
git clone --depth 1 --branch v0.2.0 \
    https://github.com/asymmetric-research/crucible.git
cd cf-invariants-jito-priorityfee

# 2. Workspace check (also runs in CI as the workspace-check job).
cargo check --workspace --locked || cargo check --workspace

# 3. Build the cf-invariants-jito-priorityfee port (SBPF).
cargo build-sbf --tools-version v1.52 \
    --manifest-path programs/priority-fee-distribution/Cargo.toml

# 4. Build the clean reference + planted twin.
for variant in jito_pfd_ref \
               jito_pfd_ref_planted_transfer_increments_total; do
    cargo build-sbf --tools-version v1.52 \
        --manifest-path "references/${variant}/programs/priority-fee-distribution/Cargo.toml"
done

# 5. Build + install Crucible CLI from source.
(cd ../crucible && cargo install --path crates/crucible-fuzz-cli --locked)

# 6. Run the harness on the clean pair (expect no FUZZ_FINDING line).
(cd references/jito_pfd_ref/fuzz/jito_transfer_priority_fee_total && \
    crucible run jito_priority_fee_distribution \
        invariant_transfer_priority_fee_tips_increments_total \
        --release --timeout 30)

# 7. Same invariant against the planted twin (expect a FUZZ_FINDING within ~1s).
(cd references/jito_pfd_ref_planted_transfer_increments_total/fuzz/jito_transfer_priority_fee_total && \
    crucible run jito_priority_fee_distribution \
        invariant_transfer_priority_fee_tips_increments_total \
        --release --timeout 30)
```

CI runs steps 2 through 7 on every push. The scorecard captures (raw
Crucible output, ANSI-stripped) are uploaded as the
`crucible-scorecards` workflow artifact and written under
`findings/<invariant>_<variant>/scorecard.md` inside the runner.
`findings/` is gitignored; the CI artifact is the canonical record.
See [`.github/workflows/ci.yml`](./.github/workflows/ci.yml) for the
canonical sequence.

## What this is not

- **Not a fork of Crucible.** Crucible is the harness;
  cf-invariants-jito-priorityfee is a target + fuzz fixture that runs
  on top of it. Credit for the LiteSVM execution rails and the
  IDL-driven fuzzing plumbing belongs to Asymmetric Research.
- **Not a Jito security audit.** The planted twin is a synthetic
  single-site regression authored to prove the corresponding
  invariant class fires. No claim is made about the production Jito
  program's security from this harness alone.
- **Not a formal-verification tool.** Randomized invariant fuzzing,
  not proofs.

## Credits

- Upstream priority-fee-distribution program: [Jito Foundation](https://www.jito.network/) — `jito-foundation/jito-programs` (Apache-2.0).
- Fuzz harness: [Crucible](https://github.com/asymmetric-research/crucible) by [Asymmetric Research](https://www.asymmetric.re/) (MIT, v0.2.0).
- Anchor framework: [coral-xyz/anchor](https://github.com/coral-xyz/anchor) (Apache-2.0).

## Reporting issues, security contact

Open an issue on this GitHub repository, or contact
[michael@caliperforge.com](mailto:michael@caliperforge.com).

## License

Apache-2.0. See [`LICENSE`](./LICENSE) and [`NOTICE`](./NOTICE). The
`NOTICE` file preserves Jito's upstream Apache-2.0 attribution and
describes the modifications relative to upstream.

---

cf-invariants-jito-priorityfee is operated by Michael Moffett under the
CaliperForge banner. CaliperForge is a sole-operator engineering studio.

This scaffold was built with AI assistance. Authored and reviewed by
Michael Moffett, operator at CaliperForge. Full policy at
[caliperforge.com/ai-disclosure](https://caliperforge.com/ai-disclosure).

[caliperforge.com](https://caliperforge.com)
