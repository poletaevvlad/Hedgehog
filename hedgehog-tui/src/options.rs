use cmd_parser::CmdParsable;
use serde::Deserialize;

macro_rules! gen_options {
    ($($command:ident($name:ident: $value:ty = $default:expr)),*$(,)?) => {
        pub(crate) struct Options {
            pub(crate) $($name: $value),*
        }

        impl Default for Options {
            fn default() -> Self {
                Options {
                    $($name: $default),*
                }
            }
        }

        #[derive(Debug, Deserialize, Clone, PartialEq, CmdParsable)]
        #[serde(rename_all = "kebab-case")]
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
}
