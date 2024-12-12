use std::collections::HashMap;

use curl::easy::{Easy, List};

use crate::error::{RelayError, Result};

pub(crate) struct HeadersBuilder<'a> {
    handle: &'a mut Easy,
}

impl<'a> HeadersBuilder<'a> {
    pub(crate) fn new(handle: &'a mut Easy) -> Self {
        Self { handle }
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn add_headers(
        &mut self,
        headers: Option<&HashMap<String, Vec<String>>>,
    ) -> Result<()> {
        let Some(headers) = headers else {
            tracing::debug!("No headers provided to add_headers");
            return Ok(());
        };

        tracing::debug!(header_count = headers.len(), "Adding headers");
        let mut list = List::new();

        for (key, values) in headers {
            tracing::debug!(
                key = %key,
                value_count = values.len(),
                values = ?values,
                "Processing header"
            );

            for value in values {
                let header = format!("{}: {}", key, value);
                tracing::debug!(header = %header, "Appending header");

                list.append(&header).map_err(|e| {
                    tracing::error!(
                        error = %e,
                        key = %key,
                        value = %value,
                        "Failed to append header to list"
                    );
                    RelayError::Network {
                        message: "Failed to append header".into(),
                        cause: Some(e.to_string()),
                    }
                })?;
            }
        }

        tracing::debug!("Setting all headers on curl handle");
        self.handle.http_headers(list).map_err(|e| {
            tracing::error!(
                error = %e,
                "Failed to set headers on curl handle"
            );
            RelayError::Network {
                message: "Failed to set headers".into(),
                cause: Some(e.to_string()),
            }
        })
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn add_content_type(&mut self, content_type: &str) -> Result<()> {
        tracing::debug!(content_type = %content_type, "Adding content-type header");

        let mut list = List::new();
        let header = format!("Content-Type: {}", content_type);

        tracing::debug!(header = %header, "Appending content-type header");
        list.append(&header).map_err(|e| {
            tracing::error!(
                error = %e,
                content_type = %content_type,
                "Failed to append content-type header to list"
            );
            RelayError::Network {
                message: "Failed to set content type".into(),
                cause: Some(e.to_string()),
            }
        })?;

        tracing::debug!("Setting content-type header on curl handle");
        self.handle.http_headers(list).map_err(|e| {
            tracing::error!(
                error = %e,
                content_type = %content_type,
                "Failed to set content-type header on curl handle"
            );
            RelayError::Network {
                message: "Failed to set content type header".into(),
                cause: Some(e.to_string()),
            }
        })
    }
}
