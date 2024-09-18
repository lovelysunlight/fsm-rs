# FSM for Rust

[![Build Status](https://github.com/lovelysunlight/fsm-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/lovelysunlight/fsm-rs/actions/workflows/ci.yml)
[![Latest Version](https://img.shields.io/crates/v/small-fsm.svg)](https://crates.io/crates/small-fsm)
[![Rust Documentation](https://docs.rs/small-fsm/badge.svg)](https://docs.rs/small-fsm)
[![License Badge](https://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/lovelysunlight/fsm-rs/main/LICENSE)

Finite State Machine for Rust.

## Installing

```toml
[dependencies]
small-fsm = "0.1"

# optional, you can also use `strum` to work with enums and strings easier in Rust.
# strum = { version = "0.26", features = ["derive"] }
```

## Usage

From examples/basic:
```rust
use small_fsm::{Closure, EventDesc, FSMState, HookType, FSM};
use std::collections::HashMap;
use strum::AsRefStr;
use strum::Display;

#[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
enum StateTag {
    #[strum(serialize = "opened")]
    Opened,
    #[strum(serialize = "closed")]
    Closed,
}
impl FSMState for StateTag {}
impl AsRef<Self> for StateTag {
    fn as_ref(&self) -> &Self {
        &self
    }
}

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

fn main() {
    let mut fsm: FSM<_, Vec<u32>, _> = FSM::new(
        StateTag::Closed,
        vec![
            EventDesc {
                name: "open",
                src: vec![StateTag::Closed],
                dst: StateTag::Opened,
            },
            EventDesc {
                name: "close",
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

    assert!(fsm.on_event("open", None).is_ok());
    println!("{}", fsm.get_current());

    assert!(fsm.on_event("close", None).is_ok());
    println!("{}", fsm.get_current());

    {
        let ret = fsm.on_event("close", None);
        assert!(ret.is_err());
        println!("{:?}", ret.err().unwrap());
        println!("{}", fsm.get_current());
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
