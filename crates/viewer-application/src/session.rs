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
}
