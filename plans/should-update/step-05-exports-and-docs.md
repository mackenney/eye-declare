# Step 05: Re-exports and Documentation

## Context
### Overall Objective
Add opt-in prop memoization to eye-declare. Public API needs `PropsMemo` re-exported and attributes documented.

### This Step
1. Re-export `PropsMemo` from the crate root
2. Update `#[component]` macro docs to document the `memo` attribute
3. Update `#[props]` macro docs to document `no_memo` escape hatch

### Why This Order
Depends on steps 01 (trait), 02 (`PropsMemo` definition), and 03 (macro changes). This step makes the API discoverable.

## Files to Read Before Starting
- `crates/eye_declare/src/lib.rs` — crate root re-exports
- `crates/eye_declare_macros/src/lib.rs` — macro doc comments

## Implementation

### Task 1: Re-export `PropsMemo`

**File:** `crates/eye_declare/src/lib.rs`

Find the line that re-exports from `component`:
```rust
pub use component::{Column, Component, EventResult, HStack, Tracked, TrackedRef, VStack};
```

Add `PropsMemo` to this re-export:
```rust
pub use component::{Column, Component, EventResult, HStack, PropsMemo, Tracked, TrackedRef, VStack};
```

### Task 2: Update `#[props]` macro documentation

**File:** `crates/eye_declare_macros/src/lib.rs`

Find the doc comment block before `pub fn props(...)`. Add documentation about memoization behavior and `no_memo`. Update the doc comment to include something like:

After the existing doc content (before the `#[proc_macro_attribute]` line), add:

```rust
///
/// # Memoization
///
/// `#[props]` automatically derives `PartialEq` and implements `PropsMemo`
/// for the struct. This enables `#[component(memo)]` to compare old and new
/// props and skip re-rendering when they're equal.
///
/// If the struct contains fields that don't implement `PartialEq` (closures,
/// trait objects, etc.), use `#[props(no_memo)]` to suppress the automatic
/// `PartialEq` derive and `PropsMemo` impl:
///
/// ```ignore
/// #[props(no_memo)]
/// struct CanvasProps {
///     pub render_fn: Box<dyn Fn(&mut Buffer, Rect) + Send + Sync>,
/// }
/// ```
```

### Task 3: Update `#[component]` macro documentation

**File:** `crates/eye_declare_macros/src/lib.rs`

Find the doc comment block before `pub fn component(...)`. Add `memo` to the attributes list. After the existing attribute docs:

```rust
/// - `memo` — optional flag. When present, generates a `should_update` override
///   that compares props using `PropsMemo`. The props type must implement `PropsMemo`
///   (automatic with `#[props]`, or manual impl). Components without `memo` always
///   re-render when their parent rebuilds (default behavior).
```

## Acceptance Criteria
- [ ] `cargo test -p eye_declare` passes (run from `/home/ignacio/s/eye-declare/.wt/should-update`)
- [ ] `cargo clippy -p eye_declare -- -D warnings` passes
- [ ] `grep -n 'PropsMemo' crates/eye_declare/src/lib.rs` shows the re-export
- [ ] `grep -n 'no_memo' crates/eye_declare_macros/src/lib.rs` shows documentation
- [ ] `grep -n 'memo' crates/eye_declare_macros/src/lib.rs` shows documentation for the component attribute

## Reviewer Instructions
You are reviewing Step 05. Verify:
1. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare` — expected: all tests pass
2. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo clippy -p eye_declare -- -D warnings` — expected: no warnings
3. `grep 'PropsMemo' crates/eye_declare/src/lib.rs` — expected: appears in a `pub use` line
4. `grep -c 'no_memo' crates/eye_declare_macros/src/lib.rs` — expected: at least 1 (in doc comment)
5. `grep -c 'memo' crates/eye_declare_macros/src/lib.rs` — expected: at least 2 (in doc comments for both macros)
Report: PASS with evidence, or FAIL: <criterion> — <what's wrong>

## Rollback
`git revert HEAD` (this step is a single commit)
