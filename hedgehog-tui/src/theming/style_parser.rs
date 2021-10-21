use nom::branch::alt;
use nom::bytes::complete::{tag, take_while, take_while_m_n};
use nom::character::complete::multispace0;
use nom::combinator::{map, map_res, value};
use nom::error::Error;
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;
use tui::style::{Color, Modifier, Style};

fn modifier(i: &str) -> IResult<&str, Modifier> {
    alt((
        value(Modifier::BOLD, tag("bold")),
        value(Modifier::CROSSED_OUT, tag("crossedout")),
        value(Modifier::DIM, tag("dim")),
        value(Modifier::HIDDEN, tag("hidden")),
        value(Modifier::ITALIC, tag("italic")),
        value(Modifier::RAPID_BLINK, tag("rapidblink")),
        value(Modifier::REVERSED, tag("reversed")),
        value(Modifier::SLOW_BLINK, tag("slowblink")),
        value(Modifier::UNDERLINED, tag("underlined")),
    ))(i)
}

fn color_rgb(i: &str) -> IResult<&str, Color> {
    fn hex_color_component(i: &str) -> IResult<&str, u8> {
        map_res(take_while_m_n(2, 2, |ch: char| ch.is_digit(16)), |hex| {
            u8::from_str_radix(hex, 16)
        })(i)
    }

    let color = map(
        tuple((
            hex_color_component,
            hex_color_component,
            hex_color_component,
        )),
        |(r, g, b)| Color::Rgb(r, g, b),
    );
    preceded(tag("#"), color)(i)
}

fn color_xterm(i: &str) -> IResult<&str, Color> {
    let index = map_res(take_while(|ch: char| ch.is_digit(10)), str::parse);
    let color = map(index, Color::Indexed);
    preceded(tag("$"), color)(i)
}

fn color(i: &str) -> IResult<&str, Color> {
    alt((
        color_rgb,
        color_xterm,
        value(Color::Black, tag("black")),
        value(Color::Blue, tag("blue")),
        value(Color::Cyan, tag("cyan")),
        value(Color::DarkGray, tag("darkgray")),
        value(Color::Gray, tag("gray")),
        value(Color::Green, tag("green")),
        value(Color::LightBlue, tag("lightblue")),
        value(Color::LightCyan, tag("lightcyan")),
        value(Color::LightGreen, tag("lightgreen")),
        value(Color::LightMagenta, tag("lightmagenta")),
        value(Color::LightRed, tag("lightred")),
        value(Color::LightYellow, tag("lightyellow")),
        value(Color::Magenta, tag("magenta")),
        value(Color::Red, tag("red")),
        value(Color::Reset, tag("reset")),
        value(Color::White, tag("white")),
        value(Color::Yellow, tag("yellow")),
    ))(i)
}

fn ws<'a, O>(
    parser: impl Fn(&'a str) -> IResult<&'a str, O>,
) -> impl FnMut(&'a str) -> IResult<&'a str, O> {
    delimited(multispace0, parser, multispace0)
}

enum StylePart {
    AddMod(Modifier),
    RemoveMod(Modifier),
    SetFg(Color),
    SetBg(Color),
}

fn style_part(i: &str) -> IResult<&str, StylePart> {
    alt((
        map(preceded(tag("+"), modifier), StylePart::AddMod),
        map(preceded(tag("-"), modifier), StylePart::RemoveMod),
        map(preceded(tag("bg:"), ws(color)), StylePart::SetBg),
        map(preceded(tag("fg:"), ws(color)), StylePart::SetFg),
    ))(i)
}

fn extend_style(style: Style, part: StylePart) -> Style {
    match part {
        StylePart::AddMod(modifier) => style.add_modifier(modifier),
        StylePart::RemoveMod(modifier) => style.remove_modifier(modifier),
        StylePart::SetFg(foreground) => style.fg(foreground),
        StylePart::SetBg(background) => style.bg(background),
    }
}

pub(crate) fn parse_style(mut input: &str) -> Result<Style, Error<&str>> {
    let mut style = Style::default();
    while !input.is_empty() {
        match ws(style_part)(input) {
            Ok((tail, part)) => {
                style = extend_style(style, part);
                input = tail;
            }
            Err(nom::Err::Error(err)) => return Err(err),
            Err(nom::Err::Failure(err)) => return Err(err),
            Err(nom::Err::Incomplete(_)) => unreachable!(),
        }
    }
    Ok(style)
}

#[cfg(test)]
mod tests {
    use super::parse_style;
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
}
