use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn main() {
    let mut out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    out_dir.push("config");
    if !out_dir.exists() {
        fs::create_dir(&out_dir).unwrap();
    }

    let mut assets_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    assets_dir.pop();
    assets_dir.push("assets");
    println!("cargo:rustc-env=HEDGEHOG_PATH={}", out_dir.display());

    if let Err(error) = copy_rc(&assets_dir, &out_dir) {
        panic!("Cannot copy RC: {}", error);
    }
    if let Err(error) = generate_themes(&assets_dir, &out_dir) {
        panic!("Failed to generate themes: {:?}", error);
    }
}

fn copy_rc(assets_dir: &Path, output_dir: &Path) -> std::io::Result<()> {
    let mut assets_dir = assets_dir.to_path_buf();
    assets_dir.push("rc");
    let mut output_dir = output_dir.to_path_buf();
    output_dir.push("rc");

    fs::copy(&assets_dir, &output_dir)?;
    println!("cargo:rerun-if-changed={}", assets_dir.display());
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct ThemeDefinition<'a> {
    name: &'a str,
    input: &'a str,
    data: Option<&'a str>,
    #[serde(default)]
    extra: HashMap<String, tera::Value>,
}

#[derive(Serialize, Deserialize)]
struct ThemeDefinitionSet<'a> {
    #[serde(borrow)]
    themes: Vec<ThemeDefinition<'a>>,
}

fn generate_themes(assets_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut assets_dir = assets_dir.to_path_buf();
    assets_dir.push("themes.toml");
    println!("cargo:rerun-if-changed={}", assets_dir.display());
    let themes_spec_string = std::fs::read_to_string(&assets_dir)?;
    let themes = toml::from_str::<ThemeDefinitionSet>(&themes_spec_string)?.themes;
    assets_dir.pop();
    assets_dir.push("themes");
    println!(
        "cargo:rerun-if-changed={}/macros.tera",
        assets_dir.display()
    );

    let mut output_dir = output_dir.to_path_buf();

    for theme in themes {
        output_dir.push(format!("{}.theme", theme.name));

        let data = if let Some(data) = theme.data {
            assets_dir.push(data);
            println!("cargo:rerun-if-changed={}", assets_dir.display());
            let data_string = fs::read_to_string(&assets_dir)?;
            assets_dir.pop();
            Some(toml::Value::from_str(&data_string)?)
        } else {
            None
        };

        assets_dir.push(theme.input);
        println!("cargo:rerun-if-changed={}", assets_dir.display());

        if let Some(data) = data {
            let template = fs::read_to_string(&assets_dir)?;
            assets_dir.pop();

            let mut tera = tera::Tera::default();
            tera.register_function("color_mix", color_mix);
            assets_dir.push("macros.tera");
            tera.add_template_file(&assets_dir, Some("macros"))?;
            tera.add_raw_template(theme.input, &template)?;

            let mut context = tera::Context::from_value(toml_value_to_tera(data))?;
            for (key, value) in theme.extra {
                context.insert(key, &value);
            }

            let file = fs::File::create(&output_dir)?;
            tera.render_to(theme.input, &context, file)?;
        } else {
            fs::copy(&assets_dir, &output_dir)?;
        }

        assets_dir.pop();
        output_dir.pop();
    }
    Ok(())
}

fn toml_value_to_tera(value: toml::Value) -> tera::Value {
    match value {
        toml::Value::String(string) => tera::Value::String(string),
        toml::Value::Integer(integer) => tera::Value::Number(integer.into()),
        toml::Value::Float(float) => {
            tera::Value::Number(tera::Number::from_f64(float).expect("Float is not a number"))
        }
        toml::Value::Boolean(boolean) => tera::Value::Bool(boolean),
        toml::Value::Datetime(_) => panic!("Datetime values aren't supported"),
        toml::Value::Array(values) => {
            tera::Value::Array(values.into_iter().map(toml_value_to_tera).collect())
        }
        toml::Value::Table(values) => tera::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, toml_value_to_tera(value)))
                .collect(),
        ),
    }
}

fn try_parse_color(value: &str) -> Result<(u8, u8, u8), ()> {
    if value.len() != 6 || !value.chars().all(|ch| char::is_ascii_hexdigit(&ch)) {
        return Err(());
    }
    let r = u8::from_str_radix(&value[0..2], 16).map_err(|_| ())?;
    let g = u8::from_str_radix(&value[2..4], 16).map_err(|_| ())?;
    let b = u8::from_str_radix(&value[4..6], 16).map_err(|_| ())?;
    Ok((r, g, b))
}

fn color_mix(args: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
    let (r1, g1, b1) = match args.get("bg") {
        Some(value) => try_parse_color(&tera::try_get_value!("color_mix", "bg", String, value))
            .map_err(|_| {
                tera::Error::msg(format!("invalid color passed as 'fg' parameter: {}", value))
            })?,
        None => {
            return Err(tera::Error::msg(
                "color_mix function required 'bg' parameter",
            ))
        }
    };
    let (r2, g2, b2) = match args.get("fg") {
        Some(value) => try_parse_color(&tera::try_get_value!("color_mix", "fg", String, value))
            .map_err(|_| {
                tera::Error::msg(format!("invalid color passed as 'fg' parameter: {}", value))
            })?,
        None => {
            return Err(tera::Error::msg(
                "color_mix function required 'fg' parameter",
            ))
        }
    };
    let factor = match args.get("f") {
        Some(value) => tera::try_get_value!("color_mix", "f", f64, value),
        None => {
            return Err(tera::Error::msg(
                "color_mix function required 'f' parameter",
            ))
        }
    };

    let red = float_to_int(r1 as f64 * factor + r2 as f64 * (1.0 - factor));
    let green = float_to_int(g1 as f64 * factor + g2 as f64 * (1.0 - factor));
    let blue = float_to_int(b1 as f64 * factor + b2 as f64 * (1.0 - factor));
    Ok(format!("{:02x}{:02x}{:02x}", red, green, blue).into())
}

fn float_to_int(value: f64) -> u8 {
    match value.round() {
        value if value <= 0.0 => 0,
        value if value >= 255.0 => 255,
        value => value as u8,
    }
}
