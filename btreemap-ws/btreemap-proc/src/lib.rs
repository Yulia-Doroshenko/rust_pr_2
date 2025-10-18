#![deny(clippy::all, clippy::pedantic)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, Result, Token,
};

struct Pair {
    key: Expr,
    _fat: Token![=>],
    val: Expr,
}

impl Parse for Pair {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self {
            key: input.parse()?,
            _fat: input.parse()?,
            val: input.parse()?,
        })
    }
}

struct Body {
    pairs: Punctuated<Pair, Token![,]>,
}

impl Parse for Body {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let pairs = Punctuated::<Pair, Token![,]>::parse_terminated(input)?;
        Ok(Self { pairs })
    }
}

#[proc_macro]
pub fn btreemap(input: TokenStream) -> TokenStream {
    if input.is_empty() {
        return quote! { ::std::collections::BTreeMap::new() }.into();
    }

    let Body { pairs } = syn::parse_macro_input!(input as Body);

    let inserts = pairs.iter().map(|Pair { key, val, .. }| {
        quote! { __m.insert(#key, #val); }
    });

    quote! {{
        let mut __m = ::std::collections::BTreeMap::new();
        #(#inserts)*
        __m
    }}
    .into()
}
