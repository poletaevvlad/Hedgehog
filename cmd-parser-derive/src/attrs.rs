use syn::{
    spanned::Spanned, Attribute, Error, Lit, Meta, MetaList, MetaNameValue, NestedMeta, Path,
};

#[derive(Default)]
pub(crate) struct VariantAttributes {
    pub(crate) aliases: Vec<String>,
    pub(crate) ignore: bool,
    pub(crate) transparent: bool,
}

pub(crate) trait BuildableAttributes {
    fn visit_name_value(&mut self, name_value: &MetaNameValue) -> Result<(), Error>;
    fn visit_path(&mut self, path: &Path) -> Result<(), Error>;
    fn visit_list(&mut self, list: &MetaList) -> Result<(), Error>;

    fn from_attributes<'a>(attrs: impl Iterator<Item = &'a Attribute>) -> Result<Self, Error>
    where
        Self: Default,
    {
        let mut attributes = Self::default();

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
                    Meta::Path(path) => attributes.visit_path(path)?,
                    Meta::NameValue(name_value) => attributes.visit_name_value(name_value)?,
                    Meta::List(list) => attributes.visit_list(list)?,
                }
            }
        }

        Ok(attributes)
    }
}

impl BuildableAttributes for VariantAttributes {
    fn visit_name_value(&mut self, name_value: &MetaNameValue) -> Result<(), Error> {
        if compare_path(&name_value.path, "rename") {
            self.aliases.push(get_name_value_string(name_value)?);
            self.ignore = true;
        } else if compare_path(&name_value.path, "alias") {
            self.aliases.push(get_name_value_string(name_value)?);
        } else {
            return Err(Error::new(name_value.span(), "Unknown argument"));
        }
        Ok(())
    }

    fn visit_path(&mut self, path: &Path) -> Result<(), Error> {
        if compare_path(path, "ignore") {
            self.ignore = true;
        } else if compare_path(path, "transparent") {
            self.transparent = true;
        } else {
            return Err(Error::new(path.span(), "Unknown argument"));
        }
        Ok(())
    }

    fn visit_list(&mut self, list: &MetaList) -> Result<(), Error> {
        Err(Error::new(list.span(), "Unknown argument"))
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
