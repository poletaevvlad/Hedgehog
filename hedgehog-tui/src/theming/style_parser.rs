use cmd_parser::{skip_ws, take_token, CmdParsable, ParseError, ParseErrorKind};
use std::borrow::{Borrow, Cow};
use tui::style::{Color, Modifier, Style};

fn parse_modifier(input: &str) -> Result<Modifier, ParseError> {
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
        _ => {
            return Err(ParseError {
                kind: ParseErrorKind::TokenParse(input.to_string().into(), None),
                expected: "modifier".into(),
            })
        }
    };
    Ok(modifier)
}

fn parse_color_rgb(input: &str) -> Result<Color, ParseError> {
    if input.len() != 6 || input.chars().any(|ch| !ch.is_ascii_hexdigit()) {
        return Err(ParseError {
            kind: ParseErrorKind::TokenParse(format!("#{}", input).into(), None),
            expected: "color".into(),
        });
    }

    // Cannot fail
    let r = u8::from_str_radix(&input[0..2], 16).unwrap();
    let g = u8::from_str_radix(&input[2..4], 16).unwrap();
    let b = u8::from_str_radix(&input[4..6], 16).unwrap();
    Ok(Color::Rgb(r, g, b))
}

fn parse_color_xterm(input: &str) -> Result<Color, ParseError> {
    input.parse().map(Color::Indexed).map_err(|_| ParseError {
        kind: ParseErrorKind::TokenParse(format!("${}", input).into(), None),
        expected: "color".into(),
    })
}

fn parse_color_named(input: &str) -> Result<Color, ParseError> {
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
        _ => {
            return Err(ParseError {
                kind: ParseErrorKind::TokenParse(input.to_string().into(), None),
                expected: "color".into(),
            })
        }
    };
    Ok(color)
}

fn parse_color(input: &str) -> Result<Color, ParseError> {
    if let Some(color) = input.strip_prefix('#') {
        parse_color_rgb(color)
    } else if let Some(color) = input.strip_prefix('$') {
        parse_color_xterm(color)
    } else {
        parse_color_named(input)
    }
}

struct StyleCmd(Style);

fn token_to_str<'a>(token: &'a Option<Cow<'a, str>>) -> &'a str {
    match token {
        Some(Cow::Borrowed(token)) => *token,
        Some(Cow::Owned(token)) => token.as_str(),
        None => "",
    }
}

impl CmdParsable for StyleCmd {
    fn parse_cmd_raw(mut input: &str) -> Result<(Self, &str), cmd_parser::ParseError<'_>> {
        let mut style = Style::default();

        loop {
            input = if let Some(remaining) = input.strip_prefix("fg:") {
                let (color, remaining) = take_token(skip_ws(remaining));
                style =
                    style.fg(parse_color(token_to_str(&color)).map_err(ParseError::into_static)?);
                remaining
            } else if let Some(remaining) = input.strip_prefix("bg:") {
                let (color, remaining) = take_token(skip_ws(remaining));
                style =
                    style.bg(parse_color(token_to_str(&color)).map_err(ParseError::into_static)?);
                remaining
            } else if let Some(remaining) = input.strip_prefix('+') {
                let (modifier, remaining) = take_token(remaining);
                style = style.add_modifier(
                    parse_modifier(token_to_str(&modifier)).map_err(ParseError::into_static)?,
                );
                remaining
            } else if let Some(remaining) = input.strip_prefix('-') {
                let (modifier, remaining) = take_token(remaining);
                style = style.remove_modifier(
                    parse_modifier(token_to_str(&modifier)).map_err(ParseError::into_static)?,
                );
                remaining
            } else {
                break;
            };
        }
        Ok((StyleCmd(style), input))
    }
}

pub(crate) fn parse_cmd(input: &str) -> Result<(Style, &str), cmd_parser::ParseError<'_>> {
    let (style, remaining) = StyleCmd::parse_cmd(input)?;
    Ok((style.0, remaining))
}

#[cfg(test)]
mod tests {
    use super::parse_cmd;
    use tui::style::{Color, Modifier, Style};

    #[test]
    fn parse_modifiers() {
        assert_eq!(
            parse_cmd("  +bold +underlined -italic   ").unwrap().0,
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
            parse_cmd("bg: #5982af fg:$45 -bold").unwrap().0,
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
            parse_cmd("fg:black bg: white").unwrap().0,
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
            &parse_cmd("+nonexistant").unwrap_err().to_string(),
            "invalid modifier \"nonexistant\""
        );
    }

    #[test]
    fn invalid_color_rgb() {
        assert_eq!(
            &parse_cmd("fg: #0011").unwrap_err().to_string(),
            "invalid color \"#0011\""
        );
    }

    #[test]
    fn invalid_color_xterm() {
        assert_eq!(
            &parse_cmd("fg: $12345 +bold").unwrap_err().to_string(),
            "invalid color \"$12345\""
        );
    }

    #[test]
    fn invalid_color_named() {
        assert_eq!(
            &parse_cmd("bg:abcdef-bold").unwrap_err().to_string(),
            "invalid color \"abcdef-bold\""
        );
    }
}
