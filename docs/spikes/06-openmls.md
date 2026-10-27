# Spike 6: OpenMLS evaluation

**Date:** 2026-10-21 · **Status:** Not adopted. Evaluated, then rejected by [ADR-0005](../adr/0005-private-space-encryption.md) on 2026-10-27. Kept as evidence. Do not add OpenMLS to the workspace from this spike.

The notes below are the evaluation as it stood. The provisional "proceed" recommendation at the bottom is historical and is not the decision.

**Code:** [`spikes/openmls-eval`](../../spikes/openmls-eval) (throwaway, own lockfile, not part of the workspace build).

**Versions tested:** `openmls` 0.9.1, `openmls_rust_crypto` 0.6.0, `openmls_basic_credential` 0.6.0. Ciphersuite `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`. Rust 1.97.0.

## Pass conditions from STACK.md section 12

| Criterion | Result | Status |
|:--|:--|:--|
| Group lifecycle works (create, add, message, remove) | Yes; all behavior checks pass | Done |
| Removal semantics match ADR-0001 | Yes (see below) | Done |
| Sizes and timing at our target scale | Measured (see below) | Done |
| Memory suitable for an iOS Notification Service Extension | 5.2 MiB peak RSS for a 3-member lifecycle on Linux x86-64 | **Not verified on device** |
| Builds for the browser | Compiles for `wasm32-unknown-unknown` (about 3.4 MB raw, no `wasm-opt`) | **Not run in a browser** |
| Maintenance status | Active (frequent upstream changes; maintained by Phoenix R&D and CE Labs per the upstream README) | Done |
| Audit status | No third-party audit of OpenMLS found; the upstream README makes no audit statement | **Open** |
| Interop with another MLS implementation | Not tested | **Open** |

## Behavior relevant to ADR-0001 (leased shares and removal)

Scenario: Alice creates a group, adds Bob, they exchange messages, Alice adds Carol, then Alice removes Bob and sends another message.

- Carol (still a member) decrypts the message sent after the removal: **yes**.
- Bob, who was offline and has not processed the removal commit, cannot decrypt it: **correct, he fails**.
- Bob, after processing the removal commit, sees his group become inactive: **yes**.

This is the property share Spaces rely on: removing a member stops them from reading what comes next. As ADR-0001 says, it does not un-send what they already received.

## Sizes (bytes) and timing

Single native process, release build, x86-64 Linux. Timings include generating keys for every member.

| Item | Size |
|:--|:--|
| Key package | 275 |
| Welcome, 2 members | 787 |
| Commit adding a third member | 803 |
| Welcome, 3 members | 1,031 |
| Application message overhead (32-byte plaintext) | 145 |
| Removal commit (3-member group) | 495 |

Adding `n - 1` members in one commit, then one self-update by the creator:

| Members | Add commit | Welcome | First update commit | Time (all members' keys included) |
|:--|:--|:--|:--|:--|
| 2 | 682 | 786 | 488 | 3 ms |
| 10 | 3,001 | 3,590 | 1,249 | 9 ms |
| 100 | 28,127 | 33,142 | 8,734 | 52 ms |
| 500 | 139,795 | 164,080 | 41,610 | 325 ms |

Sizes are comfortable for the v1 target of hundreds per group. The first update commit grows roughly linearly here, which is expected right after a bulk add (new members are not yet "healed" into the tree). Steady-state cost after members have updated was **not measured**; it should be logarithmic in theory and needs confirming.

Memory: 5.2 MiB peak RSS for the 3-member lifecycle, and 14 MiB peak at 500 members, on Linux x86-64. iOS Notification Service Extensions have a tight memory limit (commonly reported around 24 MiB; verify against current Apple documentation and on a real device). Native Linux numbers are an encouraging signal, not a pass.

## Dependencies and policy findings

- The dependency tree is **128 unique crates** natively and **138** for wasm32. That is a real addition to the dependency budget in ADR-0003.
- Run against this repository's `deny.toml`:
  - **Licenses:** four `hpke-rs*` crates (0.7.0) are **MPL-2.0**, which is not in our allow-list. MPL-2.0 is file-level copyleft and compatible with our licensing, but it must be an explicit decision. Proposed: allow it by per-crate exception for `hpke-rs`, `hpke-rs-crypto`, `hpke-rs-libcrux`, and `hpke-rs-rust-crypto`, not globally.
  - **Advisories:** `proc-macro-error2` 2.0.1 is flagged as **unmaintained** (RUSTSEC-2026-0173). It is a build-time proc macro reached through `hax-lib-macros`, `libcrux-sha3`, and `hpke-rs`. There is no vulnerability, but the policy fails on it. If we adopt OpenMLS, add a documented ignore with a revisit date.
  - Bans and sources pass.
- Neither change has been made to the root `deny.toml`; they should land with the commit that adds the dependency.

## Browser build notes

`wasm32-unknown-unknown` needs two things beyond a plain build: OpenMLS's `js` feature, and the `js` feature on `getrandom` 0.2 (a transitive dependency still uses it). OpenMLS also requires the browser's JavaScript APIs for randomness and time, which fits the Dioxus web client. The wasm output was compiled, **not executed**.

## Risks and open items

1. **Audit.** No third-party audit found. Ask the maintainers directly, and keep the pre-1.0 independent review in the plan with OpenMLS explicitly in scope.
2. **API churn.** The crate is 0.x with frequent releases. Pin exact versions and hide OpenMLS behind a trait in `ambient-spaces` so it can be replaced.
3. **iOS extension memory** on a real device, with the real core linked in (spike 1).
4. **Browser runtime**, including storage, in spike 3.
5. **Interop** with a second implementation (for example `mls-rs`) to catch spec-interpretation bugs.
6. **Persistence.** This spike uses in-memory storage. Group state must persist through `ambient-store`; OpenMLS has a storage trait and an SQLite storage crate to evaluate.
7. **Steady-state commit size** after a group has healed.
8. **Dependency weight and license policy** (above).

## Recommendation

Proceed with OpenMLS as the provisional MLS implementation. It meets the functional bar and has a plausible size, speed, and memory profile. Do not treat spike 6 as closed until items 1, 3, 4, and 5 are done. If the audit status cannot be established, `mls-rs` is the alternative to evaluate in the same way.
