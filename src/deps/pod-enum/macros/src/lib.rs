//! A proc-macro for the `pod-enum` crate
//!
//! Consider importing that crate instead

use proc_macro::TokenStream as TokenStream1;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_macro_input, Attribute, Expr, Type};

/// A variant of a [`PodEnum`]
///
/// This has already been parsed to ensure that no errors are possible during code generation.
struct Variant {
    /// The name of this variant
    ident: Ident,
    /// The discriminant given for this variant
    discriminant: Expr,
    /// The documentation attributes on this variant
    documentation: Vec<Attribute>,
}

/// An enum given to this macro
///
/// This has already been parsed to ensure that no errors are possible during code generation.
struct PodEnum {
    /// The visibility of this enum (e.g. `pub` or `pub(crate)`)
    ///
    /// We forward this visibility to the new type we generate and all of the variants.
    vis: syn::Visibility,
    /// The name of this type
    ident: Ident,
    /// The type used for the representation
    repr: Type,
    /// The variants of this enum, along with their discriminants and documentation
    variants: Vec<Variant>,
    /// The attributes to apply to the output
    ///
    /// This is all attributes on the input except the `#[repr(..)]` attribute, which is filtered
    /// out to be handled separately.
    attrs: Vec<Attribute>,
}

/// Code generation
impl PodEnum {
    /// Write all methods associated with this type
    fn write_impl(&self) -> TokenStream {
        let ident = &self.ident;
        let repr = &self.repr;
        let vis = &self.vis;
        let attrs = &self.attrs;

        let variants = self.write_variants();
        let debug = self.write_debug();
        let conversions = self.write_conversions();
        //let partial_eq = self.write_partial_eq();

        quote!(
            #( #attrs )*
            #[derive(Copy, Clone, PartialEq, Eq)]
            #[repr(transparent)]
            #vis struct #ident {
                inner: #repr,
            }

            impl ::pod_enum::PodEnum for #ident {
                type Repr = #repr;
            }

            // SAFETY:
            // The `PodEnum` trait (implemented above) checks that our internal type is
            // `Pod`, and since we're #[`repr(transparent)]` with one field, we can also
            // implement `Pod`.
            unsafe impl ::pod_enum::bytemuck::Pod for #ident {}
            // SAFETY:
            // The `PodEnum` trait (implemented above) checks that our internal type is
            // `Pod` (which implies `Zeroable`), and since we're #[`repr(transparent)]`
            // with one field, we can also implement `Zeroable`.
            unsafe impl ::pod_enum::bytemuck::Zeroable for #ident {}

            #variants

            #debug

            #conversions
        )
    }

    /// Write out all variants of this enum as constants
    fn write_variants(&self) -> TokenStream {
        let ident = &self.ident;
        let vis = &self.vis;
        let variants = self.variants.iter().map(
            |Variant {
                 ident,
                 discriminant,
                 documentation,
             }| {
                quote!(
                    #( #documentation )*
                    #vis const #ident: Self = Self { inner: #discriminant };
                )
            },
        );
        quote! {
            /// The variants of this enum
            #[allow(non_upper_case_globals)]
            impl #ident {
                #( #variants )*
            }
        }
    }

    /// Write the debug impl
    fn write_debug(&self) -> TokenStream {
        let ident = &self.ident;
        let variants = self.variants.iter().map(
            |Variant {
                 ident,
                 discriminant,
                 ..
             }| {
                let name = ident.to_string();
                quote!(#discriminant => f.write_str(#name))
            },
        );
        quote!(
            /// Display which variant this is, or call it unknown and show the discriminant
            impl ::core::fmt::Debug for #ident {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self.inner {
                        #( #variants, )*
                        val => write!(f, "Unknown ({})", val),
                    }
                }
            }
        )
    }

    /// Write conversions to and from the underlying base type
    fn write_conversions(&self) -> TokenStream {
        let ident = &self.ident;
        let repr = &self.repr;

        quote!(
            impl From<#repr> for #ident {
                fn from(inner: #repr) -> Self {
                    Self { inner }
                }
            }

            impl From<#ident> for #repr {
                fn from(pod: #ident) -> Self {
                    pod.inner
                }
            }
        )
    }

    /// Write [`PartialEq`] implementation
    fn write_partial_eq(&self) -> TokenStream {
        let ident = &self.ident;
        let variants = self
            .variants
            .iter()
            .map(|Variant { discriminant, .. }| quote!((#discriminant, #discriminant) => true));

        quote!(
            /// Listed variants compare as equaling itself and unequal to anything else
            ///
            /// Any variant not listed compares as unequal to everything, including itself.
            /// Thus, we only implement `PartialEq` and not [`Eq`].
            impl PartialEq for #ident {
                fn eq(&self, other: &Self) -> bool {
                    match (self.inner, other.inner) {
                        #( #variants, )*
                        _ => false,
                    }
                }
            }
        )
    }
}

/// Attempt to parse the input from a Rust enum definition
///
/// In the event of an error, we try to return as many compile errors as we can.
impl TryFrom<syn::ItemEnum> for PodEnum {
    type Error = TokenStream;

    fn try_from(value: syn::ItemEnum) -> Result<Self, Self::Error> {
        let ident = value.ident;
        let repr = value
            .attrs
            .iter()
            .find_map(|attr| {
                if &attr.path().get_ident()?.to_string() != "repr" {
                    return None;
                }
                attr.parse_args::<Type>().ok()
            })
            .ok_or_else(|| {
                syn::Error::new(ident.span(), "Missing `#[repr(..)]` attribute")
                    .into_compile_error()
            })?;
        let attrs = value
            .attrs
            .into_iter()
            .filter(|attr| {
                attr.path()
                    .get_ident()
                    .map_or(true, |name| &name.to_string() != "repr")
            })
            .collect();
        let variants = value
            .variants
            .into_iter()
            .map(|variant| {
                let (docs, other_attrs) =
                    variant
                        .attrs
                        .into_iter()
                        .partition::<Vec<Attribute>, _>(|attr| {
                            attr.path()
                                .get_ident()
                                .map_or(false, |name| &name.to_string() == "doc")
                        });
                if !other_attrs.is_empty() {
                    return Err(syn::Error::new(
                        variant.ident.span(),
                        "Unexpected non-documentation item on enum variant",
                    )
                    .into_compile_error());
                }
                if variant.fields != syn::Fields::Unit {
                    return Err(syn::Error::new(
                        variant.ident.span(),
                        "Unexpected non-unit enum variant",
                    )
                    .into_compile_error());
                }
                let discriminant = variant
                    .discriminant
                    .ok_or_else(|| {
                        syn::Error::new(
                            variant.ident.span(),
                            "Missing explicit discriminant on variant",
                        )
                        .into_compile_error()
                    })?
                    .1;
                Ok(Variant {
                    ident: variant.ident,
                    discriminant,
                    documentation: docs,
                })
            })
            .collect::<Result<Vec<Variant>, TokenStream>>()?;
        Ok(Self {
            vis: value.vis,
            attrs,
            ident,
            repr,
            variants,
        })
    }
}

#[doc = ""]
#[proc_macro_attribute]
pub fn pod_enum(_args: TokenStream1, input: TokenStream1) -> TokenStream1 {
    let ast = parse_macro_input!(input as syn::ItemEnum);

    let result = match PodEnum::try_from(ast) {
        Ok(result) => result,
        Err(e) => return e.into(),
    };

    result.write_impl().into()
}
