use cmdparse::{tokens::Token, CompletionResult, Parsable, Parser};
use hedgehog_library::model::{FeedSummary, FeedView, GroupSummary};

#[derive(Clone)]
pub(crate) struct CommandContext<'a> {
    pub(crate) feeds: &'a [FeedView<FeedSummary, GroupSummary>],
}

#[derive(Default)]
pub(crate) struct GroupNameParser;

impl<'c> Parser<CommandContext<'c>> for GroupNameParser {
    type Value = String;

    fn parse<'a>(
        &self,
        input: cmdparse::tokens::TokenStream<'a>,
        ctx: CommandContext<'c>,
    ) -> cmdparse::ParseResult<'a, Self::Value> {
        <String as Parsable<CommandContext<'c>>>::Parser::default().parse(input, ctx)
    }

    fn complete<'a>(
        &self,
        input: cmdparse::tokens::TokenStream<'a>,
        ctx: CommandContext<'c>,
    ) -> cmdparse::CompletionResult<'a> {
        match input.take() {
            Some(Ok((Token::Text(text), remaining))) if remaining.is_all_consumed() => {
                let text = text.parse_string();
                CompletionResult::new_final(true).add_suggestions(
                    ctx.feeds
                        .iter()
                        .filter_map(FeedView::as_group)
                        .filter_map(|group| {
                            group.name.strip_prefix(&text as &str).and_then(|key| {
                                match key.is_empty() {
                                    true => None,
                                    false => Some(key.to_string().into()),
                                }
                            })
                        }),
                )
            }
            Some(Ok((Token::Text(_), remaining))) => CompletionResult::new(remaining, true),
            Some(Ok((Token::Attribute(_), _))) => CompletionResult::new(input, false),
            Some(Err(_)) => CompletionResult::new_final(false),
            None => CompletionResult::new_final(false).add_suggestions(
                ctx.feeds
                    .iter()
                    .filter_map(FeedView::as_group)
                    .map(|group| group.name.to_string().into()),
            ),
        }
    }
}
