use std::collections::HashMap;

use curl::easy::Easy;

use crate::{
    error::{RelayError, Result},
    header::HeadersBuilder,
    interop::AuthType,
};

pub(crate) struct AuthHandler<'a> {
    handle: &'a mut Easy,
}

impl<'a> AuthHandler<'a> {
    pub(crate) fn new(handle: &'a mut Easy) -> Self {
        Self { handle }
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn set_auth(&mut self, auth: &AuthType) -> Result<()> {
        match auth {
            AuthType::Basic { username, password } => {
                tracing::trace!(username = %username, "Setting basic auth");
                self.set_basic_auth(username, password)
            }
            AuthType::Bearer { token } => {
                tracing::trace!("Setting bearer auth");
                self.set_bearer_auth(token)
            }
            AuthType::Digest {
                username, password, ..
            } => {
                tracing::trace!(username = %username, "Setting digest auth");
                self.set_digest_auth(username, password)
            }
            AuthType::None => {
                tracing::trace!("No authentication required");
                Ok(())
            }
        }
    }

    fn set_basic_auth(&mut self, username: &str, password: &str) -> Result<()> {
        self.handle
            .username(username)
            .map_err(|e| RelayError::Network {
                message: "Failed to set username".into(),
                cause: Some(e.to_string()),
            })?;

        self.handle
            .password(password)
            .map_err(|e| RelayError::Network {
                message: "Failed to set password".into(),
                cause: Some(e.to_string()),
            })
    }

    fn set_bearer_auth(&mut self, token: &str) -> Result<()> {
        HeadersBuilder::new(self.handle).add_headers(Some(&HashMap::from([(
            "Authorization".to_string(),
            vec![format!("Bearer {}", token)],
        )])))
    }

    fn set_digest_auth(&mut self, username: &str, password: &str) -> Result<()> {
        self.set_basic_auth(username, password)?;
        let mut auth = curl::easy::Auth::new();
        auth.digest(true);
        self.handle
            .http_auth(&auth)
            .map_err(|e| RelayError::Network {
                message: "Failed to set digest authentication".into(),
                cause: Some(e.to_string()),
            })
    }
}
