# should-update: Prop Memoization for eye-declare

## Status
In Progress

## Objective
Add opt-in prop memoization to eye-declare so components can skip re-rendering when their props haven't changed. The default behavior (always re-render) is preserved; components opt in via `#[component(memo)]` and `#[props]` auto-deriving `PartialEq`.

## Wave Map
| Wave | Steps | Can Parallelize | Depends On |
|------|-------|-----------------|------------|
| 0 | 01, 02 | Yes | — |
| 1 | 03 | No | Wave 0 |
| 2 | 04, 05 | Yes | Wave 1 |
| 3 | 06 | No | Wave 2 |

## Dependency Table
| Step | File(s) | Depends On | Depended By |
|------|---------|------------|-------------|
| 01 | `component.rs`, `node.rs`, `element.rs`, `renderer.rs` | — | 03, 04, 05 |
| 02 | `props.rs` (macro crate) | — | 03 |
| 03 | `component.rs` (macro crate) | 01, 02 | 04, 05 |
| 04 | `children.rs`, `components/markdown.rs`, `components/text.rs`, `components/spinner.rs` | 01, 03 | 06 |
| 05 | `lib.rs` (macro crate), `lib.rs` (eye_declare) | 01, 03 | 06 |
| 06 | tests | 04, 05 | — |

## Orchestrator Protocol
1. Read this file to identify current wave
2. Dispatch all steps in current wave in parallel
3. After each step: run acceptance criteria; mark complete only if all pass
4. Advance to next wave only when all steps in current wave are complete
5. Blockers: stop and report with full context

## Subagent Contract
- Workers: Read step file fully before acting. Implement only what the step specifies.
- Workers: Run `cargo test -p eye_declare` and `cargo clippy -p eye_declare -- -D warnings` before reporting complete.
- Workers: Report back: "Step NN complete (cargo test passed)" or "Step NN FAILED: <reason>"
- Reviewers: Run acceptance criteria verbatim. Pass or fail with evidence.

## Steps
- [ ] [step-01-trait-and-renderer](./step-01-trait-and-renderer.md) — Add `should_update` to Component/AnyComponent, change Element::update to return bool, gate force_dirty in reconcile_children
- [ ] [step-02-props-macro-partialeq](./step-02-props-macro-partialeq.md) — Update #[props] macro to auto-derive PartialEq and generate PropsMemo impl
- [ ] [step-03-component-macro-memo](./step-03-component-macro-memo.md) — Add `memo` attribute to #[component], generate should_update override using PropsMemo
- [x] [step-04-builtin-components](./step-04-builtin-components.md) — Opt in built-in components (Markdown, Text, Spinner) and add PartialEq to supporting types
- [ ] [step-05-exports-and-docs](./step-05-exports-and-docs.md) — Re-export PropsMemo, document memo attribute and no_memo escape hatch
- [ ] [step-06-tests](./step-06-tests.md) — Integration tests for memoization behavior
