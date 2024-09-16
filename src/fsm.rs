use crate::{errors::FSMError, event::Event};
use std::{collections::HashMap, hash::Hash};

type BoxClosure<'a, K, V, E> = Box<dyn Fn(&Event<K, V>) -> Result<(), E> + 'a>;
pub struct Action<'a, K, V, E>(BoxClosure<'a, K, V, E>);

impl<'a, K, V, E> Action<'a, K, V, E> {
    pub fn call(&self, e: &Event<K, V>) -> Result<(), E> {
        (self.0)(e)
    }
}

pub trait StateTag {
    fn name(&self) -> String;
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Hook<S: AsRef<str>> {
    Before(S),
    After(S),
    Custom(S),

    BeforeEvent,
    AfterEvent,
    LeaveState,
    EnterState,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CallbackType {
    None,
    BeforeEvent,
    LeaveState,
    EnterState,
    AfterEvent,
}

// EventDesc represents an event when initializing the FSM.
//
// The event can have one or more source states that is valid for performing
// the transition. If the FSM is in one of the source states it will end up in
// the specified destination state, calling all defined callbacks as it goes.
pub struct EventDesc<S>
where
    S: AsRef<str> + Clone + PartialEq + Eq + Hash,
{
    // Name is the event name used when calling for a transition.
    pub name: S,

    // Src is a slice of source states that the FSM must be in to perform a
    // state transition.
    pub src: Vec<S>,

    // Dst is the destination state that the FSM will be in if the transition
    // succeeds.
    pub dst: S,
}

// EKey is a struct key used for storing the transition map.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EKey {
    // event is the name of the event that the keys refers to.
    event: String,

    // src is the source from where the event can transition.
    src: String,
}

// CKey is a struct key used for keeping the callbacks mapped to a target.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CKey {
    // target is either the name of a state or an event depending on which
    // callback type the key refers to. It can also be "" for a non-targeted
    // callback like before_event.
    target: String,

    // callback_type is the situation when the callback will be run.
    callback_type: CallbackType,
}

pub struct FSM<'a, K, V, E> {
    // current is the state that the FSM is currently in.
    current: String,

    // transitions maps events and source states to destination states.
    transitions: HashMap<EKey, String>,

    // callbacks maps events and targets to callback functions.
    callbacks: HashMap<CKey, Action<'a, K, V, E>>,
}

impl<'a, K, V, E> FSM<'a, K, V, E>
where
    E: std::error::Error,
{
    pub fn new<S>(
        initial: impl StateTag,
        events: Vec<EventDesc<S>>,
        callback_iter: HashMap<Hook<S>, Action<'a, K, V, E>>,
    ) -> Self
    where
        S: AsRef<str> + Clone + PartialEq + Eq + Hash,
    {
        let mut all_events = HashMap::new();
        let mut all_states = HashMap::new();
        let mut transitions = HashMap::new();

        for e in events {
            all_events.insert(e.name.clone(), true);
            for src in e.src {
                transitions.insert(
                    EKey {
                        event: e.name.as_ref().to_string(),
                        src: src.as_ref().to_string(),
                    },
                    e.dst.as_ref().to_string(),
                );
                all_states.insert(src.clone(), true);
                all_states.insert(e.dst.clone(), true);
            }
        }

        let mut callbacks: HashMap<CKey, Action<'a, K, V, E>> = HashMap::new();
        for (name, callback) in callback_iter {
            let (target, callback_type) = match name {
                Hook::Before(t) => (t.as_ref().to_string(), CallbackType::BeforeEvent),
                Hook::After(t) => (t.as_ref().to_string(), CallbackType::AfterEvent),
                Hook::BeforeEvent => ("".to_string(), CallbackType::BeforeEvent),
                Hook::AfterEvent => ("".to_string(), CallbackType::AfterEvent),
                Hook::LeaveState => ("".to_string(), CallbackType::LeaveState),
                Hook::EnterState => ("".to_string(), CallbackType::EnterState),
                Hook::Custom(t) => {
                    let callback_type = if all_states.contains_key(&t) {
                        CallbackType::EnterState
                    } else if all_events.contains_key(&t) {
                        CallbackType::AfterEvent
                    } else {
                        CallbackType::None
                    };
                    (t.as_ref().to_string(), callback_type)
                }
            };

            if callback_type != CallbackType::None {
                callbacks.insert(
                    CKey {
                        target,
                        callback_type,
                    },
                    callback,
                );
            }
        }
        Self {
            current: initial.name(),
            callbacks,
            transitions,
        }
    }

    pub fn get_current(&self) -> &str {
        self.current.as_str()
    }

    pub fn on_event(
        &mut self,
        event: &str,
        args: Option<&HashMap<K, V>>,
    ) -> Result<(), FSMError<String>> {
        let dst = self
            .transitions
            .get(&EKey {
                event: event.to_string(),
                src: self.current.to_string(),
            })
            .ok_or_else(|| {
                for ekey in self.transitions.keys() {
                    if ekey.event.eq(&event) {
                        return FSMError::InvalidEvent(event.to_string(), self.current.to_string());
                    }
                }
                FSMError::UnknownEvent(event.to_string())
            })?;

        let src = &self.current.clone();
        let e = Event {
            event,
            src,
            dst,
            args,
        };

        self.before_event_callbacks(&e)
            .map_err(|err| FSMError::InternalError(err.to_string()))?;

        if self.current.eq(dst) {
            self.after_event_callbacks(&e)
                .map_err(|err| FSMError::InternalError(err.to_string()))?;
        }

        self.leave_state_callbacks(&e)
            .map_err(|err| FSMError::InternalError(err.to_string()))?;
        self.current = dst.to_string();
        self.enter_state_callbacks(&e)
            .map_err(|err| FSMError::InternalError(err.to_string()))?;
        self.after_event_callbacks(&e)
            .map_err(|err| FSMError::InternalError(err.to_string()))?;

        Ok(())
    }
}

impl<'a, K, V, E> FSM<'a, K, V, E>
where
    E: std::error::Error,
{
    fn before_event_callbacks(&self, e: &Event<K, V>) -> Result<(), E> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: e.event.to_string(),
            callback_type: CallbackType::BeforeEvent,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: "".to_string(),
            callback_type: CallbackType::BeforeEvent,
        }) {
            f.call(e)?;
        }
        Ok(())
    }

    fn after_event_callbacks(&self, e: &Event<K, V>) -> Result<(), E> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: e.event.to_string(),
            callback_type: CallbackType::AfterEvent,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: "".to_string(),
            callback_type: CallbackType::AfterEvent,
        }) {
            f.call(e)?;
        }
        Ok(())
    }

    fn enter_state_callbacks(&self, e: &Event<K, V>) -> Result<(), E> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: e.event.to_string(),
            callback_type: CallbackType::EnterState,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: "".to_string(),
            callback_type: CallbackType::EnterState,
        }) {
            f.call(e)?;
        }
        Ok(())
    }

    fn leave_state_callbacks(&self, e: &Event<K, V>) -> Result<(), E> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: e.event.to_string(),
            callback_type: CallbackType::LeaveState,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: "".to_string(),
            callback_type: CallbackType::LeaveState,
        }) {
            f.call(e)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, EventDesc, Hook, StateTag, FSM};
    use crate::event::Event;
    use std::collections::HashMap;
    use thiserror::Error;

    #[derive(Debug, Error)]
    enum MyError {
        // #[error("test error")]
        // CustomeError,
    }

    #[test]
    fn it_works() {
        let callbacks = HashMap::from([
            (
                Hook::BeforeEvent,
                Action(Box::new(|e: &Event<u32, u32>| -> Result<(), MyError> {
                    println!("before event, {}, from:{}, {:?}", e.event, e.src, e.args);
                    Ok(())
                })),
            ),
            (
                Hook::AfterEvent,
                Action(Box::new(|e: &Event<u32, u32>| -> Result<(), MyError> {
                    println!("after event, {}, from:{}, {:?}", e.event, e.src, e.args);
                    Ok(())
                })),
            ),
            (
                Hook::EnterState,
                Action(Box::new(|e: &Event<u32, u32>| -> Result<(), MyError> {
                    println!("enter state, {}, from:{}, {:?}", e.event, e.src, e.args);
                    Ok(())
                })),
            ),
            (
                Hook::LeaveState,
                Action(Box::new(|e: &Event<u32, u32>| -> Result<(), MyError> {
                    println!("leave state, {}, from:{}, {:?}", e.event, e.src, e.args);
                    Ok(())
                })),
            ),
            (
                Hook::Before("open"),
                Action(Box::new(|e: &Event<u32, u32>| -> Result<(), MyError> {
                    println!(
                        "before open event, {}, from:{}, {:?}",
                        e.event, e.src, e.args
                    );
                    Ok(())
                })),
            ),
            (
                Hook::After("open"),
                Action(Box::new(|e: &Event<u32, u32>| -> Result<(), MyError> {
                    println!(
                        "after open event, {}, from:{}, {:?}",
                        e.event, e.src, e.args
                    );
                    Ok(())
                })),
            ),
            (
                Hook::Custom("open"),
                Action(Box::new(|e: &Event<u32, u32>| -> Result<(), MyError> {
                    println!(
                        "custom open event, {}, from:{}, {:?}",
                        e.event, e.src, e.args
                    );
                    Ok(())
                })),
            ),
        ]);

        enum State {
            #[allow(dead_code)]
            Open,
            Closed,
        }

        impl StateTag for State {
            fn name(&self) -> String {
                match self {
                    State::Open => "open".to_string(),
                    State::Closed => "closed".to_string(),
                }
            }
        }

        let mut fsm = FSM::new(
            State::Closed,
            vec![
                EventDesc {
                    name: "open",
                    src: vec!["closed"],
                    dst: "open",
                },
                EventDesc {
                    name: "close",
                    src: vec!["open"],
                    dst: "closed",
                },
            ],
            callbacks,
        );

        assert_eq!("closed", fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event("open", Some(&hashmap));
        assert_eq!("open", fsm.get_current());
        let _ = fsm.on_event("close", None);
        assert_eq!("closed", fsm.get_current());
        let _ = fsm;
    }

    fn show<S: AsRef<str>>(s: S) {
        println!("{}", s.as_ref());
    }

    #[test]
    fn test_asref() {
        show("hello");
        show("hello".to_string());
    }
}
