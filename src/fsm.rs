use crate::{action::Action, error::FSMError, event::Event};
use std::{borrow::Cow, collections::HashMap, fmt::Display, hash::Hash};

pub trait EnumType: AsRef<str> + Display + Clone + Hash + PartialEq + Eq {}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum HookType<T: EnumType, S: EnumType> {
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
struct EKey<'a> {
    // event is the name of the event that the keys refers to.
    event: Cow<'a, str>,

    // src is the source from where the event can transition.
    src: Cow<'a, str>,
}

// CKey is a struct key used for keeping the callbacks mapped to a target.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CKey<'a> {
    // target is either the name of a state or an event depending on which
    // callback type the key refers to. It can also be "" for a non-targeted
    // callback like before_event.
    target: Cow<'a, str>,

    // callback_type is the situation when the callback will be run.
    callback_type: CallbackType,
}

#[derive(Debug, Clone)]
pub struct FSM<'a, I, F: Action<I>> {
    _marker: std::marker::PhantomData<I>,

    // current is the state that the FSM is currently in.
    current: String,

    // transitions maps events and source states to destination states.
    transitions: HashMap<EKey<'a>, String>,

    // callbacks maps events and targets to callback functions.
    callbacks: HashMap<CKey<'a>, F>,
}

impl<'a, I, F> FSM<'a, I, F>
where
    I: IntoIterator,
    F: Action<I>,
{
    pub fn new<T, S>(
        initial: S,
        events: impl IntoIterator<Item = EventDesc<T, S>>,
        hooks: impl IntoIterator<Item = (HookType<T, S>, F)>,
    ) -> Self
    where
        T: EnumType,
        S: EnumType,
    {
        let mut all_events = HashMap::new();
        let mut all_states = HashMap::new();
        let mut transitions = HashMap::new();

        for e in events {
            all_events.insert(e.name.to_string(), true);
            for src in e.src.iter() {
                transitions.insert(
                    EKey {
                        event: Cow::Owned(e.name.to_string()),
                        src: Cow::Owned(src.to_string()),
                    },
                    e.dst.to_string(),
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
                HookType::Before(t) => (t.to_string(), CallbackType::BeforeEvent),
                HookType::After(t) => (t.to_string(), CallbackType::AfterEvent),

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
            current: initial.to_string(),
            callbacks,
            transitions,
        }
    }

    // get_current returns the current state of the FSM.
    pub fn get_current(&self) -> &str {
        &self.current
    }

    // on_event initiates a state transition with the named event.
    //
    // The call takes a variable number of arguments that will be passed to the
    // callback, if defined.
    pub fn on_event<T: EnumType>(
        &mut self,
        event: T,
        args: Option<&I>,
    ) -> Result<(), FSMError<String>> {
        let dst = self
            .transitions
            .get(&EKey {
                event: Cow::Borrowed(event.as_ref()),
                src: Cow::Borrowed(&self.current),
            })
            .ok_or_else(|| {
                let e = event.to_string();
                for ekey in self.transitions.keys() {
                    if ekey.event.eq(&e) {
                        return FSMError::InvalidEvent(e, self.current.clone());
                    }
                }
                FSMError::UnknownEvent(e)
            })?;

        let e = Event {
            event: event.as_ref(),
            src: &self.current.clone(),
            dst: &dst.to_string(),
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

    // is returns true if state is the current state.
    pub fn is<S: EnumType>(&self, state: S) -> bool {
        self.current.eq(state.as_ref())
    }

    // can returns true if event can occur in the current state.
    pub fn can<T: EnumType>(&self, event: T) -> bool {
        self.transitions.contains_key(&EKey {
            event: Cow::Borrowed(event.as_ref()),
            src: Cow::Borrowed(&self.current),
        })
    }
}

impl<'a, I, F> FSM<'a, I, F>
where
    I: IntoIterator,
    F: Action<I>,
{
    #[inline]
    fn before_event_callbacks(&self, e: &Event<I>) -> Result<(), F::Err> {
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
    fn after_event_callbacks(&self, e: &Event<I>) -> Result<(), F::Err> {
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
    fn enter_state_callbacks(&self, e: &Event<I>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(&self.current),
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
    fn leave_state_callbacks(&self, e: &Event<I>) -> Result<(), F::Err> {
        if let Some(f) = self.callbacks.get(&CKey {
            target: Cow::Borrowed(&self.current),
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
    use super::{EnumType, EventDesc, HookType, FSM};
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
    impl EnumType for StateTag {}

    #[derive(Display, AsRefStr, Debug, Clone, Hash, PartialEq, Eq)]
    enum EventTag {
        #[strum(serialize = "open")]
        Open,
        #[strum(serialize = "close")]
        Close,
    }
    impl EnumType for EventTag {}

    type FSMWithHashMap<'a> = FSM<'a, HashMap<u32, u32>, Closure<'a, HashMap<u32, u32>, MyError>>;
    type FSMWithVec<'a> = FSM<'a, Vec<u32>, Closure<'a, Vec<u32>, MyError>>;

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
            assert_eq!("closed", fsm.get_current());
            assert!(fsm.is(StateTag::Closed));

            assert!(fsm.can(EventTag::Open));
            assert!(fsm.on_event(EventTag::Open, Some(&HashMap::new())).is_ok());
            assert_eq!("opened", fsm.get_current());
            assert!(fsm.is(StateTag::Opened));

            assert!(fsm.can(EventTag::Close));
            assert!(fsm.on_event(EventTag::Close, Some(&HashMap::new())).is_ok());
            assert_eq!("closed", fsm.get_current());
            assert!(fsm.is(StateTag::Closed));

            assert!(!fsm.can(EventTag::Close));
            let ret = fsm.on_event(EventTag::Close, None);
            assert!(ret.is_err());
            assert_eq!(
                ret.err().unwrap(),
                FSMError::InvalidEvent("close".to_string(), "closed".to_string())
            );
            assert_eq!("closed", fsm.get_current());
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
            assert_eq!("closed", fsm.get_current());

            assert!(fsm.on_event(EventTag::Open, Some(&Vec::new())).is_ok());
            assert_eq!("opened", fsm.get_current());

            assert!(fsm.on_event(EventTag::Close, Some(&Vec::new())).is_ok());
            assert_eq!("closed", fsm.get_current());

            let ret = fsm.on_event(EventTag::Close, None);
            assert!(ret.is_err());
            assert_eq!(
                ret.err().unwrap(),
                FSMError::InvalidEvent("close".to_string(), "closed".to_string())
            );
            assert_eq!("closed", fsm.get_current());
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
            assert_eq!("closed", fsm.get_current());

            assert!(fsm.on_event(EventTag::Open, Some(&Vec::new())).is_ok());
            assert_eq!("opened", fsm.get_current());

            assert!(fsm.on_event(EventTag::Close, Some(&Vec::new())).is_ok());
            assert_eq!("closed", fsm.get_current());

            let ret = fsm.on_event(EventTag::Close, None);
            assert!(ret.is_err());
            assert_eq!(
                ret.err().unwrap(),
                FSMError::InvalidEvent("close".to_string(), "closed".to_string())
            );
            assert_eq!("closed", fsm.get_current());
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
        assert_eq!("closed", fsm.get_current());

        let ret = fsm.on_event(EventTag::Open, None);
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
        assert_eq!("closed", fsm.get_current());

        let ret = fsm.on_event(EventTag::Open, None);
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
        assert_eq!("closed", fsm.get_current());
        assert!(fsm.on_event(EventTag::Open, None).is_ok());
        assert_eq!("opened", fsm.get_current());
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

        assert_eq!("closed", fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event(EventTag::Open, Some(&hashmap));
        assert_eq!("opened", fsm.get_current());
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

        assert_eq!("opened", fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event(EventTag::Close, Some(&hashmap));
        assert_eq!("closed", fsm.get_current());
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
        let mut fsm = FSM::new(
            StateTag::Closed,
            events,
            callbacks,
        );
        dbg!("{:?}", &fsm);
        assert_eq!("closed", fsm.get_current());
        let hashmap = HashMap::from([(1, 11), (2, 22)]);
        let _ = fsm.on_event(EventTag::Open, Some(&hashmap));
        assert_eq!("opened", fsm.get_current());
    }

    #[test]
    fn test_struct_action() {
        #[derive(Debug)]
        struct ActionHandler(AtomicU32);
        impl<I> Action<I> for &ActionHandler {
            type Err = MyError;
            fn call(&self, _e: &Event<I>) -> Result<(), Self::Err> {
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
        let _ = fsm.on_event(EventTag::Open, None::<&HashMap<u32, u32>>);
        assert_eq!(4, action.0.load(Ordering::Relaxed));
    }
}
