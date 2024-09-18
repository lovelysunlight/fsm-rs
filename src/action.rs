use crate::event::Event;
use std::fmt::Debug;
use std::rc::Rc as Shared;

/// Action is the trait for callbacks.
pub trait Action<S, I>: Debug {
    type Err: std::error::Error;
    fn call(&self, e: &Event<S, I>) -> Result<(), Self::Err>;
}

type WrapFn<'a, S, I, E> = Shared<dyn Fn(&Event<S, I>) -> Result<(), E> + 'a>;

/// Closure is a wrapper around a closure that implements the Action trait.
pub struct Closure<'a, S, I, E>(pub(crate) WrapFn<'a, S, I, E>);

impl<'a, S, I, E> Closure<'a, S, I, E> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&Event<S, I>) -> Result<(), E> + 'a,
    {
        Self(Shared::new(f))
    }
}

impl<'a, S, I, E: std::error::Error> Action<S, I> for Closure<'a, S, I, E> {
    type Err = E;
    fn call(&self, e: &Event<S, I>) -> Result<(), Self::Err> {
        (self.0)(e)
    }
}

impl<'a, S, I, E> Debug for Closure<'a, S, I, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<Closure<'a, I, E>(Box<dyn Fn(&Event<I>) -> Result<(), E> + 'a>)>"
        )
    }
}

impl<'a, S, I, E> Clone for Closure<'a, S, I, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
