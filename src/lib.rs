mod action;
mod error;
mod event;
mod fsm;

pub use action::{Action, Closure};
pub use error::FSMError;
pub use {fsm::CallbackType, fsm::EnumType, fsm::EventDesc, fsm::HookType, fsm::FSM};

#[cfg(test)]
mod tests {
    use strum::Display;

    #[derive(Debug, PartialEq, Display)]
    enum TestTag {
        #[strum(serialize = "Opened")]
        Opened,
        #[strum(serialize = "Closed")]
        Closed,
    }

    #[test]
    fn test_enum_display() {
        assert_eq!("Opened", TestTag::Opened.to_string());
        assert_eq!("Closed", TestTag::Closed.to_string());
    }
}
