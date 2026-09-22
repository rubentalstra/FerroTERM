//! Asking the server to reload the set it serves.
//!
//! The server re-scans its index root and its resource directory on
//! `POST /reload`, served by the admin listener its `FERROTERM_ADMIN_LISTEN`
//! binds, and answers with the systems it now serves. A reload it refuses
//! leaves the old set answering, which is what makes a failed run harmless.
//!
//! No FHIR specification governs the call: our own design.

use crate::record::ReloadReply;

/// A reload the server did not perform.
#[derive(Debug, thiserror::Error)]
pub enum ReloadError {
    /// The request never reached the server.
    #[error("the reload request to {url} failed")]
    Request {
        /// The address the request was sent to.
        url: String,
        /// Why the request failed.
        #[source]
        source: reqwest::Error,
    },
    /// The server answered, and refused.
    #[error("{url} answered {status}: {body}")]
    Refused {
        /// The address the request was sent to.
        url: String,
        /// The status the server answered with.
        status: u16,
        /// The body it answered with.
        body: String,
    },
}

/// The server's admin listener, as this service speaks to it.
#[derive(Debug, Clone)]
pub struct AdminClient {
    client: reqwest::Client,
    base_url: String,
}

impl AdminClient {
    /// The client for the admin listener at `base_url`.
    #[must_use]
    pub fn new(client: reqwest::Client, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    /// The address `POST /reload` is sent to.
    #[must_use]
    pub fn reload_url(&self) -> String {
        format!("{}/reload", self.base_url)
    }

    /// Asks the server to reload the set it serves.
    ///
    /// # Errors
    ///
    /// Returns [`ReloadError::Request`] when the request does not reach the
    /// server and [`ReloadError::Refused`] when the server answers with a
    /// status that is not a success, in which case the old set still answers.
    pub async fn reload(&self) -> Result<ReloadReply, ReloadError> {
        let url = self.reload_url();
        let response =
            self.client
                .post(&url)
                .send()
                .await
                .map_err(|source| ReloadError::Request {
                    url: url.clone(),
                    source,
                })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|source| ReloadError::Request {
                url: url.clone(),
                source,
            })?;
        if !status.is_success() {
            return Err(ReloadError::Refused {
                url,
                status: status.as_u16(),
                body,
            });
        }
        Ok(ReloadReply {
            status: status.as_u16(),
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AdminClient;

    #[test]
    fn the_reload_address_sits_under_the_base() {
        let client = AdminClient::new(reqwest::Client::new(), "http://server:8081/");
        assert_eq!(
            client.reload_url(),
            "http://server:8081/reload",
            "a trailing slash in the configuration does not double up"
        );
    }
}
