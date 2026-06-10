use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ItemImpl, LitStr, Token, Attribute};
use syn::parse::{Parse, ParseStream};

struct RbacArgs {
    scopes: Vec<String>,
    any_scopes: Vec<String>,
    profile: Option<String>,
    domain: Option<String>,
    actions: Vec<String>,
    any_actions: Vec<String>,
}

impl Parse for RbacArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = RbacArgs {
            scopes: Vec::new(),
            any_scopes: Vec::new(),
            profile: None,
            domain: None,
            actions: Vec::new(),
            any_actions: Vec::new(),
        };

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let lit: LitStr = input.parse()?;
            
            if ident == "scope" {
                args.scopes.push(lit.value());
            } else if ident == "any_scope" {
                args.any_scopes.push(lit.value());
            } else if ident == "profile" {
                args.profile = Some(lit.value());
            } else if ident == "domain" {
                args.domain = Some(lit.value());
            } else if ident == "action" {
                args.actions.push(lit.value());
            } else if ident == "any_action" {
                args.any_actions.push(lit.value());
            } else {
                return Err(syn::Error::new(ident.span(), "Unknown attribute argument. Supported: scope, any_scope, profile, domain, action, any_action"));
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(args)
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
/// them to include the domain: `#[rbac(domain = "...", action = "...")]`.
#[proc_macro_attribute]
pub fn rbac_controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    let domain_args = parse_macro_input!(attr as DomainArgs);
    let mut impl_block = parse_macro_input!(item as ItemImpl);
    let domain = domain_args.domain;

    for impl_item in &mut impl_block.items {
        if let syn::ImplItem::Fn(method) = impl_item {
            for attr in &mut method.attrs {
                if attr.path().is_ident("rbac") {
                    let attr_tokens = attr.meta.require_list().map(|l| l.tokens.clone()).unwrap_or_default();
                    
                    if let Ok(rbac_args) = syn::parse2::<RbacArgs>(attr_tokens) {
                        let mut new_args = proc_macro2::TokenStream::new();
                        
                        // Inject domain
                        new_args.extend(quote! { domain = #domain, });
                        
                        for action in rbac_args.actions {
                            new_args.extend(quote! { action = #action, });
                        }
                        for action in rbac_args.any_actions {
                            new_args.extend(quote! { any_action = #action, });
                        }
                        for scope in rbac_args.scopes {
                            new_args.extend(quote! { scope = #scope, });
                        }
                        for scope in rbac_args.any_scopes {
                            new_args.extend(quote! { any_scope = #scope, });
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
/// If `domain` and `action` are provided, it delegates the policy lookup dynamically
/// to `crate::rbac::get_policy(domain, action)`.
fn extract_profile_expr(profile: Option<String>) -> Result<proc_macro2::TokenStream, syn::Error> {
    if let Some(p) = profile {
        match syn::parse_str::<syn::Expr>(&p) {
            Ok(expr) => Ok(quote! { Some(#expr) }),
            Err(err) => Err(err),
        }
    } else {
        Ok(quote! { None })
    }
}

fn extract_claims_ident(input_fn: &ItemFn) -> Option<syn::Ident> {
    for arg in &input_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                if pat_ident.ident == "claims" || pat_ident.ident == "_claims" {
                    return Some(pat_ident.ident.clone());
                }
            }
        }
    }
    None
}

fn is_tonic_result(output: &syn::ReturnType) -> bool {
    if let syn::ReturnType::Type(_, ty) = output {
        let ty_str = quote!(#ty).to_string().replace(" ", "");
        ty_str.contains(",Status>") || ty_str.contains(",tonic::Status>") || ty_str.contains("tonic::Response<") || ty_str.contains("Response<")
    } else {
        false
    }
}

fn generate_err_return(is_tonic: bool) -> proc_macro2::TokenStream {
    if is_tonic {
        quote! { return Box::pin(async move { Err(tonic::Status::permission_denied(e)) }); }
    } else {
        quote! { return Box::pin(async move { Err(cowen_common::CowenError::Auth(e).into()) }); }
    }
}

fn generate_domain_check_block(
    args: &RbacArgs,
    domain: &str,
    claims_expr: &proc_macro2::TokenStream,
    profile_expr: &proc_macro2::TokenStream,
    err_return: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let mut actions_exprs = Vec::new();
    for action in &args.actions {
        actions_exprs.push(quote! { crate::rbac::get_policy(#domain, #action) });
    }
    for any_action in &args.any_actions {
        actions_exprs.push(quote! { crate::rbac::get_policy(#domain, #any_action) });
    }
    
    let scopes_iter = args.scopes.iter();
    let any_scopes_iter = args.any_scopes.iter();
    
    quote! {
        let mut all_scopes_dyn: Vec<&str> = Vec::new();
        let mut any_scopes_dyn: Vec<&str> = Vec::new();
        #(
            let (all, any) = #actions_exprs;
            all_scopes_dyn.extend(all);
            any_scopes_dyn.extend(any);
        )*
        
        let static_scopes: &[&str] = &[ #(#scopes_iter),* ];
        all_scopes_dyn.extend(static_scopes);
        
        let static_any_scopes: &[&str] = &[ #(#any_scopes_iter),* ];
        any_scopes_dyn.extend(static_any_scopes);
        
        let claims_opt = #claims_expr;
        if let Err(e) = crate::rbac::verify_permission(claims_opt, #profile_expr, &all_scopes_dyn, &any_scopes_dyn) {
            #err_return
        }
    }
}

fn generate_static_check_block(
    args: &RbacArgs,
    claims_expr: &proc_macro2::TokenStream,
    profile_expr: &proc_macro2::TokenStream,
    err_return: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let scopes_iter = args.scopes.iter();
    let all_scopes_expr = quote! { &[ #(#scopes_iter),* ] };

    let any_scopes_iter = args.any_scopes.iter();
    let any_scopes_expr = quote! { &[ #(#any_scopes_iter),* ] };
    
    quote! {
        let claims_opt = #claims_expr;
        if let Err(e) = crate::rbac::verify_permission(claims_opt, #profile_expr, #all_scopes_expr, #any_scopes_expr) {
            #err_return
        }
    }
}

#[proc_macro_attribute]
pub fn rbac(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as RbacArgs);
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let profile_expr = match extract_profile_expr(args.profile.clone()) {
        Ok(expr) => expr,
        Err(err) => return err.to_compile_error().into(),
    };

    let claims_ident = extract_claims_ident(&input_fn);
    let is_tonic = is_tonic_result(&input_fn.sig.output);

    let claims_expr = if let Some(ident) = &claims_ident {
        quote! { #ident }
    } else {
        quote! { claims }
    };

    let err_return = generate_err_return(is_tonic);

    let check_block = if let Some(domain) = &args.domain {
        generate_domain_check_block(&args, domain, &claims_expr, &profile_expr, &err_return)
    } else {
        generate_static_check_block(&args, &claims_expr, &profile_expr, &err_return)
    };

    let original_block = &input_fn.block;
    let claims_let = if claims_ident.is_some() {
        quote! {}
    } else {
        quote! { let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>(); }
    };

    let new_block = quote! {
        {
            #claims_let
            #check_block
            #original_block
        }
    };

    *input_fn.block = syn::parse2(new_block).expect("Failed to parse the injected block");

    TokenStream::from(quote! {
        #input_fn
    })
}
