use cmdparse::error::{ParseError, UnrecognizedToken};
use cmdparse::tokens::{Token, TokenStream};
use cmdparse::{CompletionResult, ParseResult};
use std::borrow::{Borrow, Cow};
use tui::style::{Color, Modifier, Style};

// These arrays must remain sorted
const COLOR_NAMES: &[&str] = &[
    "black",
    "blue",
    "cyan",
    "darkgray",
    "gray",
    "green",
    "lightblue",
    "lightcyan",
    "lightgreen",
    "lightmagenta",
    "lightred",
    "lightyellow",
    "magenta",
    "red",
    "reset",
    "white",
    "yellow",
];

const MODIFIER_NAMES: &[&str] = &[
    "bold",
    "crossedout",
    "dim",
    "hidden",
    "italic",
    "rapidblink",
    "reversed",
    "slowblink",
    "underlined",
];

fn parse_color_rgb(input: &str) -> Result<Color, ()> {
    if input.len() != 6 || input.chars().any(|ch| !ch.is_ascii_hexdigit()) {
        return Err(());
    }

    // Cannot fail
    let r = u8::from_str_radix(&input[0..2], 16).unwrap();
    let g = u8::from_str_radix(&input[2..4], 16).unwrap();
    let b = u8::from_str_radix(&input[4..6], 16).unwrap();
    Ok(Color::Rgb(r, g, b))
}

fn parse_color_xterm(input: &str) -> Result<Color, ()> {
    input.parse().map(Color::Indexed).map_err(|_| ())
}

fn parse_color_named(input: &str) -> Result<Color, ()> {
    let color = match input {
        "black" => Color::Black,
        "blue" => Color::Blue,
        "cyan" => Color::Cyan,
        "darkgray" => Color::DarkGray,
        "gray" => Color::Gray,
        "green" => Color::Green,
        "lightblue" => Color::LightBlue,
        "lightcyan" => Color::LightCyan,
        "lightgreen" => Color::LightGreen,
        "lightmagenta" => Color::LightMagenta,
        "lightred" => Color::LightRed,
        "lightyellow" => Color::LightYellow,
        "magenta" => Color::Magenta,
        "red" => Color::Red,
        "reset" => Color::Reset,
        "white" => Color::White,
        "yellow" => Color::Yellow,
        _ => return Err(()),
    };
    Ok(color)
}

fn parse_color(input: &str) -> Result<Color, ()> {
    if let Some(color) = input.strip_prefix('%') {
        parse_color_rgb(color)
    } else if let Some(color) = input.strip_prefix('$') {
        parse_color_xterm(color)
    } else {
        parse_color_named(input)
    }
}

fn parse_modifier(input: &str) -> Result<Modifier, ()> {
    let modifier = match input.borrow() {
        "bold" => Modifier::BOLD,
        "crossedout" => Modifier::CROSSED_OUT,
        "dim" => Modifier::DIM,
        "hidden" => Modifier::HIDDEN,
        "italic" => Modifier::ITALIC,
        "rapidblink" => Modifier::RAPID_BLINK,
        "reversed" => Modifier::REVERSED,
        "slowblink" => Modifier::SLOW_BLINK,
        "underlined" => Modifier::UNDERLINED,
        _ => return Err(()),
    };
    Ok(modifier)
}

#[derive(Debug, Default)]
pub struct StyleComponentParser;

impl<Ctx> cmdparse::Parser<Ctx> for StyleComponentParser {
    type Value = Style;

    fn parse<'a>(&self, input: TokenStream<'a>, _ctx: Ctx) -> ParseResult<'a, Self::Value> {
        match input.take().transpose()? {
            Some((token @ Token::Attribute(_), remaining)) => {
                Err(UnrecognizedToken::new(token, remaining).into())
            }
            Some((token @ Token::Text(text), remaining)) => {
                let text = text.parse_string();
                if let Some(color) = text.strip_prefix("fg:") {
                    parse_color(color)
                        .map(|color| (Style::default().fg(color), remaining))
                        .map_err(|_| {
                            ParseError::invalid(token, Some("invalid color".into())).into()
                        })
                } else if let Some(color) = text.strip_prefix("bg:") {
                    parse_color(color)
                        .map(|color| (Style::default().bg(color), remaining))
                        .map_err(|_| {
                            ParseError::invalid(token, Some("invalid color".into())).into()
                        })
                } else if let Some(modifier) = text.strip_prefix('+') {
                    parse_modifier(modifier)
                        .map(|modifier| (Style::default().add_modifier(modifier), remaining))
                        .map_err(|_| {
                            ParseError::invalid(token, Some("unknown modifier".into())).into()
                        })
                } else if let Some(modifier) = text.strip_prefix('-') {
                    parse_modifier(modifier)
                        .map(|modifier| (Style::default().remove_modifier(modifier), remaining))
                        .map_err(|_| {
                            ParseError::invalid(token, Some("unknown modifier".into())).into()
                        })
                } else {
                    Err(ParseError::invalid(token, None).into())
                }
            }
            None => Err(ParseError::token_required().expected("style").into()),
        }
    }

    fn complete<'a>(&self, input: TokenStream<'a>, _ctx: Ctx) -> cmdparse::CompletionResult<'a> {
        match input.take() {
            Some(Ok((Token::Text(text), remaining))) if remaining.is_all_consumed() => {
                let text = text.parse_string();
                if let Some(color) = text
                    .strip_prefix("fg:")
                    .or_else(|| text.strip_prefix("bg:"))
                {
                    CompletionResult::new_final(true).add_suggestions(
                        cmdparse::tokens::complete_variants(color, COLOR_NAMES).map(Cow::from),
                    )
                } else if let Some(modifier) = text.strip_prefix(&['+', '-'] as &[char]) {
                    CompletionResult::new_final(true).add_suggestions(
                        cmdparse::tokens::complete_variants(modifier, MODIFIER_NAMES)
                            .map(Cow::from),
                    )
                } else {
                    CompletionResult::new_final(true)
                }
            }
            Some(Ok((Token::Text(_), remaining))) => CompletionResult::new(remaining, true),
            Some(Ok((Token::Attribute(_), _))) => CompletionResult::new(input, false),
            Some(Err(_)) | None => CompletionResult::new_final(false),
        }
    }
}

#[derive(Default)]
pub struct ParsableStyle(Style);

impl cmdparse::parsers::ParsableCollection for ParsableStyle {
    type Item = Style;

    fn append(&mut self, item: Self::Item) {
        self.0 = self.0.patch(item);
    }
}

impl cmdparse::parsers::ParsableTransformation<Style> for ParsableStyle {
    type Input = Self;

    fn transform(input: Self::Input) -> Result<Style, ParseError<'static>> {
        Ok(input.0)
    }
}

pub type StyleParser = cmdparse::parsers::TransformParser<
    cmdparse::parsers::CollectionParser<ParsableStyle, StyleComponentParser>,
    ParsableStyle,
    Style,
>;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::StyleParser;
    use cmdparse::{complete_parser, parse_parser};
    use tui::style::{Color, Modifier, Style};

    #[test]
    fn parse_modifiers() {
        assert_eq!(
            parse_parser::<(), StyleParser>("  +bold +underlined -italic   ", ()).unwrap(),
            Style {
                fg: None,
                bg: None,
                add_modifier: Modifier::BOLD | Modifier::UNDERLINED,
                sub_modifier: Modifier::ITALIC
            }
        );
    }

    #[test]
    fn parse_color_rgb_indexed() {
        assert_eq!(
            parse_parser::<(), StyleParser>("bg:%5982af fg:$45 -bold", ()).unwrap(),
            Style {
                fg: Some(Color::Indexed(45)),
                bg: Some(Color::Rgb(0x59, 0x82, 0xAF)),
                add_modifier: Modifier::empty(),
                sub_modifier: Modifier::BOLD,
            }
        );
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(
            parse_parser::<(), StyleParser>("fg:black bg:white", ()).unwrap(),
            Style {
                fg: Some(Color::Black),
                bg: Some(Color::White),
                add_modifier: Modifier::empty(),
                sub_modifier: Modifier::empty()
            }
        );
    }

    #[test]
    fn unknown_modifier_error() {
        assert_eq!(
            parse_parser::<(), StyleParser>("+nonexistant", ())
                .unwrap_err()
                .to_string(),
            "cannot parse \"+nonexistant\" (unknown modifier)"
        );
    }

    #[test]
    fn invalid_color_rgb() {
        assert_eq!(
            &parse_parser::<(), StyleParser>("fg:%0011", ())
                .unwrap_err()
                .to_string(),
            "cannot parse \"fg:%0011\" (invalid color)"
        );
    }

    #[test]
    fn invalid_color_xterm() {
        assert_eq!(
            &parse_parser::<(), StyleParser>("fg:$12345 +bold", ())
                .unwrap_err()
                .to_string(),
            "cannot parse \"fg:$12345\" (invalid color)"
        );
    }

    #[test]
    fn invalid_color_named() {
        assert_eq!(
            &parse_parser::<(), StyleParser>("bg:abcdef-bold", ())
                .unwrap_err()
                .to_string(),
            "cannot parse \"bg:abcdef-bold\" (invalid color)"
        );
    }

    #[test]
    fn completion() {
        macro_rules! btreeset {
            [$($item:literal),*] => { BTreeSet::from([$($item.into()),*]) }
        }
        assert_eq!(
            complete_parser::<(), StyleParser>("fg:light", ()),
            btreeset!["blue", "cyan", "green", "magenta", "red", "yellow"]
        );
        assert_eq!(
            complete_parser::<(), StyleParser>("bg:light", ()),
            btreeset!["blue", "cyan", "green", "magenta", "red", "yellow"]
        );
        assert_eq!(
            complete_parser::<(), StyleParser>("+r", ()),
            btreeset!["apidblink", "eversed"]
        );
        assert_eq!(
            complete_parser::<(), StyleParser>("-r", ()),
            btreeset!["apidblink", "eversed"]
        );
        assert_eq!(
            complete_parser::<(), StyleParser>("unknown", ()),
            btreeset![]
        );
    }
}
