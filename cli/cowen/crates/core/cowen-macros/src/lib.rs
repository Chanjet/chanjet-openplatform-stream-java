use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ItemImpl, LitStr, Token, Attribute};
use syn::parse::{Parse, ParseStream};

struct RbacArgs {
    scope: Option<String>,
    profile: Option<String>,
    action: Option<String>,
}

impl Parse for RbacArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut scope = None;
        let mut profile = None;
        let mut action = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let lit: LitStr = input.parse()?;
            
            if ident == "scope" {
                scope = Some(lit.value());
            } else if ident == "profile" {
                profile = Some(lit.value());
            } else if ident == "action" {
                action = Some(lit.value());
            } else {
                return Err(syn::Error::new(ident.span(), "Unknown attribute argument. Supported: scope, profile, action"));
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(RbacArgs { scope, profile, action })
    }
}

struct DomainArgs {
    domain: String,
}

impl Parse for DomainArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        if ident != "domain" {
            return Err(syn::Error::new(ident.span(), "Expected `domain`"));
        }
        input.parse::<Token![=]>()?;
        let lit: LitStr = input.parse()?;
        Ok(DomainArgs { domain: lit.value() })
    }
}

/// A procedural macro for controller implementations to specify a common RBAC domain.
/// It preprocesses all `#[rbac(action = "...")]` attributes on methods and rewrites
/// them into `#[rbac(scope = "domain:action")]`.
#[proc_macro_attribute]
pub fn rbac_controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    let domain_args = parse_macro_input!(attr as DomainArgs);
    let mut impl_block = parse_macro_input!(item as ItemImpl);
    let domain = domain_args.domain;

    for impl_item in &mut impl_block.items {
        if let syn::ImplItem::Fn(method) = impl_item {
            for attr in &mut method.attrs {
                if attr.path().is_ident("rbac") {
                    // Extract tokens from the attribute to parse
                    let attr_tokens = attr.meta.require_list().map(|l| l.tokens.clone()).unwrap_or_default();
                    
                    if let Ok(rbac_args) = syn::parse2::<RbacArgs>(attr_tokens) {
                        let mut new_args = proc_macro2::TokenStream::new();
                        
                        if let Some(action) = rbac_args.action {
                            let scope = format!("{}:{}", domain, action);
                            new_args.extend(quote! { scope = #scope, });
                        } else if let Some(scope) = rbac_args.scope {
                            new_args.extend(quote! { scope = #scope, });
                        }
                        
                        if let Some(profile) = rbac_args.profile {
                            new_args.extend(quote! { profile = #profile, });
                        }
                        
                        let new_attr: Attribute = syn::parse_quote!( #[rbac(#new_args)] );
                        *attr = new_attr;
                    }
                }
            }
        }
    }

    TokenStream::from(quote! { #impl_block })
}

/// A procedural macro to enforce RBAC permissions on a gRPC controller method.
///
/// Note: Since this is used within `#[tonic::async_trait]`, the target function 
/// is actually transformed into a synchronous function returning `Pin<Box<dyn Future>>`.
/// Therefore, we inject the RBAC check as a synchronous step that returns an error future
/// immediately if the check fails.
#[proc_macro_attribute]
pub fn rbac(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as RbacArgs);
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let scope_expr = if let Some(s) = args.scope {
        quote! { Some(#s) }
    } else {
        quote! { None }
    };

    let profile_expr = if let Some(p) = args.profile {
        let expr: syn::Expr = match syn::parse_str(&p) {
            Ok(e) => e,
            Err(err) => return err.to_compile_error().into(),
        };
        quote! { Some(#expr) }
    } else {
        quote! { None }
    };

    let original_block = &input_fn.block;

    // We inject the check_rbac call at the beginning of the block.
    // We return a Box::pin future directly if the check fails.
    let new_block = quote! {
        {
            if let Err(e) = crate::controller::check_rbac(&request, #profile_expr, #scope_expr) {
                return Box::pin(async move { Err(e) });
            }
            #original_block
        }
    };

    input_fn.block = Box::new(syn::parse2(new_block).expect("Failed to parse the injected block"));

    TokenStream::from(quote! {
        #input_fn
    })
}
