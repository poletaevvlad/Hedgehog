#[derive(Debug)]
pub enum ParseError<'a> {
    TokenParse(&'a str, Box<dyn std::error::Error>),
    TokenRequired,
}

pub trait CmdParsable {
    fn parse_cmd(input: &str) -> Result<(Self, &str), ParseError<'_>>
    where
        Self: Sized;
}

impl CmdParsable for u8 {
    fn parse_cmd(input: &str) -> Result<(Self, &str), ParseError<'_>>
    where
        Self: Sized,
    {
        let (token, remaining) = take_token(input);
        match token {
            Some(token) => match token.parse() {
                Ok(num) => Ok((num, remaining)),
                Err(error) => Err(ParseError::TokenParse(token, Box::new(error))),
            },
            None => Err(ParseError::TokenRequired),
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
