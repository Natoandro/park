use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Exited,
    Failed,
    Killed,
}

impl ProcessState {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Exited | Self::Failed | Self::Killed)
    }

    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Starting, Self::Running | Self::Failed)
                | (
                    Self::Running,
                    Self::Stopping | Self::Exited | Self::Failed | Self::Killed
                )
                | (Self::Stopping, Self::Exited | Self::Failed | Self::Killed)
        )
    }

    pub fn transition_to(self, next: Self) -> Result<Self, InvalidStateTransition> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(InvalidStateTransition {
                from: self,
                to: next,
            })
        }
    }

    pub fn validate_action(self, action: LifecycleAction) -> Result<(), InvalidLifecycleAction> {
        let valid = match action {
            LifecycleAction::Stop => matches!(self, Self::Running),
            LifecycleAction::Signal => {
                matches!(self, Self::Starting | Self::Running | Self::Stopping)
            }
            LifecycleAction::Restart | LifecycleAction::Start | LifecycleAction::Remove => {
                self.is_terminal()
            }
        };

        if valid {
            Ok(())
        } else {
            Err(InvalidLifecycleAction {
                state: self,
                action,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    Stop,
    Signal,
    Restart,
    Start,
    Remove,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid process state transition from {from:?} to {to:?}")]
pub struct InvalidStateTransition {
    pub from: ProcessState,
    pub to: ProcessState,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("cannot perform {action:?} while process is {state:?}")]
pub struct InvalidLifecycleAction {
    pub state: ProcessState,
    pub action: LifecycleAction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_start_and_stop_transitions() {
        assert_eq!(
            ProcessState::Starting
                .transition_to(ProcessState::Running)
                .expect("starting should become running"),
            ProcessState::Running
        );
        assert_eq!(
            ProcessState::Running
                .transition_to(ProcessState::Stopping)
                .expect("running should become stopping"),
            ProcessState::Stopping
        );
    }

    #[test]
    fn rejects_invalid_transitions_and_actions() {
        assert!(
            ProcessState::Starting
                .transition_to(ProcessState::Stopping)
                .is_err()
        );
        assert!(
            ProcessState::Running
                .validate_action(LifecycleAction::Restart)
                .is_err()
        );
        assert!(
            ProcessState::Exited
                .validate_action(LifecycleAction::Stop)
                .is_err()
        );
    }

    #[test]
    fn recognizes_terminal_states() {
        assert!(ProcessState::Exited.is_terminal());
        assert!(ProcessState::Failed.is_terminal());
        assert!(ProcessState::Killed.is_terminal());
        assert!(!ProcessState::Running.is_terminal());
    }
}
