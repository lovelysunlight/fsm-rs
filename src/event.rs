use std::collections::HashMap;

// Event is the info that get passed as a reference in the callbacks.
pub struct Event<'a, K, V> {
    // Event is the event name.
    pub event: &'a str,

    // Src is the state before the transition.
    pub src: &'a str,

    // Dst is the state after the transition.
    pub dst: &'a str,

    // Args is an optional list of arguments passed to the callback.
    pub args: Option<&'a HashMap<K, V>>,
}
