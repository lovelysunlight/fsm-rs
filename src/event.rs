/// Event is the info that get passed as a reference in the callbacks.
pub struct Event<'a, I> {
    /// `event` is the event name.
    pub event: &'a str,

    /// `src` is the state before the transition.
    pub src: &'a str,

    /// `dst` is the state after the transition.
    pub dst: &'a str,

    /// `args` is an optional list of arguments passed to the callback.
    pub args: Option<&'a I>,
}
