# Step 01: Trait Extension and Renderer Plumbing

## Context
### Overall Objective
Add opt-in prop memoization to eye-declare. When a component's props haven't changed, skip setting `force_dirty = true` on the reused node, allowing cached render buffers to be reused.

### This Step
Wire up the core trait methods and renderer change:
1. Add `should_update` method to the `Component` trait (default: returns `true`)
2. Add `should_update_erased` to `AnyComponent` trait + blanket impl
3. Change `Element::update` return type from `()` to `bool` (true = props changed)
4. Update the blanket `impl<C: Component> Element for C` to call `should_update` before `swap_component` and return the result
5. Gate `force_dirty = true` on the return value in `reconcile_children`

### Why This Order
This is the foundation. The `PropsMemo` trait (step 02) and macro changes (step 03) build on top of these trait signatures. The renderer change is safe because the default `should_update` returns `true`, preserving current behavior for all existing components.

## Files to Read Before Starting
- `crates/eye_declare/src/component.rs` — the `Component` trait where `should_update` will be added
- `crates/eye_declare/src/node.rs` — `AnyComponent` trait and its blanket impl for `Component`
- `crates/eye_declare/src/element.rs` — `Element` trait and its blanket impl for `Component`
- `crates/eye_declare/src/renderer.rs` lines 690-800 — `reconcile_children` REUSE path

## Implementation

### Task 1: Add `should_update` to `Component` trait

**File:** `crates/eye_declare/src/component.rs`

Add this method to the `Component` trait, after the `props_as_any` method (after line 274 in the current source, the closing brace of `props_as_any`):

```rust
    /// Whether this component should re-render after receiving new props.
    ///
    /// Called during reconciliation when a node is reused with new props.
    /// `old_props` is `props_as_any()` on the previous component instance,
    /// before the swap. Return `false` to skip re-rendering when props are
    /// unchanged.
    ///
    /// The default returns `true` (always re-render). Components using
    /// `#[component(memo)]` override this automatically via `PropsMemo`.
    #[doc(hidden)]
    fn should_update(&self, _old_props: &dyn std::any::Any) -> bool {
        true
    }
```

This goes inside the `pub trait Component` block, alongside the other `#[doc(hidden)]` methods. Place it between `props_as_any` and `render`.

### Task 2: Add `should_update_erased` to `AnyComponent`

**File:** `crates/eye_declare/src/node.rs`

Add to the `AnyComponent` trait definition (inside the `pub(crate) trait AnyComponent` block), after `props_as_any`:

```rust
    fn should_update_erased(&self, old_props: &dyn Any) -> bool;
```

Add to the blanket `impl<C: Component> AnyComponent for C` block, after `props_as_any`:

```rust
    fn should_update_erased(&self, old_props: &dyn Any) -> bool {
        self.should_update(old_props)
    }
```

### Task 3: Change `Element::update` to return `bool`

**File:** `crates/eye_declare/src/element.rs`

Change the `Element` trait method signature from:
```rust
    fn update(self: Box<Self>, _renderer: &mut Renderer, _node_id: NodeId) {}
```
to:
```rust
    fn update(self: Box<Self>, _renderer: &mut Renderer, _node_id: NodeId) -> bool {
        true
    }
```

Change the blanket impl `impl<C: Component> Element for C` `update` method from:
```rust
    fn update(self: Box<Self>, renderer: &mut Renderer, node_id: NodeId) {
        renderer.swap_component(node_id, *self);
    }
```
to:
```rust
    fn update(self: Box<Self>, renderer: &mut Renderer, node_id: NodeId) -> bool {
        let should = self.should_update(renderer.old_props_as_any(node_id));
        renderer.swap_component(node_id, *self);
        should
    }
```

**Important ordering:** `should_update` is called BEFORE `swap_component` because the old component's props are needed for comparison. After `swap_component`, the old component is gone.

**Critical:** `renderer.nodes` is private to the renderer module. You must add a `pub(crate)` helper method to `Renderer` in `renderer.rs` BEFORE updating `element.rs`:

```rust
/// Returns `props_as_any()` on the component currently stored at `id`.
/// Used by `Element::update` to read old props before swapping.
pub(crate) fn old_props_as_any(&self, id: NodeId) -> &dyn std::any::Any {
    self.nodes[id].component.props_as_any()
}
```

Add this method to `impl Renderer` in `renderer.rs`, near `swap_component` (around line 96).  The `element.rs` blanket impl then calls `renderer.old_props_as_any(node_id)` instead of accessing `renderer.nodes` directly.

### Task 4: Gate `force_dirty` in `reconcile_children`

**File:** `crates/eye_declare/src/renderer.rs`

In `reconcile_children`, find the REUSE path (around line 744-752). Change from:

```rust
let node_id = if let Some(old_id) = matched {
    // REUSE: update props, preserve local state
    entry.element.update(self, old_id);
    self.nodes[old_id].parent = Some(parent);
    self.nodes[old_id].width_constraint =
        resolve_width_constraint(&self.nodes[old_id], entry.width_constraint);
    self.nodes[old_id].has_slot = entry.children.is_some();
    // Guarantee re-render after props update
    self.nodes[old_id].force_dirty = true;
```

to:

```rust
let node_id = if let Some(old_id) = matched {
    // REUSE: update props, preserve local state
    let props_changed = entry.element.update(self, old_id);
    self.nodes[old_id].parent = Some(parent);
    self.nodes[old_id].width_constraint =
        resolve_width_constraint(&self.nodes[old_id], entry.width_constraint);
    self.nodes[old_id].has_slot = entry.children.is_some();
    if props_changed {
        self.nodes[old_id].force_dirty = true;
    }
```

Everything after this point (`update_node`, `reconcile_children` for slot children, `push_context`/`pop_context`) remains unchanged. `update_node` MUST always run because it collects hooks.

## Acceptance Criteria
- [ ] `cargo test -p eye_declare` passes (run from `/home/ignacio/s/eye-declare/.wt/should-update`)
- [ ] `cargo clippy -p eye_declare -- -D warnings` passes
- [ ] `cargo clippy -p eye_declare_macros -- -D warnings` passes
- [ ] `grep -n 'fn should_update' crates/eye_declare/src/component.rs` shows the new method in the Component trait
- [ ] `grep -n 'fn should_update_erased' crates/eye_declare/src/node.rs` shows the new method in AnyComponent trait and blanket impl
- [ ] `grep -n 'fn update.*-> bool' crates/eye_declare/src/element.rs` shows the new return type
- [ ] `grep -n 'props_changed' crates/eye_declare/src/renderer.rs` shows the conditional force_dirty

## Reviewer Instructions
You are reviewing Step 01. Verify:
1. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare` — expected: all tests pass
2. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo clippy -p eye_declare -- -D warnings` — expected: no warnings
3. `grep -n 'fn should_update' crates/eye_declare/src/component.rs` — expected: method with signature `fn should_update(&self, _old_props: &dyn std::any::Any) -> bool`
4. `grep -n 'fn should_update_erased' crates/eye_declare/src/node.rs` — expected: trait method and blanket impl
5. `grep -n '-> bool' crates/eye_declare/src/element.rs` — expected: `update` returns `bool`
6. `grep -n 'props_changed' crates/eye_declare/src/renderer.rs` — expected: `let props_changed = entry.element.update(self, old_id);` and `if props_changed {`
7. Verify `update_node` is still called unconditionally after the `if props_changed` block
Report: PASS with evidence, or FAIL: <criterion> — <what's wrong>

## Rollback
`git revert HEAD` (this step is a single commit)
