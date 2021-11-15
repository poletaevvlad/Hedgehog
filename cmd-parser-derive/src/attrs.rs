use syn::{spanned::Spanned, Attribute, Error, Lit, Meta, MetaNameValue, NestedMeta, Path};

#[derive(Default)]
pub(crate) struct VariantAttributes {
    pub(crate) aliases: Vec<String>,
    pub(crate) ignore: bool,
    pub(crate) transparent: bool,
}

impl VariantAttributes {
    pub(crate) fn from_attributes<'a>(
        attrs: impl Iterator<Item = &'a Attribute>,
    ) -> Result<Self, Error> {
        let mut attributes = VariantAttributes::default();

        for attr in attrs {
            let meta = attr.parse_meta()?;
            let inner = match meta {
                Meta::Path(path) if compare_path(&path, "cmd") => {
                    return Err(Error::new(path.span(), "Missing argument parameters"));
                }
                Meta::NameValue(name_value) if compare_path(&name_value.path, "cmd") => {
                    return Err(Error::new(
                        name_value.span(),
                        "Key-value argument style is not allowed",
                    ));
                }
                Meta::List(list) if compare_path(&list.path, "cmd") => list,
                _ => continue,
            };

            for nested in inner.nested.iter() {
                let meta = match nested {
                    NestedMeta::Meta(meta) => meta,
                    NestedMeta::Lit(lit) => {
                        return Err(Error::new(lit.span(), "Unexpected literal"))
                    }
                };
                match meta {
                    Meta::NameValue(name_value) if compare_path(&name_value.path, "rename") => {
                        attributes.aliases.push(get_name_value_string(name_value)?);
                        attributes.ignore = true;
                    }
                    Meta::NameValue(name_value) if compare_path(&name_value.path, "alias") => {
                        attributes.aliases.push(get_name_value_string(name_value)?);
                    }
                    Meta::Path(path) if compare_path(path, "ignore") => {
                        attributes.ignore = true;
                    }
                    Meta::Path(path) if compare_path(path, "transparent") => {
                        attributes.transparent = true;
                    }
                    Meta::Path(path) => {
                        return Err(Error::new(path.span(), "Unknown argument"));
                    }
                    Meta::NameValue(name_value) => {
                        return Err(Error::new(name_value.path.span(), "Unknown argument"));
                    }
                    Meta::List(list) => {
                        return Err(Error::new(list.span(), "Unknown argument"));
                    }
                }
            }
        }

        Ok(attributes)
    }
}

fn get_name_value_string(name_value: &MetaNameValue) -> Result<String, Error> {
    if let Lit::Str(string) = &name_value.lit {
        Ok(string.value())
    } else {
        Err(Error::new(name_value.lit.span(), "Expected a string"))
    }
}

fn compare_path(path: &Path, name: &str) -> bool {
    if path.segments.len() != 1 {
        return false;
    }
    let segment = &path.segments[0];
    segment.arguments.is_empty() && segment.ident == name
}
