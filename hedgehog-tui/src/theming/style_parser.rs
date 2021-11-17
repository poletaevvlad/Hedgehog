use super::parser::ParsableStr;
use cmd_parser::CmdParsable;
use tui::style::{Color, Modifier, Style};

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("character unexpected: '{0}'")]
    UnexpectedCharacter(char),

    #[error("modifier unknown: '{0}'")]
    UnknownModifier(String),

    #[error("color is invalid: '{0}'")]
    InvalidColor(String),
}

fn modifier(input: &mut ParsableStr<'_>) -> Result<Modifier, Error> {
    let ident = input.take_while(char::is_ascii_alphanumeric);
    let modifier = match ident {
        "bold" => Modifier::BOLD,
        "crossedout" => Modifier::CROSSED_OUT,
        "dim" => Modifier::DIM,
        "hidden" => Modifier::HIDDEN,
        "italic" => Modifier::ITALIC,
        "rapidblink" => Modifier::RAPID_BLINK,
        "reversed" => Modifier::REVERSED,
        "slowblink" => Modifier::SLOW_BLINK,
        "underlined" => Modifier::UNDERLINED,
        _ => return Err(Error::UnknownModifier(ident.to_string())),
    };
    Ok(modifier)
}

fn color_rgb(input: &mut ParsableStr<'_>) -> Result<Color, Error> {
    let color_str = input.take_while(|ch| ch.is_digit(16));
    if color_str.len() != 6 {
        return Err(Error::InvalidColor(format!("#{}", color_str)));
    }

    // Cannot fail because color_str contain only valid hex characters
    let r = u8::from_str_radix(&color_str[0..2], 16).unwrap();
    let g = u8::from_str_radix(&color_str[2..4], 16).unwrap();
    let b = u8::from_str_radix(&color_str[4..6], 16).unwrap();
    Ok(Color::Rgb(r, g, b))
}

fn color_xterm(input: &mut ParsableStr<'_>) -> Result<Color, Error> {
    let color_str = input.take_while(|ch| ch.is_digit(10));
    color_str
        .parse()
        .map(Color::Indexed)
        .map_err(|_| Error::InvalidColor(format!("${}", color_str)))
}

fn color_named(input: &mut ParsableStr<'_>) -> Result<Color, Error> {
    let color_str = input.take_while(char::is_ascii_alphanumeric);
    let color = match color_str {
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
        _ => return Err(Error::InvalidColor(color_str.to_string())),
    };
    Ok(color)
}

fn color(input: &mut ParsableStr<'_>) -> Result<Color, Error> {
    if input.take_token("#") {
        color_rgb(input)
    } else if input.take_token("$") {
        color_xterm(input)
    } else {
        color_named(input)
    }
}

pub(crate) fn parse_style(input: &str) -> Result<Style, Error> {
    let mut input = ParsableStr::new(input);
    let mut style = Style::default();

    loop {
        input.take_while(char::is_ascii_whitespace);
        if input.is_empty() {
            break;
        }

        if input.take_token("fg:") {
            input.take_while(char::is_ascii_whitespace);
            style = style.fg(color(&mut input)?);
        } else if input.take_token("bg:") {
            input.take_while(char::is_ascii_whitespace);
            style = style.bg(color(&mut input)?);
        } else if input.take_token("+") {
            style = style.add_modifier(modifier(&mut input)?);
        } else if input.take_token("-") {
            style = style.remove_modifier(modifier(&mut input)?);
        } else {
            let ch = input.take();
            // input is non-empty
            return Err(Error::UnexpectedCharacter(ch.unwrap()));
        }
    }

    Ok(style)
}

struct StyleCmd(Style);

impl CmdParsable for StyleCmd {
    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), cmd_parser::ParseError<'_>> {
        let (token, remaining) = cmd_parser::take_token(input);
        match token {
            Some(token) => match parse_style(&token) {
                Ok(style) => Ok((StyleCmd(style), remaining)),
                Err(err) => Err(cmd_parser::ParseError {
                    kind: cmd_parser::ParseErrorKind::TokenParse(
                        token,
                        Some(err.to_string().into()),
                    ),
                    expected: "style".into(),
                }),
            },
            None => Err(cmd_parser::ParseError {
                kind: cmd_parser::ParseErrorKind::TokenRequired,
                expected: "style".into(),
            }),
        }
    }
}

pub(crate) fn parse_cmd(input: &str) -> Result<(Style, &str), cmd_parser::ParseError<'_>> {
    let (style, remaining) = StyleCmd::parse_cmd(input)?;
    Ok((style.0, remaining))
}

#[cfg(test)]
mod tests {
    use super::{parse_style, Error};
    use tui::style::{Color, Modifier, Style};

    #[test]
    fn parse_modifiers() {
        assert_eq!(
            parse_style("  +bold +underlined -italic   ").unwrap(),
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
            parse_style("bg: #5982af fg:$45 -bold").unwrap(),
            Style {
                fg: Some(Color::Indexed(45)),
                bg: Some(Color::Rgb(0x59, 0x82, 0xAF)),
                add_modifier: Modifier::empty(),
                sub_modifier: Modifier::BOLD,
            }
        )
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(
            parse_style("fg:black bg: white").unwrap(),
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
            parse_style("+nonexistant"),
            Err(Error::UnknownModifier("nonexistant".to_string()))
        )
    }

    #[test]
    fn invalid_color_rgb() {
        assert_eq!(
            parse_style("fg: #0011"),
            Err(Error::InvalidColor("#0011".to_string()))
        );
    }

    #[test]
    fn invalid_color_xterm() {
        assert_eq!(
            parse_style("bg: $12345 +bold"),
            Err(Error::InvalidColor("$12345".to_string()))
        );
    }

    #[test]
    fn invalid_color_named() {
        assert_eq!(
            parse_style("bg:abcdef-bold"),
            Err(Error::InvalidColor("abcdef".to_string()))
        )
    }
}
