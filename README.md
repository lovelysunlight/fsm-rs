# FSM for Rust

[![Build Status](https://github.com/lovelysunlight/fsm-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/lovelysunlight/fsm-rs/actions/workflows/ci.yml)
[![License Badge](https://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/lovelysunlight/fsm-rs/main/LICENSE)

Finite State Machine for Rust.

## Installing

```toml
[dependencies]
scc = "0.1"

# optional, you can also use `strum` to work with enums and strings easier in Rust.
# strum = { version = "0.26", features = ["derive"] }
```

## Usage

From examples/basic:
```rust
use scc::{Closure, EventDesc, FSMEvent, FSMState, HookType, FSM};
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
```

## Contributing

You can contribute in one of three ways:

1. File bug reports using the [issue tracker](https://github.com/lovelysunlight/fsm-rs/issues).
2. Answer questions or fix bugs on the [issue tracker](https://github.com/lovelysunlight/fsm-rs/issues).
3. Contribute new features or update the wiki.

## License

MIT
