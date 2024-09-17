mod error;
mod event;
mod fsm;

pub use error::FSMError;
pub use {fsm::Action, fsm::CallbackType, fsm::EnumType, fsm::EventDesc, fsm::Hook, fsm::FSM};

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use strum::{Display, EnumString};

    #[derive(Debug, PartialEq, EnumString, Display)]
    enum TestTag {
        #[strum(serialize = "Opened")]
        Opened,
        #[strum(serialize = "Closed")]
        Closed,
    }

    #[test]
    fn test_enum_from_str() {
        assert_eq!(TestTag::Opened, TestTag::from_str("Opened").unwrap());
        assert_eq!(TestTag::Closed, TestTag::from_str("Closed").unwrap());
        assert!(TestTag::from_str("Unknown").is_err());
    }

    #[test]
    fn test_enum_display() {
        assert_eq!("Opened", TestTag::Opened.to_string());
        assert_eq!("Closed", TestTag::Closed.to_string());
    }
}
