mod attrs;

use attrs::{BuildableAttributes, VariantAttributes};
use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, spanned::Spanned, DataEnum, DataStruct, DeriveInput, Fields, Ident};

fn derive_fields(
    name: impl ToTokens,
    fields: &Fields,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    match fields {
        Fields::Named(fields) => {
            let field_parse = fields.named.iter().map(|field| {
                let ident = field.ident.as_ref().unwrap();
                let field_type = &field.ty;
                quote! { let (#ident, input) = <#field_type as ::cmd_parser::CmdParsable>::parse_cmd(input)?; }
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
                quote! { let (#ident, input) = <#field_type as ::cmd_parser::CmdParsable>::parse_cmd(input)?; }
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

fn derive_enum(name: Ident, data: DataEnum) -> Result<proc_macro2::TokenStream, syn::Error> {
    let mut variant_parse = Vec::new();
    let mut transparent_parse = Vec::new();
    for variant in data.variants.iter() {
        let variant_ident = &variant.ident;
        let variant_path = quote! { #name::#variant_ident };
        let parse_fields = derive_fields(variant_path, &variant.fields)?;

        let attrs = VariantAttributes::from_attributes(variant.attrs.iter())?;
        if attrs.transparent {
            transparent_parse.push(quote! {
                let parsed: ::std::result::Result<(#name ,&str), ::cmd_parser::ParseError> = (||{ #parse_fields })();
                if let Ok((result, remaining)) = parsed{
                    return Ok((result, remaining));
                }
            });
        } else {
            let mut discriminators = attrs.aliases;
            if !attrs.ignore {
                discriminators.push(variant.ident.to_string());
            }
            if discriminators.is_empty() {
                continue;
            }

            let pattern = discriminators.iter().enumerate().map(|(index, value)| {
                if index == 0 {
                    quote! { #value }
                } else {
                    quote! { | #value }
                }
            });
            variant_parse.push(quote! {
                #(#pattern)* => { #parse_fields }
            });
        }
    }
    Ok(quote! {
        impl cmd_parser::CmdParsable for #name {
            fn parse_cmd_raw(mut original_input: &str) -> Result<(Self, &str), cmd_parser::ParseError<'_>> {
                let (discriminator, input) = cmd_parser::take_token(original_input);
                let discriminator = match discriminator {
                    Some(discriminator) => discriminator,
                    None => return Err(cmd_parser::ParseError {
                        kind: cmd_parser::ParseErrorKind::TokenRequired,
                        expected: "name".into(),
                    }),
                };

                let d_str: &str = &discriminator;
                match d_str {
                    #(#variant_parse)*
                    _ => {
                        let input = original_input;
                        #(#transparent_parse)*
                        Err(cmd_parser::ParseError{
                            kind: cmd_parser::ParseErrorKind::UnknownVariant(discriminator),
                            expected: "name".into(),
                        })
                    }
                }
            }
        }
    })
}

fn derive_struct(name: Ident, data: DataStruct) -> Result<proc_macro2::TokenStream, syn::Error> {
    let struct_create = derive_fields(&name, &data.fields)?;

    Ok(quote! {
        impl cmd_parser::CmdParsable for #name {
            fn parse_cmd_raw(mut input: &str) -> Result<(Self, &str), cmd_parser::ParseError<'_>> {
                #struct_create
            }
        }
    })
}

#[proc_macro_derive(CmdParsable, attributes(cmd))]
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
