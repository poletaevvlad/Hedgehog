use cmdparse::error::UnrecognizedToken;
use cmdparse::tokens::{Token, TokenStream, UnbalancedParenthesis};
use cmdparse::{CompletionResult, ParseResult, Parser};

#[derive(Default)]
pub struct SearchQueryParser;

fn extend_space_separated(input: &mut String, text: &str) {
    if !input.is_empty() {
        input.push(' ');
    }
    input.push_str(text);
}

fn handle_nested(mut input: TokenStream) -> ParseResult<String> {
    let mut result = String::new();
    loop {
        match input.take() {
            Some(Ok((token, remaining))) => {
                let text = token.into_raw_lexeme().parse_string();
                extend_space_separated(&mut result, &text);
                input = remaining;
            }
            Some(Err(UnbalancedParenthesis)) => {
                let (item, remaining) = input
                    .with_nested(handle_nested)
                    .expect("handle_nested cannot fail");
                extend_space_separated(&mut result, &item);
                input = remaining;
            }
            None => break,
        }
    }
    Ok((result, input))
}

impl<Ctx> Parser<Ctx> for SearchQueryParser {
    type Value = String;

    fn parse<'a>(&self, mut input: TokenStream<'a>, _ctx: Ctx) -> ParseResult<'a, Self::Value> {
        let mut result = String::new();
        let mut is_first = false;

        loop {
            match input.take() {
                None => break,
                Some(Ok((Token::Text(text), remaining))) => {
                    input = remaining;
                    let text = text.parse_string();
                    extend_space_separated(&mut result, &text);
                }
                Some(Ok((token @ Token::Attribute(_), remaining))) if is_first => {
                    return Err(UnrecognizedToken::new(token, remaining).into())
                }
                Some(Ok((Token::Attribute(_), remaining))) => {
                    input = remaining;
                    break;
                }
                Some(Err(UnbalancedParenthesis)) => {
                    let (nested, remaining) = input
                        .with_nested(handle_nested)
                        .expect("handle_nested cannot fail");
                    extend_space_separated(&mut result, &nested);
                    input = remaining;
                }
            }
            is_first = true;
        }

        Ok((result, input))
    }

    fn complete<'a>(&self, input: TokenStream<'a>, _ctx: Ctx) -> CompletionResult<'a> {
        let consumed = match input.take() {
            Some(Ok((Token::Text(_), _))) => true,
            Some(Ok((Token::Attribute(_), _))) => false,
            Some(Err(_)) => true,
            None => false,
        };
        CompletionResult::new_final(consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::SearchQueryParser;
    use cmdparse::parse_parser;

    #[test]
    fn parses_nested() {
        assert_eq!(
            &parse_parser::<(), SearchQueryParser>("abc def (ghi jkl \"mno\" 'pqr' (stu))", ())
                .unwrap(),
            "abc def ghi jkl mno pqr stu"
        );
    }
}
