macro_rules! key {
    ($char:literal $(,$mod:ident)?) => {
        ::crossterm::event::Event::Key(::crossterm::event::KeyEvent {
            code: ::crossterm::event::KeyCode::Char($char),
            modifiers: key!(@internal_mod $($mod)?),
        })
    };
    ($name:ident $(,$mod:ident)?) => {
        ::crossterm::event::Event::Key(::crossterm::event::KeyEvent {
            code: ::crossterm::event::KeyCode::$name,
            modifiers: key!(@internal_mod $($mod)?),
        })
    };
    (F($num:literal) $(,$mod:ident)?) => {
        ::crossterm::event::Event::Key(::crossterm::event::KeyEvent {
            code: ::crossterm::event::KeyCode::F($num),
            modifiers: key!(@internal_mod $($mod)?),
        })
    };

    (@internal_mod) => {::crossterm::event::KeyModifiers::NONE};
    (@internal_mod $mod:ident) => {$crate::events::key_modifiers::$mod};
}

pub(crate) use key;

pub(crate) mod key_modifiers {
    use crossterm::event::KeyModifiers;

    pub(crate) const CONTROL: KeyModifiers = KeyModifiers::CONTROL;
    pub(crate) const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
    pub(crate) const ALT: KeyModifiers = KeyModifiers::ALT;
}
