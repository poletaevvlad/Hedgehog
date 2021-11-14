use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, DataEnum, DataStruct, DeriveInput, Fields, Ident};

fn derive_fields(name: &Ident, fields: Fields) -> Result<proc_macro2::TokenStream, syn::Error> {
    match fields {
        Fields::Named(_) => todo!(),
        Fields::Unnamed(_) => todo!(),
        Fields::Unit => Ok(quote! {Ok((#name, input))}),
    }
}

fn derive_enum(_name: Ident, _data: DataEnum) -> Result<proc_macro2::TokenStream, syn::Error> {
    todo!();
}

fn derive_struct(name: Ident, data: DataStruct) -> Result<proc_macro2::TokenStream, syn::Error> {
    let struct_create = derive_fields(&name, data.fields)?;

    Ok(quote! {
        impl cmd_parser::CmdParsable for #name {
            fn parse_cmd_raw(mut input: &str) -> Result<(Self, &str), cmd_parser::ParseError<'_>> {
                #struct_create
            }
        }
    })
}

#[proc_macro_derive(CmdParsable)]
pub fn derive_parseable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let result = match input.data {
        syn::Data::Struct(data) => derive_struct(name, data),
        syn::Data::Enum(data) => derive_enum(name, data),
        syn::Data::Union(data) => Err(syn::Error::new(
            data.union_token.span(),
            "parsing unions is not supported",
        )),
    };
    match result {
        Ok(token_stream) => token_stream.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
