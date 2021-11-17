mod attrs;

use attrs::{BuildableAttributes, FieldAttributes, VariantAttributes};
use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::str::FromStr;
use syn::{parse_macro_input, spanned::Spanned, DataEnum, DataStruct, DeriveInput, Fields, Ident};

fn to_kebab_case(ident: &str) -> String {
    let mut result = String::new();
    for (i, ch) in ident.chars().enumerate() {
        let lowercase = ch.to_ascii_lowercase();
        if i > 0 && ch != lowercase {
            result.push('-');
        }
        result.push(lowercase);
    }
    result
}

fn derive_fields(
    name: impl ToTokens,
    fields: &Fields,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let fields_data = match fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(fields) => &fields.unnamed,
        Fields::Unit => return Ok(quote! {Ok((#name, input))}),
    };

    let mut field_parse = Vec::new();
    let mut field_construct = Vec::new();
    for (index, field) in fields_data.iter().enumerate() {
        let ident = format_ident!("field_{}", index);
        if let Some(field_ident) = field.ident.as_ref() {
            field_construct.push(quote! {#field_ident: #ident});
        } else {
            field_construct.push(quote! {#ident});
        }

        let attr = FieldAttributes::from_attributes(field.attrs.iter())?;
        let parse_expr = attr
            .parse_with
            .map(|parse_with| {
                let fn_path = proc_macro2::TokenStream::from_str(&parse_with).unwrap();
                quote! {#fn_path(input)}
            })
            .unwrap_or_else(|| {
                let field_type = &field.ty;
                quote! { <#field_type as ::cmd_parser::CmdParsable>::parse_cmd(input) }
            });

        field_parse.push(quote! { let (#ident, input) = #parse_expr?; })
    }

    if let Fields::Named(_) = fields {
        Ok(quote! {
            #(#field_parse)*
            Ok((#name { #(#field_construct),* }, input))
        })
    } else {
        Ok(quote! {
            #(#field_parse)*
            Ok((#name(#(#field_construct),*), input))
        })
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
                let label = to_kebab_case(&variant.ident.to_string());
                discriminators.push(label);
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

#[cfg(test)]
mod tests {
    use super::to_kebab_case;

    #[test]
    fn rename_kebab_case() {
        assert_eq!(&to_kebab_case("Word"), "word");
        assert_eq!(&to_kebab_case("TwoWords"), "two-words");
    }
}
