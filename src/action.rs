use crate::event::Event;
use std::fmt::Debug;
use std::rc::Rc as Shared;

/// Action is the trait for callbacks.
pub trait Action<I>: Debug + Clone {
    type Err: std::error::Error;
    fn call(&self, e: &Event<I>) -> Result<(), Self::Err>;
}

type WrapFn<'a, I, E> = Shared<dyn Fn(&Event<I>) -> Result<(), E> + 'a>;

/// Closure is a wrapper around a closure that implements the Action trait.
pub struct Closure<'a, I, E>(pub(crate) WrapFn<'a, I, E>);

impl<'a, I, E> Closure<'a, I, E> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Event<I>) -> Result<(), E> + 'a,
    {
        Self(Shared::new(f))
    }
}

impl<'a, I, E: std::error::Error> Action<I> for Closure<'a, I, E> {
    type Err = E;
    fn call(&self, e: &Event<I>) -> Result<(), Self::Err> {
        (self.0)(e)
    }
}

impl<'a, I, E> Debug for Closure<'a, I, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<Closure<'a, I, E>(Box<dyn Fn(&Event<I>) -> Result<(), E> + 'a>)>"
        )
    }
}

impl<'a, I, E> Clone for Closure<'a, I, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::{event::Event, Action};

    use super::Closure;
    use std::rc::Rc;
    use thiserror::Error;

    #[derive(Debug, Clone, Error)]
    enum MyError {
        #[error("my error: {0}")]
        CustomeError(&'static str),
    }

    #[test]
    fn test_clone() {
        let cb = Closure(Rc::new(|_e| -> Result<(), MyError> {
            Err(MyError::CustomeError("test"))
        }));
        let e = Event {
            event: "",
            src: "",
            dst: "",
            args: None::<&Vec<u32>>,
        };
        assert_eq!(
            cb.call(&e).err().unwrap().to_string(),
            cb.clone().call(&e).err().unwrap().to_string()
        );
    }
}
