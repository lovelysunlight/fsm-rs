use std::fmt::Display;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FSMError<S: Display> {
    #[error("internal error: {0}")]
    InternalError(S),

    #[error("event {0} does not exist")]
    UnknownEvent(S),

    #[error("event {0} inappropriate in current state {1}")]
    InvalidEvent(S, S),
}
