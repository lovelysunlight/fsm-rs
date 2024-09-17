use crate::{action::Action, error::FSMError, event::Event};
use std::{collections::HashMap, fmt::Display, hash::Hash, str::FromStr};

pub trait EnumType: FromStr + Display + Clone + Hash + PartialEq + Eq {}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Hook<T: EnumType, S: EnumType> {
    Before(T),
    After(T),
    Leave(S),
    Enter(S),
    Custom(&'static str),

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
pub struct EventDesc<T, S>
where
    T: EnumType,
    S: EnumType,
{
    // Name is the event name used when calling for a transition.
    pub name: T,

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

#[derive(Debug)]
pub struct FSM<K, V, F: Action<K, V>> {
    _marker: std::marker::PhantomData<(K, V)>,

    // current is the state that the FSM is currently in.
    current: String,

    // transitions maps events and source states to destination states.
    transitions: HashMap<EKey, String>,

    // callbacks maps events and targets to callback functions.
    callbacks: HashMap<CKey, F>,
}

impl<K, V, F> FSM<K, V, F>
where
    F: Action<K, V>,
{
    pub fn new<T, S>(
        initial: S,
        events: Vec<EventDesc<T, S>>,
        callback_iter: HashMap<Hook<T, S>, F>,
    ) -> Self
    where
        T: EnumType,
        S: EnumType,
    {
        let mut all_events = HashMap::new();
        let mut all_states = HashMap::new();
        let mut transitions = HashMap::new();

        for e in events {
            all_events.insert(e.name.clone(), true);
            for src in e.src {
                transitions.insert(
                    EKey {
                        event: e.name.to_string(),
                        src: src.to_string(),
                    },
                    e.dst.to_string(),
                );
                all_states.insert(src.to_string(), true);
                all_states.insert(e.dst.to_string(), true);
            }
        }

        let mut callbacks: HashMap<CKey, F> = HashMap::new();
        for (name, callback) in callback_iter {
            let (target, callback_type) = match name {
                Hook::BeforeEvent => ("".to_string(), CallbackType::BeforeEvent),
                Hook::AfterEvent => ("".to_string(), CallbackType::AfterEvent),
                Hook::Before(t) => (t.to_string(), CallbackType::BeforeEvent),
                Hook::After(t) => (t.to_string(), CallbackType::AfterEvent),

                Hook::LeaveState => ("".to_string(), CallbackType::LeaveState),
                Hook::EnterState => ("".to_string(), CallbackType::EnterState),
                Hook::Leave(t) => (t.to_string(), CallbackType::LeaveState),
                Hook::Enter(t) => (t.to_string(), CallbackType::EnterState),

                Hook::Custom(t) => {
                    let target = t.to_string();
                    let callback_type = if all_states.contains_key(&target) {
                        CallbackType::EnterState
                    } else if let Ok(t) = T::from_str(t) {
                        if all_events.contains_key(&t) {
                            CallbackType::AfterEvent
                        } else {
                            CallbackType::None
                        }
                    } else {
                        CallbackType::None
                    };
                    (target, callback_type)
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
            _marker: std::marker::PhantomData,
            current: initial.to_string(),
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
            if let Err(err) = self.after_event_callbacks(&e) {
                return Err(FSMError::NoTransitionWithError(err.to_string()));
            }
            return Err(FSMError::NoTransition);
        }

        self.leave_state_callbacks(&e)
            .map_err(|err| FSMError::InternalError(err.to_string()))?;
        self.current = dst.to_string();

        // ignore errors
        let _ = self.enter_state_callbacks(&e);
        let _ = self.after_event_callbacks(&e);

        Ok(())
    }
}

impl<K, V, F> FSM<K, V, F>
where
    F: Action<K, V>,
{
    fn before_event_callbacks(&self, e: &Event<K, V>) -> Result<(), F::Err> {
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

    fn after_event_callbacks(&self, e: &Event<K, V>) -> Result<(), F::Err> {
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

    fn enter_state_callbacks(&self, e: &Event<K, V>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: self.current.clone(),
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

    fn leave_state_callbacks(&self, e: &Event<K, V>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: self.current.clone(),
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
    use super::{EnumType, EventDesc, Hook, FSM};
    use crate::{action::Closure, error::FSMError, event::Event, Action};
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicU32, Ordering},
    };
    use strum::{Display, EnumString};
    use thiserror::Error;

    #[derive(Debug, Error)]
    enum MyError {
        #[error("my error: {0}")]
        CustomeError(&'static str),
    }

    #[derive(Display, EnumString, Debug, Clone, Hash, PartialEq, Eq)]
    enum StateTag {
        #[strum(serialize = "opened")]
        Opened,
        #[strum(serialize = "closed")]
        Closed,
    }

    impl EnumType for StateTag {}

    #[derive(Display, EnumString, Debug, Clone, Hash, PartialEq, Eq)]
    enum EventTag {
        #[strum(serialize = "open")]
        Open,
        #[strum(serialize = "close")]
        Close,
    }
    impl EnumType for EventTag {}

    #[test]
    fn test_fsm_state_parse() {
        let fsm: FSM<u32, u32, Closure<'_, _, _, MyError>> = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            HashMap::new(),
        );
        let state: StateTag = fsm.get_current().parse().unwrap();
        assert_eq!(StateTag::Closed, state);

        assert!(fsm.get_current().parse::<EventTag>().is_err());
    }

    #[test]
    fn test_fsm_state() {
        let mut fsm: FSM<u32, u32, Closure<'_, _, _, MyError>> = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            HashMap::new(),
        );
        assert_eq!("closed", fsm.get_current());

        assert!(fsm.on_event("open", None).is_ok());
        assert_eq!("opened", fsm.get_current());

        assert!(fsm.on_event("close", None).is_ok());
        assert_eq!("closed", fsm.get_current());

        let ret = fsm.on_event("close", None);
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            FSMError::InvalidEvent("close".to_string(), "closed".to_string())
        );
        assert_eq!("closed", fsm.get_current());
    }

    #[test]
    fn test_fsm_before_event_fail() {
        let callbacks = HashMap::from([
            (
                Hook::<EventTag, StateTag>::BeforeEvent,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    Err(MyError::CustomeError("before event fail"))
                })),
            ),
            (
                Hook::<EventTag, StateTag>::AfterEvent,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    Err(MyError::CustomeError("after event fail"))
                })),
            ),
        ]);
        let mut fsm: FSM<u32, u32, Closure<'_, _, _, MyError>> = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            callbacks,
        );
        assert_eq!("closed", fsm.get_current());

        let ret = fsm.on_event("open", None);
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            FSMError::InternalError("my error: before event fail".to_string())
        );
        assert_eq!("closed", fsm.get_current());
    }

    #[test]
    fn test_fsm_leave_state_fail() {
        let callbacks = HashMap::from([(
            Hook::<EventTag, StateTag>::LeaveState,
            Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                Err(MyError::CustomeError("leave state fail"))
            })),
        )]);
        let mut fsm: FSM<u32, u32, Closure<'_, _, _, MyError>> = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            callbacks,
        );
        assert_eq!("closed", fsm.get_current());

        let ret = fsm.on_event("open", None);
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            FSMError::InternalError("my error: leave state fail".to_string())
        );
        assert_eq!("closed", fsm.get_current());
    }

    #[test]
    fn test_fsm_ignore_after_fail() {
        let callbacks = HashMap::from([
            (
                Hook::<EventTag, StateTag>::AfterEvent,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    Err(MyError::CustomeError("after event fail"))
                })),
            ),
            (
                Hook::<EventTag, StateTag>::EnterState,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    Err(MyError::CustomeError("enter state fail"))
                })),
            ),
        ]);
        let mut fsm: FSM<u32, u32, Closure<'_, _, _, MyError>> = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            callbacks,
        );
        assert_eq!("closed", fsm.get_current());
        assert!(fsm.on_event("open", None).is_ok());
        assert_eq!("opened", fsm.get_current());
    }

    #[test]
    fn test_fsm_closed_to_opened() {
        let counter = AtomicU32::new(0);
        let callbacks = HashMap::from([
            (
                Hook::BeforeEvent,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(1, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::AfterEvent,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(5, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::EnterState,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(3, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::LeaveState,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(2, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::Before(EventTag::Open),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(0, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::After(EventTag::Open),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(4, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
        ]);

        let mut fsm = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            callbacks,
        );

        assert_eq!("closed", fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event("open", Some(&hashmap));
        assert_eq!("opened", fsm.get_current());
    }

    #[test]
    fn test_fsm_opened_to_closed() {
        let counter = AtomicU32::new(0);
        let callbacks = HashMap::from([
            (
                Hook::BeforeEvent,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(0, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::AfterEvent,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(5, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::EnterState,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(4, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::LeaveState,
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(2, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::Leave(StateTag::Opened),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(1, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::Enter(StateTag::Closed),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(3, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
        ]);

        let mut fsm = FSM::new(
            StateTag::Opened,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            callbacks,
        );

        assert_eq!("opened", fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event("close", Some(&hashmap));
        assert_eq!("closed", fsm.get_current());
    }

    #[test]
    fn test_fsm_custom() {
        let counter = AtomicU32::new(0);
        let callbacks = HashMap::from([
            (
                Hook::Before(EventTag::Open),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(0, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::Custom("opened"),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(1, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::Before(EventTag::Close),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(2, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
            (
                Hook::Custom("closed"),
                Closure(Box::new(|_e: &Event<u32, u32>| -> Result<(), MyError> {
                    assert_eq!(3, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })),
            ),
        ]);

        let mut fsm = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            callbacks,
        );
        dbg!("{:?}", &fsm);
        assert_eq!("closed", fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event("open", Some(&hashmap));
        assert_eq!("opened", fsm.get_current());
    }

    #[test]
    fn test_struct_action() {
        #[derive(Debug)]
        struct ActionHandler(AtomicU32);
        impl<K, V> Action<K, V> for &ActionHandler {
            type Err = MyError;
            fn call(&self, _e: &Event<K, V>) -> Result<(), Self::Err> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }
        let action = ActionHandler(AtomicU32::new(0));
        let callbacks = HashMap::from([
            (Hook::BeforeEvent, &action),
            (Hook::AfterEvent, &action),
            (Hook::LeaveState, &action),
            (Hook::EnterState, &action),
        ]);
        let mut fsm = FSM::new(
            StateTag::Closed,
            vec![
                EventDesc {
                    name: EventTag::Open,
                    src: vec![StateTag::Closed],
                    dst: StateTag::Opened,
                },
                EventDesc {
                    name: EventTag::Close,
                    src: vec![StateTag::Opened],
                    dst: StateTag::Closed,
                },
            ],
            callbacks,
        );
        let _ = fsm.on_event("open", None::<&HashMap<u32, u32>>);
        assert_eq!(4, action.0.load(Ordering::Relaxed));
    }
}
