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

        tracing::info!(
            header_count = headers.len(),
            "Building complete header list"
        );
        let mut list = List::new();

        for (key, values) in headers {
            tracing::debug!(
                key = %key,
                value_count = values.len(),
                values = ?values,
                "Processing header group"
            );

            for value in values {
                let header = format!("{}: {}", key, value);
                tracing::debug!(header = %header, "Adding header to list");

                list.append(&header).map_err(|e| {
                    tracing::error!(
                        error = %e,
                        key = %key,
                        value = %value,
                        "Failed to append header to list"
                    );
                    RelayError::Network {
                        message: format!("Failed to append header {}: {}", key, value),
                        cause: Some(e.to_string()),
                    }
                })?;
            }
        }

        tracing::info!("Setting complete header list on curl handle");
        self.handle.http_headers(list).map_err(|e| {
            tracing::error!(error = %e, "Failed to set complete header list");
            RelayError::Network {
                message: "Failed to set complete headers".into(),
                cause: Some(e.to_string()),
            }
        })
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn add_content_type(&mut self, content_type: &str) -> Result<()> {
        tracing::info!(content_type = %content_type, "Adding content-type header");
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), vec![content_type.to_string()]);
        self.add_headers(Some(&headers))
    }
}
