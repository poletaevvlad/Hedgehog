macro_rules! gen_options {
    ($($(#$attr:tt)* $command:ident($(#$arg_attr:tt)* $name:ident: $value:ty = $default:expr)),*$(,)?) => {
        pub(crate) struct Options {
            $(pub(crate) $name: $value),*
        }

        impl Default for Options {
            fn default() -> Self {
                Options {
                    $($name: $default),*
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, cmdparse::Parsable)]
        pub(crate) enum OptionsUpdate {
            $($(#$attr)* $command($(#$arg_attr)* $value)),*
        }

        impl Options {
            pub(crate) fn update(&mut self, update: OptionsUpdate) {
                match update {
                    $(OptionsUpdate::$command(value) => self.$name = value),*
                }
            }
        }
    };
}

gen_options! {
    DateFormat(date_format: String = "%x".to_string()),
    LabelPlaybackStatePlaying(label_playback_status_playing: String = " > ".to_string()),
    LabelPlaybackStatePaused(label_playback_status_paused: String = " | ".to_string()),
    LabelPlaybackStateBuffering(label_playback_status_buffering: String = " o ".to_string()),
    LabelPlaybackStateNone(label_playback_status_none: String = " - ".to_string()),
    LabelEpisodeNew(label_episode_new: String = " new ".to_string()),
    LabelEpisodeSeen(label_episode_seen: String = "".to_string()),
    LabelEpisodePlaying(label_episode_playing: String = " playing ".to_string()),
    LabelEpisodeStarted(label_episode_started: String = " started ".to_string()),
    LabelEpisodeFinished(label_episode_finished: String = " finished ".to_string()),
    LabelEpisodeError(label_episode_error: String = " error ".to_string()),
    LabelFeedError(label_feed_error: String = "E".to_string()),
    FeedUpdatingChars(
        #[cmd(parser = "cmdparse::parsers::TransformParser<cmdparse::parsers::StringParser, CharVecTransformation, Vec<char>>")]
        feed_updating_chars: Vec<char> = vec!['⠇', '⠋', '⠙', '⠸', '⢰', '⣠', '⣄', '⡆']
    ),
    AnimationTickDuration(animation_tick_duration: u64 = 150),
    UpdateOnStart(update_on_start: bool = true),
    ShowEpisodeNumber(show_episode_number: bool = true),
    Hidden(hidden: bool = false),
    ProgressBarWidth(progress_bar_width: u16 = 32),
    ProgressBarChars(
        #[cmd(parser = "cmdparse::parsers::TransformParser<cmdparse::parsers::StringParser, CharVecTransformation, Vec<char>>")]
        progress_bar_chars: Vec<char> = vec![' ', '⠁', '⠃', '⠇', '⡇', '⡏', '⡟', '⡿', '⣿']
    ),
}

impl OptionsUpdate {
    pub(crate) fn affects_episodes_list(&self) -> bool {
        matches!(self, OptionsUpdate::Hidden(_))
    }
}

struct CharVecTransformation;

impl cmdparse::parsers::ParsableTransformation<Vec<char>> for CharVecTransformation {
    type Input = String;

    fn transform(input: Self::Input) -> Result<Vec<char>, cmdparse::error::ParseError<'static>> {
        Ok(input.chars().collect())
    }
}
