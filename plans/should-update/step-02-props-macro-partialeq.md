# Step 02: Props Macro — PartialEq Derive and PropsMemo Impl

## Context
### Overall Objective
Add opt-in prop memoization to eye-declare. The `#[props]` macro should auto-derive `PartialEq` and generate a `PropsMemo` impl so that `#[component(memo)]` can use it for comparison.

### This Step
1. Define the `PropsMemo` trait in eye-declare library code
2. Update the `#[props]` macro to auto-derive `PartialEq` on the struct
3. Update the `#[props]` macro to generate an `impl PropsMemo` for the struct
4. Add `#[props(no_memo)]` escape hatch to suppress both `PartialEq` and `PropsMemo`

### Why This Order
This step runs in parallel with step 01. It only touches the macro crate and the `PropsMemo` trait definition in the library. No dependency on the `should_update` trait method.

## Files to Read Before Starting
- `crates/eye_declare_macros/src/props.rs` — the `#[props]` macro implementation
- `crates/eye_declare_macros/src/lib.rs` — macro entry point (for attribute signature)
- `crates/eye_declare/src/lib.rs` — re-exports (will need `PropsMemo` re-exported in step 05)
- `crates/eye_declare/src/component.rs` — where `PropsMemo` trait definition will go

## Implementation

### Task 1: Define `PropsMemo` trait

**File:** `crates/eye_declare/src/component.rs`

Add this trait definition BEFORE the `Component` trait (e.g. after the `EventResult` enum, around line 65):

```rust
/// Helper trait for prop comparison in memoized components.
///
/// Implemented automatically by the `#[props]` macro for types that
/// derive `PartialEq`. Used by `#[component(memo)]` to generate
/// `should_update` implementations.
///
/// Manual implementation is supported for custom comparison logic.
pub trait PropsMemo: 'static {
    /// Return `true` if props have changed (component should re-render).
    ///
    /// `old` is the result of `props_as_any()` on the previous component.
    /// Implementations downcast `old` to `Self` and compare.
    fn props_changed(&self, old: &dyn std::any::Any) -> bool;
}
```

### Task 2: Update `#[props]` macro to accept `no_memo` attribute

**File:** `crates/eye_declare_macros/src/lib.rs`

Change the `props` function signature from:
```rust
pub fn props(_attr: TokenStream, input: TokenStream) -> TokenStream {
    match props::props_impl(input.into()) {
```
to:
```rust
pub fn props(attr: TokenStream, input: TokenStream) -> TokenStream {
    match props::props_impl(attr.into(), input.into()) {
```

### Task 3: Update `props_impl` to derive PartialEq and generate PropsMemo

**File:** `crates/eye_declare_macros/src/props.rs`

Change the function signature from:
```rust
pub fn props_impl(input: TokenStream) -> syn::Result<TokenStream> {
```
to:
```rust
pub fn props_impl(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
```

At the top of the function, parse the attribute for `no_memo`:
```rust
    let no_memo = {
        let attr_str = attr.to_string();
        if attr_str.is_empty() {
            false
        } else if attr_str == "no_memo" {
            true
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("unknown #[props] attribute: `{attr_str}`. Expected `no_memo` or nothing."),
            ));
        }
    };
```

After the existing `item.attrs.push(syn::parse_quote! { #[derive(::eye_declare::TypedBuilder)] });` line, add:

```rust
    if !no_memo {
        item.attrs.push(syn::parse_quote! {
            #[derive(PartialEq)]
        });
    }
```

Then change the final return from:
```rust
    Ok(quote! { #item })
```
to:
```rust
    let name = &item.ident;

    let props_memo_impl = if !no_memo {
        quote! {
            impl ::eye_declare::PropsMemo for #name {
                fn props_changed(&self, old: &dyn ::std::any::Any) -> bool {
                    old.downcast_ref::<Self>()
                        .map_or(true, |old_props| self != old_props)
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #item
        #props_memo_impl
    })
```

### Task 4: Update existing tests and add new tests

**File:** `crates/eye_declare_macros/src/props.rs`

Update the existing test `generates_typed_builder_derive` to pass the new signature:
```rust
    #[test]
    fn generates_typed_builder_derive() {
        let input = quote! {
            struct MyProps {
                pub title: String,
            }
        };
        let result = props_impl(quote! {}, input).expect("macro should succeed");
        let output = result.to_string();
        assert!(
            output.contains("TypedBuilder"),
            "should have TypedBuilder derive: {}",
            output
        );
    }
```

Update ALL existing test calls from `props_impl(input)` to `props_impl(quote! {}, input)`.

Add new tests:

```rust
    #[test]
    fn generates_partial_eq_derive() {
        let input = quote! {
            struct MyProps {
                pub title: String,
            }
        };
        let result = props_impl(quote! {}, input).expect("macro should succeed");
        let output = result.to_string();
        assert!(
            output.contains("PartialEq"),
            "should have PartialEq derive: {}",
            output
        );
    }

    #[test]
    fn generates_props_memo_impl() {
        let input = quote! {
            struct MyProps {
                pub title: String,
            }
        };
        let result = props_impl(quote! {}, input).expect("macro should succeed");
        let output = result.to_string();
        assert!(
            output.contains("PropsMemo"),
            "should have PropsMemo impl: {}",
            output
        );
        assert!(
            output.contains("props_changed"),
            "should have props_changed method: {}",
            output
        );
    }

    #[test]
    fn no_memo_suppresses_partial_eq_and_props_memo() {
        let input = quote! {
            struct MyProps {
                pub title: String,
            }
        };
        let result = props_impl(quote! { no_memo }, input).expect("macro should succeed");
        let output = result.to_string();
        assert!(
            !output.contains("PartialEq"),
            "no_memo should suppress PartialEq: {}",
            output
        );
        assert!(
            !output.contains("PropsMemo"),
            "no_memo should suppress PropsMemo: {}",
            output
        );
    }

    #[test]
    fn rejects_unknown_props_attr() {
        let input = quote! {
            struct MyProps {
                pub title: String,
            }
        };
        assert!(props_impl(quote! { foobar }, input).is_err());
    }
```

Also update the `rejects_enum` and `rejects_tuple_struct` test calls:
```rust
    #[test]
    fn rejects_enum() {
        let input = quote! {
            enum Bad { A, B }
        };
        assert!(props_impl(quote! {}, input).is_err());
    }

    #[test]
    fn rejects_tuple_struct() {
        let input = quote! {
            struct Bad(u32, String);
        };
        assert!(props_impl(quote! {}, input).is_err());
    }
```

## Acceptance Criteria
- [ ] `cargo test -p eye_declare_macros` passes (run from `/home/ignacio/s/eye-declare/.wt/should-update`)
- [ ] `cargo clippy -p eye_declare_macros -- -D warnings` passes
- [ ] `cargo clippy -p eye_declare -- -D warnings` passes
- [ ] `grep -n 'trait PropsMemo' crates/eye_declare/src/component.rs` shows the trait definition
- [ ] `grep -n 'PartialEq' crates/eye_declare_macros/src/props.rs` shows the derive being added
- [ ] `grep -n 'PropsMemo' crates/eye_declare_macros/src/props.rs` shows the impl being generated
- [ ] `grep -n 'no_memo' crates/eye_declare_macros/src/props.rs` shows the escape hatch

## Reviewer Instructions
You are reviewing Step 02. Verify:
1. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare_macros` — expected: all tests pass
2. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo clippy -p eye_declare_macros -- -D warnings` — expected: no warnings
3. `grep -n 'trait PropsMemo' crates/eye_declare/src/component.rs` — expected: trait with `props_changed` method
4. `grep -n 'no_memo' crates/eye_declare_macros/src/props.rs` — expected: attribute parsing and conditional generation
5. `cd /home/ignacio/s/eye-declare/.wt/should-update && cargo test -p eye_declare_macros -- no_memo` — expected: test passes
Report: PASS with evidence, or FAIL: <criterion> — <what's wrong>

## Rollback
`git revert HEAD` (this step is a single commit)
