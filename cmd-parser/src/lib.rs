pub use cmd_parser_derive::*;

use std::borrow::{Borrow, Cow};
use std::fmt;
use std::num::{IntErrorKind, ParseIntError};
use std::time::Duration;

#[derive(Debug)]
pub enum ParseErrorKind<'a> {
    TokenParse(Cow<'a, str>, Option<Cow<'static, str>>),
    TokenRequired,
    UnexpectedToken(Cow<'a, str>),
    UnbalancedParenthesis,
    UnknownVariant(Cow<'a, str>),
}

#[derive(Debug)]
pub struct ParseError<'a> {
    pub kind: ParseErrorKind<'a>,
    pub expected: Cow<'static, str>,
}

impl<'a> ParseErrorKind<'a> {
    fn from_parse_int_error(token: Cow<'a, str>, error: ParseIntError) -> Self {
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
                f.write_fmt(format_args!("expected {}", self.expected))
            }
            ParseErrorKind::UnexpectedToken(token) => {
                f.write_fmt(format_args!("unexpected token: \"{}\"", token))
            }
            ParseErrorKind::UnbalancedParenthesis => f.write_str("unbalanced parenthesis"),
            ParseErrorKind::UnknownVariant(variant) => {
                f.write_fmt(format_args!("unknown variant: \"{}\"", variant))
            }
        }
    }
}

pub trait CmdParsable: Sized {
    fn parse_cmd(mut input: &str) -> Result<(Self, &str), ParseError<'_>> {
        input = skip_ws(input);

        if let Some(input) = input.strip_prefix('(') {
            let (value, mut remaining) = Self::parse_cmd_raw(input)?;
            if remaining.starts_with(')') {
                remaining = skip_ws(&remaining[1..])
            } else {
                let (token, _) = take_token(remaining);
                if let Some(token) = token {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnexpectedToken(token),
                        expected: "".into(),
                    });
                }
            }
            Ok((value, remaining))
        } else {
            Self::parse_cmd_raw(input)
        }
    }

    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>>;
}

impl CmdParsable for bool {
    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>> {
        let (token, remaining) = take_token(input);
        let token = match token {
            Some(token) => token,
            None => {
                return Err(ParseError {
                    kind: ParseErrorKind::TokenRequired,
                    expected: "boolean".into(),
                })
            }
        };
        let value = match token.borrow() {
            "true" | "t" | "yes" | "y" => true,
            "false" | "f" | "no" | "n" => false,
            _ => {
                return Err(ParseError {
                    kind: ParseErrorKind::TokenParse(token, None),
                    expected: "boolean".into(),
                })
            }
        };
        Ok((value, remaining))
    }
}

macro_rules! gen_parsable_int {
    ($type:ty) => {
        impl CmdParsable for $type {
            fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>> {
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
            fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>> {
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

impl CmdParsable for String {
    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>> {
        let (token, remaining) = take_token(input);
        match token {
            Some(token) => Ok((token.into_owned(), remaining)),
            None => Err(ParseError {
                kind: ParseErrorKind::TokenRequired,
                expected: "string".into(),
            }),
        }
    }
}

impl<T: CmdParsable> CmdParsable for Vec<T> {
    fn parse_cmd_raw(mut input: &str) -> Result<(Self, &str), ParseError<'_>> {
        let mut result = Vec::new();
        while has_tokens(input) {
            let (item, remaining) = T::parse_cmd(input)?;
            input = remaining;
            result.push(item);
        }
        Ok((result, input))
    }
}

macro_rules! gen_parsable_tuple {
    ($($name:ident)*) => {
        impl<$($name: CmdParsable),*> CmdParsable for ($($name),*,) {
            #[allow(non_snake_case)]
            fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>> {
                $(let ($name, input) = $name::parse_cmd(input)?;)*
                Ok((($($name),*,), input))
            }
        }
    }
}

gen_parsable_tuple!(T1);
gen_parsable_tuple!(T1 T2);
gen_parsable_tuple!(T1 T2 T3);
gen_parsable_tuple!(T1 T2 T3 T4);
gen_parsable_tuple!(T1 T2 T3 T4 T5);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);
gen_parsable_tuple!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16);

impl CmdParsable for Duration {
    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>> {
        let (token, input) = take_token(input);
        match token {
            Some(token) => {
                let mut parts = token.rsplitn(3, ':');
                let mut seconds: f64 = match parts.next().unwrap().parse() {
                    Ok(seconds) => seconds,
                    Err(_) => {
                        return Err(ParseError {
                            kind: ParseErrorKind::TokenParse(token, None),
                            expected: "duration".into(),
                        })
                    }
                };

                let mut multiplier = 60.0;
                for part in parts {
                    match part.parse::<u64>() {
                        Ok(part) => {
                            seconds += multiplier * part as f64;
                            multiplier *= 60.0;
                        }
                        Err(_) => {
                            return Err(ParseError {
                                kind: ParseErrorKind::TokenParse(token, None),
                                expected: "duration".into(),
                            })
                        }
                    }
                }
                Ok((Duration::from_secs_f64(seconds), input))
            }
            None => Err(ParseError {
                kind: ParseErrorKind::TokenRequired,
                expected: "duration".into(),
            }),
        }
    }
}

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

pub fn has_tokens(input: &str) -> bool {
    !input.is_empty() && !input.starts_with(')')
}

pub fn take_token(mut input: &str) -> (Option<Cow<'_, str>>, &str) {
    if input.starts_with(')') {
        return (None, input);
    }

    let token_start = input;
    if input.starts_with('"') || input.starts_with('\'') {
        let mut result = String::new();
        let mut chars = input.chars();
        let quote_ch = chars.next().unwrap();
        let mut escaped = false;
        for ch in &mut chars {
            if escaped {
                result.push(ch);
                escaped = false;
            } else {
                match ch {
                    ch if ch == quote_ch => break,
                    '\\' => escaped = true,
                    ch => result.push(ch),
                }
            }
        }
        (Some(result.into()), skip_ws(chars.as_str()))
    } else {
        loop {
            let mut chars = input.chars();
            match chars.next() {
                Some(ch) if !ch.is_whitespace() && ch != '"' && ch != '\'' && ch != ')' => {
                    input = chars.as_str()
                }
                _ => break,
            }
        }
        let token = &token_start[..(token_start.len() - input.len())];
        if !token.is_empty() {
            (Some(Cow::Borrowed(token)), skip_ws(input))
        } else {
            (None, skip_ws(input))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{take_token, CmdParsable};

    mod numbers {
        use super::*;

        #[test]
        fn parse_u8() {
            assert_eq!(u8::parse_cmd("15 ").unwrap(), (15, ""));
        }

        #[test]
        fn parse_f32() {
            assert_eq!(f32::parse_cmd("14.0 ").unwrap(), (14.0, ""));
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

    mod boolean {
        use super::*;

        #[test]
        fn success() {
            assert_eq!(bool::parse_cmd("true 1").unwrap(), (true, "1"));
            assert_eq!(bool::parse_cmd("t 1").unwrap(), (true, "1"));
            assert_eq!(bool::parse_cmd("yes 1").unwrap(), (true, "1"));
            assert_eq!(bool::parse_cmd("y 1").unwrap(), (true, "1"));
            assert_eq!(bool::parse_cmd("false 1").unwrap(), (false, "1"));
            assert_eq!(bool::parse_cmd("f 1").unwrap(), (false, "1"));
            assert_eq!(bool::parse_cmd("no 1").unwrap(), (false, "1"));
            assert_eq!(bool::parse_cmd("n 1").unwrap(), (false, "1"));
        }

        #[test]
        fn unknown_variant() {
            assert_eq!(
                &bool::parse_cmd("unknown").unwrap_err().to_string(),
                "invalid boolean \"unknown\""
            );
        }

        #[test]
        fn missing() {
            assert_eq!(
                &bool::parse_cmd("").unwrap_err().to_string(),
                "expected boolean"
            );
        }
    }

    mod string {
        use super::*;

        #[test]
        fn parse_string() {
            assert_eq!(
                String::parse_cmd("abc def").unwrap(),
                ("abc".to_string(), "def")
            )
        }

        #[test]
        fn missing_string() {
            assert_eq!(
                &String::parse_cmd("").unwrap_err().to_string(),
                "expected string"
            )
        }

        #[test]
        fn unexpected_token() {
            assert_eq!(
                &String::parse_cmd("(first second)").unwrap_err().to_string(),
                "unexpected token: \"second\""
            )
        }
    }

    mod take_token_tests {
        use super::*;
        use std::borrow::Cow;

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
            assert_eq!(take_token("abcdef"), (Some(Cow::Borrowed("abcdef")), ""));
        }

        #[test]
        fn takes_entire_string_with_whitespaces() {
            assert_eq!(take_token("abcdef  "), (Some(Cow::Borrowed("abcdef")), ""));
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

        #[test]
        fn empry_quoted_string() {
            assert_eq!(take_token("''  a"), (Some(Cow::Owned(String::new())), "a"));
            assert_eq!(
                take_token("\"\"  a"),
                (Some(Cow::Owned(String::new())), "a")
            );
        }

        #[test]
        fn non_empty_quoted_string() {
            assert_eq!(
                take_token("'abc \"def'  a"),
                (Some(Cow::Owned("abc \"def".to_string())), "a")
            );
            assert_eq!(
                take_token("\"abc 'def\"  a"),
                (Some(Cow::Owned("abc 'def".to_string())), "a")
            );
        }

        #[test]
        fn string_with_escape_sequence() {
            assert_eq!(
                take_token(r#"'"\'\\\a'  a"#),
                (Some(Cow::Owned(r#""'\a"#.to_string())), "a")
            );
            assert_eq!(
                take_token(r#""\"'\\\a"  a"#),
                (Some(Cow::Owned(r#""'\a"#.to_string())), "a")
            );
        }

        #[test]
        fn token_followed_by_string() {
            assert_eq!(
                take_token("abc\"def\""),
                (Some(Cow::Borrowed("abc")), "\"def\"")
            );
            assert_eq!(
                take_token("abc'def'"),
                (Some(Cow::Borrowed("abc")), "'def'")
            );
        }
    }

    mod parse_vec {
        use super::*;

        #[test]
        fn parse_vec() {
            let (vector, remaining) = Vec::<u8>::parse_cmd("10 20 30 40 50").unwrap();
            assert_eq!(vector, vec![10, 20, 30, 40, 50]);
            assert!(remaining.is_empty());
        }

        #[test]
        fn parse_vec_empty() {
            let (vector, remaining) = Vec::<u8>::parse_cmd("").unwrap();
            assert_eq!(vector, vec![]);
            assert!(remaining.is_empty());
        }

        #[test]
        fn empty_parenthesis() {
            let (vector, remaining) = Vec::<u8>::parse_cmd("() 10 20").unwrap();
            assert_eq!(vector, vec![]);
            assert_eq!(remaining, "10 20");
        }

        #[test]
        fn stops_at_parenthesis() {
            let (vector, remaining) = Vec::<u8>::parse_cmd("(10 20) 30 40").unwrap();
            assert_eq!(vector, vec![10, 20]);
            assert_eq!(remaining, "30 40");
        }
    }

    mod parse_tuples {
        use super::*;

        #[test]
        fn parse_tuples() {
            let (result, remaining) =
                <(u8, u8, (u8, u8), (u8,))>::parse_cmd("10 20 30 40 50 60").unwrap();
            assert_eq!(result, (10, 20, (30, 40), (50,)));
            assert_eq!(remaining, "60");
        }

        #[test]
        fn too_many_values() {
            assert_eq!(
                &<(u8, u8)>::parse_cmd("(10 20 30)").unwrap_err().to_string(),
                "unexpected token: \"30\""
            );
        }

        #[test]
        fn too_few_values() {
            assert_eq!(
                &<(u8, u8)>::parse_cmd("(10)").unwrap_err().to_string(),
                "expected integer"
            );
        }
    }

    mod parse_duration {
        use std::time::Duration;

        use super::*;

        #[test]
        fn parse_seconds() {
            let (result, _) = Duration::parse_cmd("10").unwrap();
            assert_eq!(result, Duration::from_secs(10))
        }

        #[test]
        fn parse_seconds_decimal() {
            let (result, _) = Duration::parse_cmd("10.4").unwrap();
            assert_eq!(result, Duration::from_secs_f64(10.4))
        }

        #[test]
        fn parse_munites_seconds() {
            let (result, _) = Duration::parse_cmd("14:10").unwrap();
            assert_eq!(result, Duration::from_secs(14 * 60 + 10))
        }

        #[test]
        fn parse_hours_munites_seconds() {
            let (result, _) = Duration::parse_cmd("2:14:10.4").unwrap();
            assert_eq!(
                result,
                Duration::from_secs_f64(2.0 * 3600.0 + 14.0 * 60.0 + 10.4)
            )
        }

        #[test]
        fn too_many_parts() {
            Duration::parse_cmd("4:2:14:10").unwrap_err();
        }

        #[test]
        fn invalid_symbol() {
            Duration::parse_cmd("1s4:10").unwrap_err();
        }
    }
}
