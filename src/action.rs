use crate::event::Event;
use std::fmt::Debug;

pub trait Action<I>: Debug {
    type Err: std::error::Error;
    fn call(&self, e: &Event<I>) -> Result<(), Self::Err>;
}

type BoxFn<'a, I, E> = Box<dyn Fn(&Event<I>) -> Result<(), E> + 'a>;

pub struct Closure<'a, I, E>(pub(crate) BoxFn<'a, I, E>);

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
