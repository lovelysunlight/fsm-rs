#[doc(hidden)]
use fsm::{Closure, EventDesc, FSMEvent, FSMState, HookType, FSM};
use std::collections::HashMap;
use strum::AsRefStr;
use strum::Display;

fn main() {
    let mut fsm: FSM<Vec<u32>, _> = FSM::new(
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
        HashMap::from([
            (
                HookType::BeforeEvent,
                Closure::new(|_e| -> Result<(), MyError> { Ok(()) }),
            ),
            (
                HookType::AfterEvent,
                Closure::new(|_e| -> Result<(), MyError> { Ok(()) }),
            ),
        ]),
    );
    println!("{}", fsm.get_current());

    assert!(fsm.on_event(EventTag::Open, None).is_ok());
    println!("{}", fsm.get_current());

    assert!(fsm.on_event(EventTag::Close, None).is_ok());
    println!("{}", fsm.get_current());

    {
        let ret = fsm.on_event(EventTag::Close, None);
        assert!(ret.is_err());
        println!("{:?}", ret.err().unwrap());
        println!("{}", fsm.get_current());
    }
}

#[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
enum StateTag {
    #[strum(serialize = "opened")]
    Opened,
    #[strum(serialize = "closed")]
    Closed,
}
impl FSMState for StateTag {}

#[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
enum EventTag {
    #[strum(serialize = "open")]
    Open,
    #[strum(serialize = "close")]
    Close,
}
impl FSMEvent for EventTag {}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum MyError {
    Unknown,
}

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
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
