use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mahjong_core::UserId;

use crate::token::{random_token, tokens_match};

const SESSION_LIFETIME: Duration = Duration::from_secs(8 * 60 * 60);

#[derive(Clone)]
pub(crate) struct AdminSessions {
    inner: Arc<AdminSessionStore>,
}

struct AdminSessionStore {
    enabled: bool,
    cookie_secure: bool,
    login_csrf: Option<String>,
    sessions: Mutex<HashMap<String, AdminSession>>,
}

struct AdminSession {
    user_id: UserId,
    csrf_token: String,
    expires_at: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdminSessionView {
    pub(crate) token: String,
    pub(crate) user_id: UserId,
    pub(crate) csrf_token: String,
}

impl AdminSessions {
    #[must_use]
    pub(crate) fn disabled() -> Self {
        Self {
            inner: Arc::new(AdminSessionStore {
                enabled: false,
                cookie_secure: false,
                login_csrf: None,
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn enabled(cookie_secure: bool) -> Result<Self, AdminSessionError> {
        Ok(Self {
            inner: Arc::new(AdminSessionStore {
                enabled: true,
                cookie_secure,
                login_csrf: Some(random_token().ok_or(AdminSessionError::Random)?),
                sessions: Mutex::new(HashMap::new()),
            }),
        })
    }

    #[must_use]
    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.enabled
    }

    #[must_use]
    pub(crate) fn login_csrf(&self) -> Option<&str> {
        self.inner.login_csrf.as_deref()
    }

    pub(crate) fn create(&self, user_id: UserId) -> Result<AdminSessionView, AdminSessionError> {
        if !self.is_enabled() {
            return Err(AdminSessionError::Disabled);
        }
        let token = random_token().ok_or(AdminSessionError::Random)?;
        let csrf_token = random_token().ok_or(AdminSessionError::Random)?;
        let session = AdminSession {
            user_id: user_id.clone(),
            csrf_token: csrf_token.clone(),
            expires_at: Instant::now() + SESSION_LIFETIME,
        };
        self.inner
            .sessions
            .lock()
            .map_err(|_| AdminSessionError::LockPoisoned)?
            .insert(token.clone(), session);
        Ok(AdminSessionView {
            token,
            user_id,
            csrf_token,
        })
    }

    pub(crate) fn authenticate(
        &self,
        token: &str,
    ) -> Result<Option<AdminSessionView>, AdminSessionError> {
        let mut sessions = self
            .inner
            .sessions
            .lock()
            .map_err(|_| AdminSessionError::LockPoisoned)?;
        let now = Instant::now();
        sessions.retain(|_, session| session.expires_at > now);
        Ok(sessions.get(token).map(|session| AdminSessionView {
            token: token.to_owned(),
            user_id: session.user_id.clone(),
            csrf_token: session.csrf_token.clone(),
        }))
    }

    pub(crate) fn remove(&self, token: &str) -> Result<(), AdminSessionError> {
        self.inner
            .sessions
            .lock()
            .map_err(|_| AdminSessionError::LockPoisoned)?
            .remove(token);
        Ok(())
    }

    #[must_use]
    pub(crate) fn cookie_secure(&self) -> bool {
        self.inner.cookie_secure
    }

    #[must_use]
    pub(crate) fn csrf_matches(expected: &str, actual: &str) -> bool {
        tokens_match(expected, actual)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminSessionError {
    Disabled,
    LockPoisoned,
    Random,
}

impl Display for AdminSessionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("administrator console is disabled"),
            Self::LockPoisoned => formatter.write_str("administrator session lock is poisoned"),
            Self::Random => formatter.write_str("administrator session token generation failed"),
        }
    }
}

impl Error for AdminSessionError {}

#[cfg(test)]
mod tests {
    use mahjong_core::UserId;

    use super::AdminSessions;

    #[test]
    fn session_and_csrf_tokens_are_independent() {
        let sessions = AdminSessions::enabled(false).expect("sessions");
        let session = sessions.create(UserId::new()).expect("session");

        assert_ne!(session.token, session.csrf_token);
        assert!(AdminSessions::csrf_matches(
            &session.csrf_token,
            &session.csrf_token
        ));
        assert!(!AdminSessions::csrf_matches(
            &session.csrf_token,
            "incorrect"
        ));
        assert_eq!(
            sessions
                .authenticate(&session.token)
                .expect("authenticate")
                .expect("active")
                .user_id,
            session.user_id
        );
    }
}
