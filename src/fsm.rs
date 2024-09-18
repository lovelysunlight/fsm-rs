use crate::{action::Action, error::FSMError, event::Event};
use std::{borrow::Cow, collections::HashMap, fmt::Display, hash::Hash};

/// FSMState represents the state of the FSM.
pub trait FSMState: AsRef<Self> + AsRef<str> + Display + Clone + Eq + PartialEq {}

/// HookType represents the type of event.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum HookType<T: AsRef<str>, S: FSMState> {
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

/// CallbackType represents the type of callback.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CallbackType {
    None,
    BeforeEvent,
    LeaveState,
    EnterState,
    AfterEvent,
}

/// EventDesc represents an event when initializing the FSM.
//
// The event can have one or more source states that is valid for performing
// the transition. If the FSM is in one of the source states it will end up in
// the specified destination state, calling all defined callbacks as it goes.
#[derive(Debug)]
pub struct EventDesc<T, S>
where
    T: AsRef<str>,
    S: FSMState,
{
    /// `name` is the event name used when calling for a transition.
    pub name: T,

    /// `src` is a slice of source states that the FSM must be in to perform a
    /// state transition.
    pub src: Vec<S>,

    /// `dst` is the destination state that the FSM will be in if the transition
    /// succeeds.
    pub dst: S,
}

/// EKey is a struct key used for storing the transition map.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EKey<'a> {
    // event is the name of the event that the keys refers to.
    event: Cow<'a, str>,

    // src is the source from where the event can transition.
    src: Cow<'a, str>,
}

/// CKey is a struct key used for keeping the callbacks mapped to a target.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CKey<'a> {
    // target is either the name of a state or an event depending on which
    // callback type the key refers to. It can also be "" for a non-targeted
    // callback like before_event.
    target: Cow<'a, str>,

    // callback_type is the situation when the callback will be run.
    callback_type: CallbackType,
}

/// FSM represents a finite state machine.
///
/// The FSM is initialized with an initial state and a list of events.
///
#[derive(Debug, Clone)]
pub struct FSM<'a, S, I, F: Action<S, I>> {
    _marker: std::marker::PhantomData<I>,

    // current is the state that the FSM is currently in.
    current: S,

    // transitions maps events and source states to destination states.
    transitions: HashMap<EKey<'a>, S>,

    // callbacks maps events and targets to callback functions.
    callbacks: HashMap<CKey<'a>, F>,
}

impl<'a, S, I, F> FSM<'a, S, I, F>
where
    S: FSMState,
    I: IntoIterator,
    F: Action<S, I>,
{
    /// new creates a new FSM.
    pub fn new<T>(
        initial: S,
        events: impl IntoIterator<Item = EventDesc<T, S>>,
        hooks: impl IntoIterator<Item = (HookType<T, S>, F)>,
    ) -> Self
    where
        T: AsRef<str>,
    {
        let mut all_events = HashMap::new();
        let mut all_states = HashMap::new();
        let mut transitions = HashMap::new();

        for e in events {
            all_events.insert(e.name.as_ref().to_string(), true);
            for src in e.src.iter() {
                transitions.insert(
                    EKey {
                        event: Cow::Owned(e.name.as_ref().to_string()),
                        src: Cow::Owned(src.to_string()),
                    },
                    e.dst.clone(),
                );
                all_states.insert(src.to_string(), true);
                all_states.insert(e.dst.to_string(), true);
            }
        }

        let mut callbacks: HashMap<CKey, F> = HashMap::new();
        for (name, callback) in hooks {
            let (target, callback_type) = match name {
                HookType::BeforeEvent => ("".to_string(), CallbackType::BeforeEvent),
                HookType::AfterEvent => ("".to_string(), CallbackType::AfterEvent),
                HookType::Before(t) => (t.as_ref().to_string(), CallbackType::BeforeEvent),
                HookType::After(t) => (t.as_ref().to_string(), CallbackType::AfterEvent),

                HookType::LeaveState => ("".to_string(), CallbackType::LeaveState),
                HookType::EnterState => ("".to_string(), CallbackType::EnterState),
                HookType::Leave(t) => (t.to_string(), CallbackType::LeaveState),
                HookType::Enter(t) => (t.to_string(), CallbackType::EnterState),

                HookType::Custom(t) => {
                    let callback_type = if all_states.contains_key(t) {
                        CallbackType::EnterState
                    } else if all_events.contains_key(t) {
                        CallbackType::AfterEvent
                    } else {
                        CallbackType::None
                    };
                    (t.to_string(), callback_type)
                }
            };

            if callback_type != CallbackType::None {
                callbacks.insert(
                    CKey {
                        target: Cow::Owned(target),
                        callback_type,
                    },
                    callback,
                );
            }
        }
        Self {
            _marker: std::marker::PhantomData,
            current: initial,
            callbacks,
            transitions,
        }
    }

    /// get_current returns the current state of the FSM.
    pub fn get_current(&self) -> S {
        self.current.clone()
    }

    /// on_event initiates a state transition with the named event.
    //
    // The call takes a variable number of arguments that will be passed to the
    // callback, if defined.
    pub fn on_event<T: AsRef<str>>(
        &mut self,
        event: T,
        args: Option<&I>,
    ) -> Result<(), FSMError<String>> {
        let dst = self
            .transitions
            .get(&EKey {
                event: Cow::Borrowed(event.as_ref()),
                src: Cow::Owned(self.current.to_string()),
            })
            .ok_or_else(|| {
                let e = event.as_ref().to_string();
                for ekey in self.transitions.keys() {
                    if ekey.event.eq(&e) {
                        return FSMError::InvalidEvent(e, self.current.to_string());
                    }
                }
                FSMError::UnknownEvent(e)
            })?;

        let e = Event {
            event: event.as_ref(),
            src: &self.current.clone(),
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
        self.current = dst.clone();

        // ignore errors
        let _ = self.enter_state_callbacks(&e);
        let _ = self.after_event_callbacks(&e);

        Ok(())
    }

    /// is returns true if state is the current state.
    pub fn is<T: AsRef<S>>(&self, state: T) -> bool {
        self.current.eq(state.as_ref())
    }

    /// can returns true if event can occur in the current state.
    pub fn can<T: AsRef<str>>(&self, event: T) -> bool {
        self.transitions.contains_key(&EKey {
            event: Cow::Borrowed(event.as_ref()),
            src: Cow::Borrowed(self.current.as_ref()),
        })
    }
}

impl<'a, S, I, F> FSM<'a, S, I, F>
where
    S: FSMState,
    I: IntoIterator,
    F: Action<S, I>,
{
    #[inline]
    fn before_event_callbacks(&self, e: &Event<S, I>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(e.event),
            callback_type: CallbackType::BeforeEvent,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(""),
            callback_type: CallbackType::BeforeEvent,
        }) {
            f.call(e)?;
        }
        Ok(())
    }

    #[inline]
    fn after_event_callbacks(&self, e: &Event<S, I>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(e.event),
            callback_type: CallbackType::AfterEvent,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(""),
            callback_type: CallbackType::AfterEvent,
        }) {
            f.call(e)?;
        }
        Ok(())
    }

    #[inline]
    fn enter_state_callbacks(&self, e: &Event<S, I>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(self.current.as_ref()),
            callback_type: CallbackType::EnterState,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(""),
            callback_type: CallbackType::EnterState,
        }) {
            f.call(e)?;
        }
        Ok(())
    }

    #[inline]
    fn leave_state_callbacks(&self, e: &Event<S, I>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(self.current.as_ref()),
            callback_type: CallbackType::LeaveState,
        }) {
            f.call(e)?;
        }
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(""),
            callback_type: CallbackType::LeaveState,
        }) {
            f.call(e)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{EventDesc, FSMState, HookType, FSM};
    use crate::{action::Closure, error::FSMError, event::Event, Action};
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicU32, Ordering},
    };
    use strum::AsRefStr;
    use strum::Display;
    use thiserror::Error;

    #[derive(Debug, Error)]
    enum MyError {
        #[error("my error: {0}")]
        CustomeError(&'static str),
    }

    #[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
    enum StateTag {
        #[strum(serialize = "opened")]
        Opened,
        #[strum(serialize = "closed")]
        Closed,
    }
    impl FSMState for StateTag {}
    impl AsRef<Self> for StateTag {
        fn as_ref(&self) -> &Self {
            &self
        }
    }

    #[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
    enum EventTag {
        #[strum(serialize = "open")]
        Open,
        #[strum(serialize = "close")]
        Close,
    }

    type FSMWithHashMap<'a> =
        FSM<'a, StateTag, HashMap<u32, u32>, Closure<'a, StateTag, HashMap<u32, u32>, MyError>>;
    type FSMWithVec<'a> = FSM<'a, StateTag, Vec<u32>, Closure<'a, StateTag, Vec<u32>, MyError>>;

    #[test]
    fn test_fsm_state() {
        {
            let mut fsm: FSMWithHashMap = FSM::new(
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
            assert_eq!(StateTag::Closed, fsm.get_current());
            assert!(fsm.is(StateTag::Closed));
            assert!(fsm.is(&StateTag::Closed));

            assert!(fsm.can(EventTag::Open));
            assert!(fsm.on_event("open", Some(&HashMap::new())).is_ok());
            assert_eq!(StateTag::Opened, fsm.get_current());
            assert!(fsm.is(StateTag::Opened));
            assert!(fsm.is(&StateTag::Opened));

            assert!(fsm.can(EventTag::Close));
            assert!(fsm.on_event("close", Some(&HashMap::new())).is_ok());
            assert_eq!(StateTag::Closed, fsm.get_current());
            assert!(fsm.is(StateTag::Closed));
            assert!(fsm.is(&StateTag::Closed));

            assert!(!fsm.can(EventTag::Close));
            let ret = fsm.on_event("close", None);
            assert!(ret.is_err());
            assert_eq!(
                ret.err().unwrap(),
                FSMError::InvalidEvent("close".to_string(), StateTag::Closed.to_string())
            );
            assert_eq!(StateTag::Closed, fsm.get_current());
            assert!(fsm.is(StateTag::Closed));
        }

        {
            let mut fsm: FSMWithVec = FSM::new(
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
            assert_eq!(StateTag::Closed, fsm.get_current());

            assert!(fsm.on_event("open", Some(&Vec::new())).is_ok());
            assert_eq!(StateTag::Opened, fsm.get_current());

            assert!(fsm.on_event("close", Some(&Vec::new())).is_ok());
            assert_eq!(StateTag::Closed, fsm.get_current());

            let ret = fsm.on_event("close", None);
            assert!(ret.is_err());
            assert_eq!(
                ret.err().unwrap(),
                FSMError::InvalidEvent("close".to_string(), "closed".to_string())
            );
            assert_eq!(StateTag::Closed, fsm.get_current());
        }

        {
            let mut fsm: FSMWithVec = FSM::new(
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
                vec![(
                    HookType::<EventTag, StateTag>::BeforeEvent,
                    Closure::new(|_e| -> Result<(), MyError> { Ok(()) }),
                )],
            );
            assert_eq!(StateTag::Closed, fsm.get_current());

            assert!(fsm.on_event("open", Some(&Vec::new())).is_ok());
            assert_eq!(StateTag::Opened, fsm.get_current());

            assert!(fsm.on_event("close", Some(&Vec::new())).is_ok());
            assert_eq!(StateTag::Closed, fsm.get_current());

            let ret = fsm.on_event("close", None);
            assert!(ret.is_err());
            assert_eq!(
                ret.err().unwrap(),
                FSMError::InvalidEvent("close".to_string(), "closed".to_string())
            );
            assert_eq!(StateTag::Closed, fsm.get_current());
        }
    }

    #[test]
    fn test_fsm_before_event_fail() {
        let callbacks = HashMap::from([
            (
                HookType::<EventTag, StateTag>::BeforeEvent,
                Closure::new(|_e| -> Result<(), MyError> {
                    Err(MyError::CustomeError("before event fail"))
                }),
            ),
            (
                HookType::<EventTag, StateTag>::AfterEvent,
                Closure::new(|_e| -> Result<(), MyError> {
                    Err(MyError::CustomeError("after event fail"))
                }),
            ),
        ]);
        let mut fsm: FSMWithHashMap = FSM::new(
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
        assert_eq!(StateTag::Closed, fsm.get_current());

        let ret = fsm.on_event("open", None);
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            FSMError::InternalError("my error: before event fail".to_string())
        );
        assert_eq!(StateTag::Closed, fsm.get_current());
    }

    #[test]
    fn test_fsm_leave_state_fail() {
        let callbacks = HashMap::from([(
            HookType::<EventTag, StateTag>::LeaveState,
            Closure::new(|_e| -> Result<(), MyError> {
                Err(MyError::CustomeError("leave state fail"))
            }),
        )]);
        let mut fsm: FSMWithHashMap = FSM::new(
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
        assert_eq!(StateTag::Closed, fsm.get_current());

        let ret = fsm.on_event("open", None);
        assert!(ret.is_err());
        assert_eq!(
            ret.err().unwrap(),
            FSMError::InternalError("my error: leave state fail".to_string())
        );
        assert_eq!(StateTag::Closed, fsm.get_current());
    }

    #[test]
    fn test_fsm_ignore_after_fail() {
        let callbacks = HashMap::from([
            (
                HookType::<EventTag, StateTag>::AfterEvent,
                Closure::new(|_e| -> Result<(), MyError> {
                    Err(MyError::CustomeError("after event fail"))
                }),
            ),
            (
                HookType::<EventTag, StateTag>::EnterState,
                Closure::new(|_e| -> Result<(), MyError> {
                    Err(MyError::CustomeError("enter state fail"))
                }),
            ),
        ]);
        let mut fsm: FSMWithHashMap = FSM::new(
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
        assert_eq!(StateTag::Closed, fsm.get_current());
        assert!(fsm.on_event("open", None).is_ok());
        assert_eq!(StateTag::Opened, fsm.get_current());
    }

    #[test]
    fn test_fsm_closed_to_opened() {
        let counter = AtomicU32::new(0);
        let callbacks = HashMap::from([
            (
                HookType::BeforeEvent,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(1, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::AfterEvent,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(5, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::EnterState,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(3, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::LeaveState,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(2, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::Before(EventTag::Open),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(0, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::After(EventTag::Open),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(4, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
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

        assert_eq!(StateTag::Closed, fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event("open", Some(&hashmap));
        assert_eq!(StateTag::Opened, fsm.get_current());
    }

    #[test]
    fn test_fsm_opened_to_closed() {
        let counter = AtomicU32::new(0);
        let callbacks = HashMap::from([
            (
                HookType::BeforeEvent,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(0, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::AfterEvent,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(5, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::EnterState,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(4, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::LeaveState,
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(2, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::Leave(StateTag::Opened),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(1, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::Enter(StateTag::Closed),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(3, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
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

        assert_eq!(StateTag::Opened, fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event("close", Some(&hashmap));
        assert_eq!(StateTag::Closed, fsm.get_current());
    }

    #[test]
    fn test_fsm_custom() {
        let counter = AtomicU32::new(0);
        let callbacks = HashMap::from([
            (
                HookType::Before(EventTag::Open),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(0, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::Custom("opened"),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(1, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::Before(EventTag::Close),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(2, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
            (
                HookType::Custom("closed"),
                Closure::new(|_e| -> Result<(), MyError> {
                    assert_eq!(3, counter.load(Ordering::Relaxed));
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
            ),
        ]);
        let events = [
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
        ];
        let mut fsm = FSM::new(StateTag::Closed, events, callbacks);
        dbg!("{:?}", &fsm);
        assert_eq!(StateTag::Closed, fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event("open", Some(&hashmap));
        assert_eq!(StateTag::Opened, fsm.get_current());
    }

    #[test]
    fn test_struct_action() {
        #[derive(Debug)]
        struct ActionHandler(AtomicU32);
        impl<S, I> Action<S, I> for &ActionHandler {
            type Err = MyError;
            fn call(&self, _e: &Event<S, I>) -> Result<(), Self::Err> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }
        let action = ActionHandler(AtomicU32::new(0));
        let callbacks: HashMap<HookType<EventTag, StateTag>, &ActionHandler> = HashMap::from([
            (HookType::BeforeEvent, &action),
            (HookType::AfterEvent, &action),
            (HookType::LeaveState, &action),
            (HookType::EnterState, &action),
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
