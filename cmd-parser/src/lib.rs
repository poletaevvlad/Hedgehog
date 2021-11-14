use std::borrow::Cow;
use std::fmt;
use std::num::{IntErrorKind, ParseIntError};

#[derive(Debug)]
pub enum ParseErrorKind<'a> {
    TokenParse(&'a str, Option<Cow<'static, str>>),
    TokenRequired,
}

#[derive(Debug)]
pub struct ParseError<'a> {
    kind: ParseErrorKind<'a>,
    expected: Cow<'static, str>,
}

impl<'a> ParseErrorKind<'a> {
    fn from_parse_int_error(token: &'a str, error: ParseIntError) -> Self {
        match error.kind() {
            IntErrorKind::PosOverflow => {
                ParseErrorKind::TokenParse(token, Some("too large".into()))
            }
            IntErrorKind::NegOverflow => {
                ParseErrorKind::TokenParse(token, Some("too small".into()))
            }
            _ => ParseErrorKind::TokenParse(token, None),
        }
    }
}

impl<'a> std::error::Error for ParseError<'a> {}

impl<'a> fmt::Display for ParseError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ParseErrorKind::TokenParse(token, error) => {
                f.write_fmt(format_args!("invalid {} \"{}\"", self.expected, token))?;
                if let Some(error) = error {
                    f.write_fmt(format_args!(": {}", error))?;
                }
                Ok(())
            }
            ParseErrorKind::TokenRequired => {
                f.write_fmt(format_args!("{} is required", self.expected))
            }
        }
    }
}

pub trait CmdParsable: Sized {
    fn parse_cmd(input: &str) -> Result<(Self, &str), ParseError<'_>>;
}

macro_rules! gen_parsable_int {
    ($type:ty) => {
        impl CmdParsable for $type {
            fn parse_cmd(input: &str) -> Result<(Self, &str), ParseError<'_>> {
                let (token, remaining) = take_token(input);
                let result = match token {
                    Some(token) => token
                        .parse()
                        .map(|num| (num, remaining))
                        .map_err(|error| ParseErrorKind::from_parse_int_error(token, error)),
                    None => Err(ParseErrorKind::TokenRequired),
                };
                result.map_err(|kind| ParseError {
                    kind,
                    expected: "integer".into(),
                })
            }
        }
    };
}

gen_parsable_int!(u8);
gen_parsable_int!(i8);
gen_parsable_int!(u16);
gen_parsable_int!(i16);
gen_parsable_int!(u32);
gen_parsable_int!(i32);
gen_parsable_int!(u64);
gen_parsable_int!(i64);
gen_parsable_int!(u128);
gen_parsable_int!(i128);

macro_rules! gen_parsable_float {
    ($type:ty) => {
        impl CmdParsable for $type {
            fn parse_cmd(input: &str) -> Result<(Self, &str), ParseError<'_>> {
                let (token, remaining) = take_token(input);
                let result = match token {
                    Some(token) => token
                        .parse()
                        .map(|num| (num, remaining))
                        .map_err(|_| ParseErrorKind::TokenParse(token, None)),
                    None => Err(ParseErrorKind::TokenRequired),
                };
                result.map_err(|kind| ParseError {
                    kind,
                    expected: "real number".into(),
                })
            }
        }
    };
}

gen_parsable_float!(f32);
gen_parsable_float!(f64);

fn skip_ws(mut input: &str) -> &str {
    loop {
        let mut chars = input.chars();
        match chars.next() {
            Some(ch) if ch.is_whitespace() => {
                input = chars.as_str();
            }
            None | Some(_) => return input,
        }
    }
}

pub fn take_token(input: &str) -> (Option<&str>, &str) {
    let mut input = skip_ws(input);
    if input.is_empty() {
        return (None, input);
    }

    let token_start = input;
    loop {
        let mut chars = input.chars();
        match chars.next() {
            Some(ch) if !ch.is_whitespace() => input = chars.as_str(),
            _ => break,
        }
    }
    let token = &token_start[..(token_start.len() - input.len())];
    (Some(token), skip_ws(input))
}

#[cfg(test)]
mod tests {
    use super::{take_token, CmdParsable};

    mod numbers {
        use super::*;

        #[test]
        fn parse_u8() {
            assert_eq!(u8::parse_cmd(" 15 ").unwrap(), (15, ""));
        }

        #[test]
        fn parse_f32() {
            assert_eq!(f32::parse_cmd(" 14.0 ").unwrap(), (14.0, ""));
        }

        #[test]
        fn parse_error() {
            assert_eq!(
                &i16::parse_cmd("123456781234567").unwrap_err().to_string(),
                "invalid integer \"123456781234567\": too large"
            );
        }

        #[test]
        fn parse_error_no_description() {
            assert_eq!(
                &i16::parse_cmd("abc").unwrap_err().to_string(),
                "invalid integer \"abc\""
            );
        }

        #[test]
        fn parse_float_error() {
            assert_eq!(
                &f32::parse_cmd("abc").unwrap_err().to_string(),
                "invalid real number \"abc\""
            );
        }
    }

    mod take_token_tests {
        use super::*;

        #[test]
        fn empty_string() {
            assert_eq!(take_token(""), (None, ""))
        }

        #[test]
        fn whitespace_only() {
            assert_eq!(take_token("   "), (None, ""))
        }

        #[test]
        fn takes_entire_string() {
            assert_eq!(take_token("abcdef"), (Some("abcdef"), ""));
        }

        #[test]
        fn takes_entire_string_with_whitespaces() {
            assert_eq!(take_token("  abcdef  "), (Some("abcdef"), ""));
        }

        #[test]
        fn tokenizes_multiple() {
            let mut input = "first second third";
            let mut tokens = Vec::new();
            loop {
                let (token, remaining) = take_token(input);
                if let Some(token) = token {
                    tokens.push(token);
                } else {
                    break;
                }
                input = remaining;
            }
            assert_eq!(tokens, vec!["first", "second", "third"]);
        }
    }
}
