use std::fmt::Display;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FSMError<S: Display> {
    #[error("no transition with error: {0}")]
    NoTransitionWithError(S),

    #[error("no transition")]
    NoTransition,

    #[error("internal error: {0}")]
    InternalError(S),

    #[error("event {0} does not exist")]
    UnknownEvent(S),

    #[error("event {0} inappropriate in current state {1}")]
    InvalidEvent(S, S),
}
