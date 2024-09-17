use crate::event::Event;
use std::fmt::Debug;

pub trait Action<K, V>: Debug {
    type Err: std::error::Error;
    fn call(&self, e: &Event<K, V>) -> Result<(), Self::Err>;
}

type BoxFn<'a, K, V, E> = Box<dyn Fn(&Event<K, V>) -> Result<(), E> + 'a>;

pub struct Closure<'a, K, V, E>(pub(crate) BoxFn<'a, K, V, E>);

impl<'a, K, V, E: std::error::Error> Action<K, V> for Closure<'a, K, V, E> {
    type Err = E;
    fn call(&self, e: &Event<K, V>) -> Result<(), Self::Err> {
        (self.0)(e)
    }
}

impl<'a, K, V, E> Debug for Closure<'a, K, V, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<Closure<'a, K, V, E>(Box<dyn Fn(&Event<K, V>) -> Result<(), E> + 'a>)>"
        )
    }
}
