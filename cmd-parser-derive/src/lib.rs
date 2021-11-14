use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned, DataEnum, DataStruct, DeriveInput, Fields, Ident};

fn derive_fields(name: &Ident, fields: Fields) -> Result<proc_macro2::TokenStream, syn::Error> {
    match fields {
        Fields::Named(fields) => {
            let field_parse = fields.named.iter().map(|field| {
                let ident = field.ident.as_ref().unwrap();
                let field_type = &field.ty;
                quote! { let (#ident, input) = #field_type::parse_cmd(input)?; }
            });
            let field_idents = fields
                .named
                .iter()
                .map(|field| field.ident.as_ref().unwrap());
            Ok(quote! {
                #(#field_parse)*
                Ok((#name { #(#field_idents),* }, input))
            })
        }
        Fields::Unnamed(fields) => {
            let field_parse = fields.unnamed.iter().enumerate().map(|(index, field)| {
                let ident = format_ident!("field_{}", index);
                let field_type = &field.ty;
                quote! { let (#ident, input) = #field_type::parse_cmd(input)?; }
            });
            let field_var_names =
                (0..fields.unnamed.len()).map(|index| format_ident!("field_{}", index));

            Ok(quote! {
                #(#field_parse)*
                Ok((#name(#(#field_var_names),*), input))
            })
        }
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
