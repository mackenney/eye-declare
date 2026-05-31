# Step 03: Component Macro — `memo` Attribute and `should_update` Generation

## Context
### Overall Objective
Add opt-in prop memoization to eye-declare. The `#[component(memo)]` attribute triggers generation of a `should_update` override that delegates to `PropsMemo::props_changed`.

### This Step
1. Parse `memo` as a new key in the `#[component(...)]` attribute
2. In the slot/no-children path: when `memo` is set, generate a `should_update` override on the props type
3. In the data-children path: when `memo` is set, generate a `should_update` override on the `__PropsWithData` wrapper that compares both `__props` and `__data` via downcast
4. For `ComponentWithSlot<P>`: add a `should_update` override in library code that delegates to the inner component's `should_update`

### Why This Order
Depends on step 01 (the `should_update` method on `Component`) and step 02 (the `PropsMemo` trait and `#[props]` generating its impl). This step wires them together in the `#[component]` macro.

## Files to Read Before Starting
- `crates/eye_declare_macros/src/component.rs` — the `#[component]` macro implementation. Focus on `ComponentArgs`, `generate_slot_or_none`, and `generate_data_children`
- `crates/eye_declare/src/component.rs` — the `Component` trait with `should_update` (added in step 01)
- `crates/eye_declare/src/children.rs` — `ComponentWithSlot<C>` definition and its `AddTo` impl
- `crates/eye_declare/src/node.rs` — `AnyComponent` trait (for reference on how `should_update_erased` is plumbed)

## Implementation

### Task 1: Add `memo` to `ComponentArgs`

**File:** `crates/eye_declare_macros/src/component.rs`

Add a `memo: bool` field to `ComponentArgs`:

```rust
struct ComponentArgs {
    props: Ident,
    state: Option<Ident>,
    children: Option<syn::Type>,
    initial_state: Option<syn::Expr>,
    crate_path: Option<syn::Path>,
    memo: bool,
}
```

In the `Parse` impl for `ComponentArgs`, add a `let mut memo = false;` initialization alongside the other `let mut` declarations.

Add a match arm in the parsing loop:
```rust
                "memo" => {
                    memo = true;
                    // memo is a flag — no `= value` expected, but the parser
                    // already consumed `=` above. We need to handle this
                    // differently since memo is a bare flag, not a key=value.
                }
```

**Wait — the current parser expects `key = value` pairs.** The `memo` attribute is a bare flag with no value. The parser structure is:
```rust
let key: Ident = input.parse()?;
input.parse::<Token![=]>()?;
```

This means we need to restructure slightly. Change the parsing to check for `=` before consuming it:

Replace the parsing body inside `while !input.is_empty()` with:

```rust
            let key: Ident = input.parse()?;

            match key.to_string().as_str() {
                "memo" => {
                    memo = true;
                }
                "initial_state" => {
                    input.parse::<Token![=]>()?;
                    initial_state_key_span = Some(key.span());
                    let expr: syn::Expr = input.parse()?;
                    initial_state = Some(expr);
                }
                "props" => {
                    input.parse::<Token![=]>()?;
                    let value: Ident = input.parse()?;
                    props = Some(value);
                }
                "state" => {
                    input.parse::<Token![=]>()?;
                    let value: Ident = input.parse()?;
                    state = Some(value);
                }
                "children" => {
                    input.parse::<Token![=]>()?;
                    let value: syn::Type = input.parse()?;
                    children = Some(value);
                }
                "crate_path" => {
                    input.parse::<Token![=]>()?;
                    let value: syn::Path = input.parse()?;
                    crate_path = Some(value);
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        key,
                        format!("unknown component attribute: `{other}`"),
                    ));
                }
            }
```

This moves the `input.parse::<Token![=]>()?;` into each arm that needs it, so `memo` (a bare flag) doesn't try to parse `=`.

Add `memo` to the `Ok(ComponentArgs { ... })` at the end.

### Task 2: Pass `memo` through to codegen functions

**File:** `crates/eye_declare_macros/src/component.rs`

In `component_impl`, pass `args.memo` to both `generate_data_children` and `generate_slot_or_none`. Add a `memo: bool` parameter to both functions.

For `generate_slot_or_none`, add `memo` parameter:
```rust
fn generate_slot_or_none(
    func: &ItemFn,
    func_name: &Ident,
    props_type: &Ident,
    crate_path: &TokenStream,
    state_type: &TokenStream,
    has_state: bool,
    has_hooks: bool,
    has_children: bool,
    initial_state_impl: &TokenStream,
    memo: bool,
) -> syn::Result<TokenStream> {
```

For `generate_data_children`, add `memo` parameter:
```rust
fn generate_data_children(
    func: &ItemFn,
    func_name: &Ident,
    props_type: &Ident,
    crate_path: &TokenStream,
    state_type: &TokenStream,
    has_state: bool,
    has_hooks: bool,
    initial_state_impl: &TokenStream,
    children_type: &syn::Type,
    memo: bool,
) -> syn::Result<TokenStream> {
```

Update the call sites in `component_impl` to pass `args.memo`.

### Task 3: Generate `should_update` in slot/no-children path

**File:** `crates/eye_declare_macros/src/component.rs`

In `generate_slot_or_none`, add a `should_update_impl` variable after `initial_state_impl`:

```rust
    let should_update_impl = if memo {
        quote! {
            fn should_update(&self, old_props: &dyn ::std::any::Any) -> bool {
                <#props_type as #crate_path::PropsMemo>::props_changed(self, old_props)
            }
        }
    } else {
        quote! {}
    };
```

Include it in the `impl Component for #props_type` block:

```rust
    Ok(quote! {
        #func

        impl #crate_path::Component for #props_type {
            type State = #state_type;

            #initial_state_impl
            #should_update_impl
            #update_impl
        }

        #child_collector
    })
```

### Task 4: Generate `should_update` in data-children path

**File:** `crates/eye_declare_macros/src/component.rs`

In `generate_data_children`, add a `should_update_impl` for the props-type impl (no children):

```rust
    let props_should_update_impl = if memo {
        quote! {
            fn should_update(&self, old_props: &dyn ::std::any::Any) -> bool {
                <#props_type as #crate_path::PropsMemo>::props_changed(self, old_props)
            }
        }
    } else {
        quote! {}
    };
```

For the `__PropsWithData` wrapper, the `should_update` needs to compare via `props_as_any()` on the old component. The wrapper's `props_as_any()` returns `&self.__props` (the inner props struct). So the old props passed to `should_update` will be `&dyn Any` pointing to the inner props type. The wrapper delegates to the inner props' `PropsMemo`:

```rust
    let wrapper_should_update_impl = if memo {
        quote! {
            fn should_update(&self, old_props: &dyn ::std::any::Any) -> bool {
                <#props_type as #crate_path::PropsMemo>::props_changed(&self.__props, old_props)
            }
        }
    } else {
        quote! {}
    };
```

**Note on data children comparison:** The wrapper's `should_update` only compares the props portion, not `__data`. This is intentional — data children are baked into the component and always processed during `update()`. Since `should_update` is called BEFORE `swap_component` (per step 01), and the old component on the node is the previous wrapper with its previous data, we'd need to downcast to the wrapper type to compare data. But `props_as_any()` on the wrapper returns the inner props, not the wrapper itself. So data children comparison is not possible with this architecture without additional plumbing.

The practical consequence: data-children components with `memo` will skip re-render when props are equal, even if data children changed. For the current built-in components (Text, Markdown), data children changing means the component output changes, so we should NOT use `memo` on data-children components unless the data children are also compared.

**For now:** Generate the same props-only comparison. Document that `memo` on data-children components only compares props. If data children are likely to change independently, don't use `memo`.

Include them in the generated code:

In the `impl Component for #props_type` block, add `#props_should_update_impl`.
In the `impl Component for #wrapper_name` block, add `#wrapper_should_update_impl`.

### Task 5: Update `ComponentWithSlot` to delegate `should_update`

**File:** `crates/eye_declare/src/children.rs`

`ComponentWithSlot<C>` is used with `add_with_children` in `Elements`, which directly adds the inner component `self.component` with the children `self.children`. It does NOT implement `Component` itself — it passes through to `Elements::add_with_children`.

Check the `AddTo` impl:
```rust
impl<C: Component> AddTo<Elements> for ComponentWithSlot<C> {
    fn add_to(self, els: &mut Elements) -> ElementHandle<'_> {
        els.add_with_children(self.component, self.children)
    }
}
```

So `ComponentWithSlot` is never itself an `Element` or `Component` on a node. The inner component `C` is what becomes the node's component. Slot children are reconciled separately. This means `ComponentWithSlot` does NOT need a `should_update` override — the inner component's `should_update` is already used via the `Element` blanket impl on `C`.

**No change needed for `ComponentWithSlot`.** The slot-children path in `generate_slot_or_none` generates `impl Component for #props_type`, and the `ComponentWithSlot` wrapper just passes `self.component` (of type `#props_type`) to `Elements::add_with_children`. The component on the node IS the props type, not the wrapper.

### Task 6: Verify no compile breakage

After making all changes, ensure that existing `#[component]` usage without `memo` still compiles identically. The `memo` flag defaults to `false`, so no `should_update` override is generated, and the default `true` from the `Component` trait applies.

## Acceptance Criteria
- [ ] `cargo test -p eye_declare` passes (run from `/home/ignacio/s/eye-declare/.wt/should-update`)
- [ ] `cargo test -p eye_declare_macros` passes
- [ ] `cargo clippy -p eye_declare -- -D warnings` passes
- [ ] `cargo clippy -p eye_declare_macros -- -D warnings` passes
- [ ] `grep -n 'memo' crates/eye_declare_macros/src/component.rs` shows the `memo` field, parsing, and conditional codegen
- [ ] `grep -n 'should_update' crates/eye_declare_macros/src/component.rs` shows the generated `should_update` override using `PropsMemo`

## Reviewer Instructions
You are reviewing Step 03. Verify:
1. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare` — expected: all tests pass
2. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare_macros` — expected: all tests pass
3. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo clippy -p eye_declare -- -D warnings` — expected: no warnings
4. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo clippy -p eye_declare_macros -- -D warnings` — expected: no warnings
5. `grep -c 'memo' crates/eye_declare_macros/src/component.rs` — expected: multiple occurrences
6. Verify that `#[component(props = Foo)]` (without memo) still compiles without `PropsMemo` on `Foo`
Report: PASS with evidence, or FAIL: <criterion> — <what's wrong>

## Rollback
`git revert HEAD` (this step is a single commit)
