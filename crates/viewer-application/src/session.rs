use crate::ports::ProjectAccess;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Empty,
    Opening,
    ActiveReadWrite,
    ActiveReadOnly,
    Closing,
}

#[derive(Debug)]
pub struct ProjectSession {
    state: SessionState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SessionTransitionError {
    #[error("invalid session state transition")]
    Invalid,
}

impl Default for ProjectSession {
    fn default() -> Self {
        Self {
            state: SessionState::Empty,
        }
    }
}

impl ProjectSession {
    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn begin_open(&mut self) -> Result<(), SessionTransitionError> {
        match self.state {
            SessionState::Empty => {
                self.state = SessionState::Opening;
                Ok(())
            }
            _ => Err(SessionTransitionError::Invalid),
        }
    }

    pub fn activate(&mut self, access: ProjectAccess) -> Result<(), SessionTransitionError> {
        match self.state {
            SessionState::Opening => {
                self.state = match access {
                    ProjectAccess::ReadWrite => SessionState::ActiveReadWrite,
                    ProjectAccess::ReadOnly => SessionState::ActiveReadOnly,
                };
                Ok(())
            }
            _ => Err(SessionTransitionError::Invalid),
        }
    }

    pub fn fail_open(&mut self) -> Result<(), SessionTransitionError> {
        match self.state {
            SessionState::Opening => {
                self.state = SessionState::Closing;
                Ok(())
            }
            _ => Err(SessionTransitionError::Invalid),
        }
    }

    pub fn begin_close(&mut self) -> Result<(), SessionTransitionError> {
        match self.state {
            SessionState::ActiveReadWrite | SessionState::ActiveReadOnly => {
                self.state = SessionState::Closing;
                Ok(())
            }
            _ => Err(SessionTransitionError::Invalid),
        }
    }

    pub fn finish_close(&mut self) -> Result<(), SessionTransitionError> {
        match self.state {
            SessionState::Closing => {
                self.state = SessionState::Empty;
                Ok(())
            }
            _ => Err(SessionTransitionError::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_follows_the_only_valid_writeable_path() {
        let mut session = ProjectSession::default();
        session.begin_open().unwrap();
        session.activate(ProjectAccess::ReadWrite).unwrap();
        session.begin_close().unwrap();
        session.finish_close().unwrap();
        assert_eq!(session.state(), SessionState::Empty);
    }

    #[test]
    fn active_session_rejects_second_open() {
        let mut session = ProjectSession::default();
        session.begin_open().unwrap();
        session.activate(ProjectAccess::ReadOnly).unwrap();
        assert_eq!(session.begin_open(), Err(SessionTransitionError::Invalid));
    }

    #[test]
    fn session_supports_the_read_only_path() {
        let mut session = ProjectSession::default();
        session.begin_open().unwrap();
        session.activate(ProjectAccess::ReadOnly).unwrap();
        assert_eq!(session.state(), SessionState::ActiveReadOnly);
        session.begin_close().unwrap();
        session.finish_close().unwrap();
        assert_eq!(session.state(), SessionState::Empty);
    }

    #[test]
    fn failed_open_uses_the_close_cleanup_path() {
        let mut session = ProjectSession::default();
        session.begin_open().unwrap();
        session.fail_open().unwrap();
        assert_eq!(session.state(), SessionState::Closing);
        session.finish_close().unwrap();
        assert_eq!(session.state(), SessionState::Empty);
    }

    #[test]
    fn invalid_transitions_preserve_the_current_state() {
        let mut session = ProjectSession::default();
        assert_eq!(
            session.activate(ProjectAccess::ReadWrite),
            Err(SessionTransitionError::Invalid)
        );
        assert_eq!(session.begin_close(), Err(SessionTransitionError::Invalid));
        assert_eq!(session.finish_close(), Err(SessionTransitionError::Invalid));
        assert_eq!(session.state(), SessionState::Empty);

        session.begin_open().unwrap();
        assert_eq!(session.begin_open(), Err(SessionTransitionError::Invalid));
        assert_eq!(session.begin_close(), Err(SessionTransitionError::Invalid));
        assert_eq!(session.state(), SessionState::Opening);
    }

    #[test]
    fn every_invalid_transition_preserves_state() {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        enum Operation {
            BeginOpen,
            ActivateReadWrite,
            ActivateReadOnly,
            FailOpen,
            BeginClose,
            FinishClose,
        }

        let states = [
            SessionState::Empty,
            SessionState::Opening,
            SessionState::ActiveReadWrite,
            SessionState::ActiveReadOnly,
            SessionState::Closing,
        ];
        let operations = [
            Operation::BeginOpen,
            Operation::ActivateReadWrite,
            Operation::ActivateReadOnly,
            Operation::FailOpen,
            Operation::BeginClose,
            Operation::FinishClose,
        ];
        let mut invalid_cells = 0;

        for state in states {
            for operation in operations {
                let is_legal = matches!(
                    (state, operation),
                    (SessionState::Empty, Operation::BeginOpen)
                        | (SessionState::Opening, Operation::ActivateReadWrite)
                        | (SessionState::Opening, Operation::ActivateReadOnly)
                        | (SessionState::Opening, Operation::FailOpen)
                        | (SessionState::ActiveReadWrite, Operation::BeginClose)
                        | (SessionState::ActiveReadOnly, Operation::BeginClose)
                        | (SessionState::Closing, Operation::FinishClose)
                );
                if is_legal {
                    continue;
                }

                invalid_cells += 1;
                let mut session = ProjectSession { state };
                let result = match operation {
                    Operation::BeginOpen => session.begin_open(),
                    Operation::ActivateReadWrite => session.activate(ProjectAccess::ReadWrite),
                    Operation::ActivateReadOnly => session.activate(ProjectAccess::ReadOnly),
                    Operation::FailOpen => session.fail_open(),
                    Operation::BeginClose => session.begin_close(),
                    Operation::FinishClose => session.finish_close(),
                };

                assert_eq!(
                    result,
                    Err(SessionTransitionError::Invalid),
                    "{state:?} with {operation:?}"
                );
                assert_eq!(
                    session.state(),
                    state,
                    "{state:?} changed after {operation:?}"
                );
            }
        }

        assert_eq!(invalid_cells, 23);
    }
}
