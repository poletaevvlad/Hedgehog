use cmd_parser::CmdParsable;

macro_rules! gen_options {
    ($($command:ident($name:ident: $value:ty = $default:expr)),*$(,)?) => {
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

        #[derive(Debug, Clone, PartialEq, CmdParsable)]
        pub(crate) enum OptionsUpdate {
            $($command($value)),*
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
    LabelFeedError(label_feed_error: String = " E ".to_string()),
    LabelFeedUpdating(label_feed_updating: String = " U ".to_string()),
}
