//! # FSM for Rust
//!
//! [![Build Status](https://github.com/lovelysunlight/fsm-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/lovelysunlight/fsm-rs/actions/workflows/ci.yml)
//! [![Latest Version](https://img.shields.io/crates/v/small-fsm.svg)](https://crates.io/crates/small-fsm)
//! [![Rust Documentation](https://docs.rs/small-fsm/badge.svg)](https://docs.rs/small-fsm)
//! [![License Badge](https://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/lovelysunlight/fsm-rs/main/LICENSE)
//!
//! Finite State Machine for Rust.
//!
//! The full version of the README can be found on [GitHub](https://github.com/lovelysunlight/fsm-rs).
//!
//! # Including Fsm in Your Project
//!
//! ```toml
//! [dependencies]
//! small-fsm = "0.1"
//!
//! # optional, you can also use `strum` to work with enums and strings easier in Rust.
//! # strum = { version = "0.26", features = ["derive"] }
//! ```
//!
//! # Example
//!
//! ```rust
//! use small_fsm::{Closure, EventDesc, FSMEvent, FSMState, HookType, FSM};
//! use std::collections::HashMap;
//! use strum::AsRefStr;
//! use strum::Display;
//!
//! #[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
//! enum StateTag {
//!     #[strum(serialize = "opened")]
//!     Opened,
//!     #[strum(serialize = "closed")]
//!     Closed,
//! }
//! impl FSMState for StateTag {}
//!
//! #[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
//! enum EventTag {
//!     #[strum(serialize = "open")]
//!     Open,
//!     #[strum(serialize = "close")]
//!     Close,
//! }
//! impl FSMEvent for EventTag {}
//!
//! #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
//! pub enum MyError {
//!     Unknown,
//! }
//!
//! impl std::fmt::Display for MyError {
//!     fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
//!         match self {
//!             MyError::Unknown => write!(f, "unknown error"),
//!         }
//!     }
//! }
//!
//! impl std::error::Error for MyError {
//!     fn description(&self) -> &str {
//!         match self {
//!             MyError::Unknown => "unknown error.",
//!         }
//!     }
//! }
//!
//! let mut fsm: FSM<Vec<u32>, _> = FSM::new(
//!     StateTag::Closed,
//!     vec![
//!         EventDesc {
//!             name: EventTag::Open,
//!             src: vec![StateTag::Closed],
//!             dst: StateTag::Opened,
//!         },
//!         EventDesc {
//!             name: EventTag::Close,
//!             src: vec![StateTag::Opened],
//!             dst: StateTag::Closed,
//!         },
//!     ],
//!     HashMap::from([
//!         (
//!             HookType::BeforeEvent,
//!             Closure::new(|_e| -> Result<(), MyError> { Ok(()) }),
//!         ),
//!         (
//!             HookType::AfterEvent,
//!             Closure::new(|_e| -> Result<(), MyError> { Ok(()) }),
//!         ),
//!     ]),
//! );
//!
//! assert_eq!("closed", fsm.get_current());
//!
//! assert!(fsm.on_event(EventTag::Open, None).is_ok());
//! assert_eq!("opened", fsm.get_current());
//!
//! assert!(fsm.on_event(EventTag::Close, None).is_ok());
//! assert_eq!("closed", fsm.get_current());
//! ```
//!

mod action;
mod error;
mod event;
mod fsm;

pub use self::fsm::{CallbackType, EventDesc, FSMEvent, FSMState, HookType, FSM};
pub use action::{Action, Closure};
pub use error::FSMError;

#[cfg(test)]
mod tests {
    use strum::AsRefStr;
    use strum::Display;

    #[derive(Debug, Display, AsRefStr)]
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

    #[test]
    fn test_enum_as_ref() {
        assert_eq!("Opened", TestTag::Opened.as_ref());
        assert_eq!("Closed", TestTag::Closed.as_ref());
    }
}
