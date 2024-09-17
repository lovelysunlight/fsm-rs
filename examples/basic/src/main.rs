use strum::{Display, EnumString};
use fsm_rs::{EnumType, EventDesc, FSM};
use std::collections::HashMap;

fn main() {
    let mut fsm: FSM<u32, u32, MyError> = FSM::new(
        StateTag::Closed,
        vec![
            EventDesc {
                name: EventTag::Open,
                src: vec![StateTag::Closed],
                dst: StateTag::Opened,
            },
            EventDesc {
                name: EventTag::Close,
                src: vec![StateTag::Opened],
                dst: StateTag::Closed,
            },
        ],
        HashMap::new(),
    );
    dbg!("{}", fsm.get_current());

    assert!(fsm.on_event("open", None).is_ok());
    dbg!("{}", fsm.get_current());

    assert!(fsm.on_event("close", None).is_ok());
    dbg!("{}", fsm.get_current());

    let ret = fsm.on_event("close", None);
    assert!(ret.is_err());
    dbg!("{:?}", ret.err().unwrap());
    dbg!("{}", fsm.get_current());
}

#[derive(Display, EnumString, Debug, Clone, Hash, PartialEq, Eq)]
enum StateTag {
    #[strum(serialize = "opened")]
    Opened,
    #[strum(serialize = "closed")]
    Closed,
}
impl EnumType for StateTag {}

#[derive(Display, EnumString, Debug, Clone, Hash, PartialEq, Eq)]
enum EventTag {
    #[strum(serialize = "open")]
    Open,
    #[strum(serialize = "close")]
    Close,
}
impl EnumType for EventTag {}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum MyError {
    Unknown,
}

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        // We could use our macro here, but this way we don't take a dependency on the
        // macros crate.
        match self {
            MyError::Unknown => write!(f, "unknown error"),
        }
    }
}

impl std::error::Error for MyError {
    fn description(&self) -> &str {
        match self {
            MyError::Unknown => "unknown error.",
        }
    }
}
