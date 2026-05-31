use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Expr, Field, Fields, Meta, parse2};

/// Implementation of the `#[props]` attribute macro.
///
/// Translates `#[default(expr)]` on fields to `#[builder(default = expr, setter(into))]`
/// and adds `#[derive(::eye_declare::TypedBuilder)]` to the struct.
///
/// Fields without `#[default]` are required — the builder won't compile
/// without them being set. Fields with `#[default(expr)]` are optional.
pub fn props_impl(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let no_memo: bool = if attr.is_empty() {
        false
    } else {
        let ident: syn::Ident = syn::parse2(attr.clone()).map_err(|_| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "expected `no_memo` or nothing in #[props(...)]",
            )
        })?;
        if ident == "no_memo" {
            true
        } else {
            return Err(syn::Error::new_spanned(
                ident,
                "unknown #[props] attribute. Expected `no_memo` or nothing.",
            ));
        }
    };

    let mut item: DeriveInput = parse2(input)?;

    let fields = match &mut item.data {
        syn::Data::Struct(data) => match &mut data.fields {
            Fields::Named(fields) => &mut fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &item.ident,
                    "#[props] only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &item.ident,
                "#[props] can only be applied to structs",
            ));
        }
    };

    // Transform field attributes: #[default(expr)] → #[builder(default = expr, setter(into))]
    // Fields without #[default] get #[builder(setter(into))] (required)
    for field in fields.iter_mut() {
        let default_expr = extract_and_remove_default_attr(field)?;

        match default_expr {
            Some(expr) => {
                // Optional field with default
                field.attrs.push(syn::parse_quote! {
                    #[builder(default = #expr, setter(into))]
                });
            }
            None => {
                // Required field
                field.attrs.push(syn::parse_quote! {
                    #[builder(setter(into))]
                });
            }
        }
    }

    // Add #[derive(::eye_declare::TypedBuilder)] to the struct,
    // using the re-export so downstream crates don't need typed_builder
    // as a direct dependency.
    item.attrs.push(syn::parse_quote! {
        #[derive(::eye_declare::TypedBuilder)]
    });

    if !no_memo && !already_has_partial_eq(&item.attrs) {
        item.attrs.push(syn::parse_quote! {
            #[derive(PartialEq)]
        });
    }

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
}

fn already_has_partial_eq(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        if let Ok(nested) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) && nested.iter().any(|p| p.is_ident("PartialEq"))
        {
            return true;
        }
    }
    false
}

/// Extract and remove a `#[default(expr)]` attribute from a field.
/// Returns `Some(expr)` if found, `None` otherwise.
fn extract_and_remove_default_attr(field: &mut Field) -> syn::Result<Option<Expr>> {
    let mut default_expr = None;

    for attr in field.attrs.iter() {
        if attr.path().is_ident("default") {
            if default_expr.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    "duplicate #[default] attribute",
                ));
            }

            let expr: Expr = match &attr.meta {
                Meta::List(list) => syn::parse2(list.tokens.clone())?,
                Meta::Path(_) => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "#[default] requires a value: #[default(expr)]",
                    ));
                }
                Meta::NameValue(_) => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "use #[default(expr)] not #[default = expr]",
                    ));
                }
            };

            default_expr = Some(expr);
        }
    }

    // Strip #[default] attributes
    if default_expr.is_some() {
        field.attrs.retain(|a| !a.path().is_ident("default"));
    }

    Ok(default_expr)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn required_field_gets_setter_into() {
        let input = quote! {
            struct MyProps {
                pub title: String,
            }
        };
        let result = props_impl(quote! {}, input).expect("macro should succeed");
        let output = result.to_string();
        // proc_macro2 adds spaces: "setter (into)"
        assert!(
            output.contains("setter"),
            "required field should have setter(into): {}",
            output
        );
    }

    #[test]
    fn default_field_gets_builder_default() {
        let input = quote! {
            struct MyProps {
                #[default(true)]
                pub visible: bool,
            }
        };
        let result = props_impl(quote! {}, input).expect("macro should succeed");
        let output = result.to_string();
        assert!(
            output.contains("default"),
            "optional field should have builder default: {}",
            output
        );
        // #[default(true)] should be stripped, replaced with #[builder(...)]
        assert!(
            !output.contains("# [default (true)]"),
            "original #[default] should be stripped: {}",
            output
        );
    }

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
    #[test]
    fn trailing_comma_in_no_memo_is_rejected() {
        let input = quote! {
            struct MyProps {
                pub title: String,
            }
        };
        // 'no_memo,' should fail with a parse error (extra tokens), not a confusing message
        assert!(props_impl(quote! { no_memo , }, input).is_err());
    }

    #[test]
    fn existing_partial_eq_not_duplicated() {
        let input = quote! {
            #[derive(PartialEq)]
            struct MyProps {
                pub title: String,
            }
        };
        let result = props_impl(quote! {}, input).expect("macro should succeed");
        let output = result.to_string();
        // PartialEq should appear exactly once (from the original derive, not injected again)
        let count = output.matches("PartialEq").count();
        assert_eq!(
            count, 1,
            "PartialEq should appear exactly once, got: {output}"
        );
        // PropsMemo should still be generated
        assert!(
            output.contains("PropsMemo"),
            "PropsMemo should still be generated: {output}"
        );
    }
}
