use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::parse_macro_input;
use syn::Expr;
use syn::ExprLit;
use syn::ExprRange;
use syn::Lit;

#[proc_macro]
pub fn generate_certificate_ids(input: TokenStream) -> TokenStream {
    let range: ExprRange = parse_macro_input!(input as ExprRange);

    let range = parse_range(range);

    let mut quotes = Vec::new();
    for i in range {
        let certificate_name = format_ident!("CERTIFICATE_ID_{}", i);
        quotes.push(quote! {
            pub const #certificate_name: ::topos_core::uci::CertificateId = ::topos_core::uci::CertificateId::from_array([#i; ::topos_core::uci::CERTIFICATE_ID_LENGTH]);
        });
    }

    TokenStream::from(quote! { #(#quotes)* })
}

#[proc_macro]
pub fn generate_source_subnet_ids(input: TokenStream) -> TokenStream {
    generate_subnet_ids("SOURCE", input)
}

#[proc_macro]
pub fn generate_target_subnet_ids(input: TokenStream) -> TokenStream {
    generate_subnet_ids("TARGET", input)
}

fn generate_subnet_ids(subnet_type: &str, input: TokenStream) -> TokenStream {
    let range: ExprRange = parse_macro_input!(input as ExprRange);

    let range = parse_range(range);

    let mut quotes = Vec::new();
    for (index, i) in range.enumerate() {
        let source_subnet_name = format_ident!("{}_SUBNET_ID_{}", subnet_type, index + 1);
        quotes.push(quote! {
            pub const #source_subnet_name: ::topos_core::uci::SubnetId = ::topos_core::uci::SubnetId::from_array([#i; ::topos_core::uci::SUBNET_ID_LENGTH]);
        });
    }

    TokenStream::from(quote! { #(#quotes)* })
}

fn parse_range(range: ExprRange) -> std::ops::Range<u8> {
    let from: u8 = if let Expr::Lit(ExprLit {
        lit: Lit::Int(value),
        ..
    }) = *range
        .from
        .expect("topos_test_sdk: generate cert/subnet, from input isn't valid")
    {
        value
            .base10_parse()
            .expect("topos_test_sdk: generate cert/subnet, unable to parse from int")
    } else {
        panic!("topos_test_sdk: generate cert/subnet, unable to parse from input");
    };

    let to: u8 = if let Expr::Lit(ExprLit {
        lit: Lit::Int(value),
        ..
    }) = *range
        .to
        .expect("topos_test_sdk: generate cert/subnet, to input isn't valid")
    {
        value
            .base10_parse()
            .expect("topos_test_sdk: generate cert/subnet, unable to parse to int")
    } else {
        panic!("topos_test_sdk: generate cert/subnet, unable to parse to input");
    };

    match range.limits {
        syn::RangeLimits::HalfOpen(_) => from..to,
        syn::RangeLimits::Closed(_) => from..(to + 1),
    }
}
