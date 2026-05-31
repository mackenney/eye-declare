# Step 04: Opt In Built-in Components and Supporting Types

## Context
### Overall Objective
Add opt-in prop memoization to eye-declare. Built-in components that benefit from memoization should opt in via `#[component(memo)]`.

### This Step
1. Add `PartialEq` derives to supporting types needed by memoized components
2. Add `memo` to `#[component(...)]` attributes for Markdown, Spinner, and selected simple components
3. Canvas is explicitly excluded (closures prevent `PartialEq`)
4. Text is a data-children component — evaluate and handle appropriately

### Why This Order
Depends on step 01 (trait plumbing), step 02 (`#[props]` generating `PartialEq` + `PropsMemo`), and step 03 (`#[component(memo)]` codegen). This step applies the infrastructure to actual components.

## Files to Read Before Starting
- `crates/eye_declare/src/components/markdown.rs` — Markdown component (highest-value target)
- `crates/eye_declare/src/components/text.rs` — Text component (data-children)
- `crates/eye_declare/src/components/spinner.rs` — Spinner component
- `crates/eye_declare/src/components/canvas.rs` — Canvas component (no change)
- `crates/eye_declare/src/children.rs` — `DataChildren<T>` (needs PartialEq)
- `crates/eye_declare/src/component.rs` — VStack, HStack, Column definitions

## Implementation

### Task 1: Add `PartialEq` to `DataChildren<T>`

**File:** `crates/eye_declare/src/children.rs`

Find the `DataChildren<T>` struct definition. It's currently:
```rust
pub struct DataChildren<T>(Vec<T>);
```

Add `PartialEq` derive. Change to:
```rust
#[derive(PartialEq)]
pub struct DataChildren<T>(Vec<T>);
```

This enables `PartialEq` when `T: PartialEq`, which is needed for data-children components that want memo.

### Task 2: Opt in Markdown

**File:** `crates/eye_declare/src/components/markdown.rs`

Markdown uses `#[derive(Default, typed_builder::TypedBuilder)]` on the `Markdown` struct (not `#[props]`). It needs:

1. Add `PartialEq` to the derives on the `Markdown` struct:
```rust
#[derive(Default, PartialEq, typed_builder::TypedBuilder)]
pub struct Markdown {
```

2. Add a manual `PropsMemo` impl after the `Markdown` impl block:
```rust
impl crate::PropsMemo for Markdown {
    fn props_changed(&self, old: &dyn std::any::Any) -> bool {
        old.downcast_ref::<Self>()
            .map_or(true, |old_props| self != old_props)
    }
}
```

3. Add `memo` to the `#[component]` attribute (find the line with `#[eye_declare_macros::component(props = Markdown, state = MarkdownState, ...)]`):
```rust
#[eye_declare_macros::component(props = Markdown, state = MarkdownState, initial_state = MarkdownState::new(), memo, crate_path = crate)]
```

**Note:** The `memo` flag position doesn't matter as long as it's comma-separated. Place it after `initial_state` and before `crate_path`.

### Task 3: Evaluate Text component

**File:** `crates/eye_declare/src/components/text.rs`

Text is a data-children component with `DataChildren<TextChild>`. Read the file to understand the structure:
- Does `TextChild` already derive `PartialEq`?
- Does `Span` (if used) derive `PartialEq`?

Check the Text struct definition and its `#[component]` attribute to understand the current setup.

**If `TextChild` and its contained types all support `PartialEq`:**
- Add `PartialEq` to `TextChild` derive
- Add `PartialEq` to `Text` struct derive
- Add `PropsMemo` impl for `Text`
- Add `memo` to the `#[component]` attribute

**If `TextChild` contains non-`PartialEq` types (closures, trait objects, etc.):**
- Skip Text for now. Add a comment noting why.

**Caution with data-children memo:** As noted in step 03, `memo` on data-children components only compares props, not data children. For Text, the props are minimal (styling) and the data children are the actual text content. Memoizing only on props while ignoring content changes would be a bug.

**Decision:** Do NOT add `memo` to Text. Text's content comes through data children, and the current architecture doesn't support comparing data children in `should_update`. Skipping memo on Text is correct — Text rendering is cheap anyway.

### Task 4: Opt in Spinner

**File:** `crates/eye_declare/src/components/spinner.rs`

Read the file to check the Spinner struct and its component attribute. Spinner has interval-based state (animation ticks) but its props (label, etc.) are simple strings.

1. If Spinner uses `#[props]`, it already gets `PartialEq` and `PropsMemo` from step 02. Just add `memo` to the component attribute.

2. If Spinner uses manual derive (like Markdown), add `PartialEq` derive, manual `PropsMemo` impl, and `memo` to the component attribute.

Check the actual code and apply accordingly. The component attribute should look like:
```
#[component(props = Spinner, ..., memo, crate_path = crate)]
```
or the equivalent with `eye_declare_macros::component`.

### Task 5: Skip Canvas

**File:** `crates/eye_declare/src/components/canvas.rs`

No changes. Canvas contains closures (`Box<dyn Fn(...)>`) — `PartialEq` is impossible. Canvas is a manual `impl Component` without `#[component]`. Its default `should_update` returns `true` (always re-render), which is correct.

### Task 6: Skip VStack, HStack, Column

**File:** `crates/eye_declare/src/component.rs`

VStack, HStack, Column are trivial containers. They use `#[eye_declare_macros::component(props = VStack, children = Elements, crate_path = crate)]`. They are near-zero-cost and their render cost is dominated by children.

Adding memo to these would require `PropsMemo` impls on unit/trivial structs. Low value, skip for now. Can be added later.

## Acceptance Criteria
- [ ] `cargo test -p eye_declare` passes (run from `/home/ignacio/s/eye-declare/.wt/should-update`)
- [ ] `cargo clippy -p eye_declare -- -D warnings` passes
- [ ] `grep -n 'memo' crates/eye_declare/src/components/markdown.rs` shows `memo` in the component attribute
- [ ] `grep -n 'PartialEq' crates/eye_declare/src/components/markdown.rs` shows the derive on the Markdown struct
- [ ] `grep -n 'PropsMemo' crates/eye_declare/src/components/markdown.rs` shows the impl
- [ ] `grep -n 'PartialEq' crates/eye_declare/src/children.rs` shows the derive on DataChildren
- [ ] Canvas component has no `memo` attribute: `grep -c 'memo' crates/eye_declare/src/components/canvas.rs` returns 0

## Reviewer Instructions
You are reviewing Step 04. Verify:
1. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare` — expected: all tests pass
2. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo clippy -p eye_declare -- -D warnings` — expected: no warnings
3. `grep -n 'memo' crates/eye_declare/src/components/markdown.rs` — expected: `memo` in component attribute
4. `grep -n 'PropsMemo' crates/eye_declare/src/components/markdown.rs` — expected: impl present
5. `grep -c 'memo' crates/eye_declare/src/components/canvas.rs` — expected: 0
6. Verify Text does NOT have `memo` (data-children comparison limitation)
Report: PASS with evidence, or FAIL: <criterion> — <what's wrong>

## Rollback
`git revert HEAD` (this step is a single commit)
