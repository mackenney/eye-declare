# Step 06: Integration Tests for Memoization

## Context
### Overall Objective
Add opt-in prop memoization to eye-declare. Verify that the full pipeline works end-to-end.

### This Step
Write integration tests that verify:
1. Default behavior preserved (components without `memo` always get `force_dirty = true`)
2. Memoized components skip re-render when props unchanged
3. Memoized components re-render when props change
4. State-dirty still triggers render even when props unchanged
5. Markdown memoization works correctly

### Why This Order
All implementation is complete. This step validates the full integration.

## Files to Read Before Starting
- `crates/eye_declare/src/renderer.rs` — `reconcile_children`, `render_node`, how `force_dirty` is used
- `crates/eye_declare/src/component.rs` — `Component` trait, `PropsMemo` trait
- `crates/eye_declare/src/node.rs` — `Node` struct, `force_dirty` field
- Existing test files in `crates/eye_declare/` (look for test modules and `tests/` directory)

## Implementation

### Task 1: Locate the right test file

Check if `crates/eye_declare/tests/` exists. If not, add tests as inline `#[cfg(test)]` modules.

The most appropriate location is inline tests in `crates/eye_declare/src/renderer.rs` since that's where `force_dirty` gating happens, OR in a new test file if integration tests exist separately.

Check existing test infrastructure to follow the pattern.

### Task 2: Write memoization tests

The tests need to exercise the reconciliation path. The pattern is:
1. Create a `Renderer`
2. Push a root component
3. Build an initial element tree
4. Reconcile with same/different props
5. Check `force_dirty` on the reused node

Here are the test cases. Adapt the exact API calls to match what's available in the crate (check existing tests for patterns):

**Test 1: Default behavior — no memo, always force_dirty**

Create a simple component without `memo`. Reconcile with identical props. Verify `force_dirty` is `true` after reconciliation.

```rust
#[derive(Default, Clone, PartialEq)]
struct NoMemoComp {
    value: u32,
}

impl Component for NoMemoComp {
    type State = ();
}

// Build initial tree with NoMemoComp(value: 1)
// Reconcile with NoMemoComp(value: 1) (same props)
// Assert node.force_dirty == true (no memo = always dirty)
```

**Test 2: Memo component skips force_dirty when props equal**

Create a component with `should_update` that compares props. Reconcile with identical props. Verify `force_dirty` stays `false`.

```rust
#[derive(Default, Clone, PartialEq)]
struct MemoComp {
    value: u32,
}

impl PropsMemo for MemoComp {
    fn props_changed(&self, old: &dyn std::any::Any) -> bool {
        old.downcast_ref::<Self>()
            .map_or(true, |old_props| self != old_props)
    }
}

impl Component for MemoComp {
    type State = ();

    fn should_update(&self, old_props: &dyn std::any::Any) -> bool {
        self.props_changed(old_props)
    }
}

// Build initial tree with MemoComp(value: 1)
// Clear force_dirty on the node
// Reconcile with MemoComp(value: 1) (same props)
// Assert node.force_dirty == false (memo = props unchanged = not dirty)
```

**Test 3: Memo component sets force_dirty when props change**

Same component as test 2. Reconcile with different props. Verify `force_dirty` is `true`.

```rust
// Build initial tree with MemoComp(value: 1)
// Reconcile with MemoComp(value: 2) (different props)
// Assert node.force_dirty == true (memo = props changed = dirty)
```

**Test 4: Element::update returns correct bool**

Test the `Element::update` method directly on a memo component.

```rust
// Create a Renderer, push a MemoComp(value: 1)
// Call Element::update with MemoComp(value: 1) — expect false
// Push another MemoComp(value: 1)
// Call Element::update with MemoComp(value: 2) — expect true
```

**Test 5: Default should_update returns true**

```rust
#[derive(Default, Clone)]
struct DefaultComp;

impl Component for DefaultComp {
    type State = ();
}

// Verify DefaultComp.should_update(&42u32 as &dyn Any) == true
// Verify DefaultComp.should_update(&DefaultComp as &dyn Any) == true
```

### Task 3: Adapt to existing test patterns

Before writing tests, search for existing test patterns in the renderer:
- `grep -rn '#[cfg(test)]' crates/eye_declare/src/renderer.rs`
- `ls crates/eye_declare/tests/`

Use whichever testing approach already exists. If inline tests create a `Renderer` instance, follow that pattern. If integration tests use a higher-level API, follow that.

Key APIs you'll likely need:
- `Renderer::new()` or equivalent
- `renderer.append_child(root, component)` or `renderer.push(component)`
- `Elements::new()` + `els.add(component)`
- `renderer.reconcile_children(parent, elements.into_items())`
- `renderer.nodes[node_id].force_dirty`

Some of these are `pub(crate)` so inline tests have access but external tests may not. Place tests accordingly.

## Acceptance Criteria
- [ ] `cargo test -p eye_declare` passes (run from `/home/ignacio/s/eye-declare/.wt/should-update`)
- [ ] `cargo clippy -p eye_declare -- -D warnings` passes
- [ ] `cargo test -p eye_declare -- memo` shows at least 3 passing tests related to memoization
- [ ] Tests cover: no-memo always dirty, memo skip when equal, memo dirty when changed

## Reviewer Instructions
You are reviewing Step 06. Verify:
1. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare` — expected: all tests pass
2. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare -- memo 2>&1 | grep 'test result'` — expected: at least 3 tests pass
3. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo clippy -p eye_declare -- -D warnings` — expected: no warnings
4. Verify tests are not trivial (check that they actually exercise reconciliation or Element::update)
Report: PASS with evidence, or FAIL: <criterion> — <what's wrong>

## Rollback
`git revert HEAD` (this step is a single commit)
