use std::collections::HashMap;

use bytes::{Bytes, BytesMut};
use curl::easy::Easy;
use tokio_util::sync::CancellationToken;

use crate::error::{RelayError, Result};

pub(crate) struct TransferHandler {
    body: BytesMut,
    headers: HashMap<String, String>,
}

impl TransferHandler {
    pub(crate) fn new() -> Self {
        Self {
            body: BytesMut::new(),
            headers: HashMap::new(),
        }
    }

    #[tracing::instrument(skip(self, handle), level = "debug")]
    pub(crate) fn handle_transfer(
        &mut self,
        handle: &mut Easy,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        tracing::debug!("Setting up transfer handlers");
        let mut transfer = handle.transfer();

        let body = &mut self.body;
        let headers = &mut self.headers;

        transfer
            .write_function(move |data| {
                body.extend_from_slice(data);
                tracing::trace!(bytes = data.len(), "Received response data chunk");
                Ok(data.len())
            })
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to set write callback");
                RelayError::Network {
                    message: "Failed to set write callback".into(),
                    cause: Some(e.to_string()),
                }
            })?;

        transfer
            .header_function(move |header| {
                if let Ok(header_str) = String::from_utf8(header.to_vec()) {
                    // NOTE: curl passes the headers of every response it receives
                    // (redirects, `100 Continue`, proxy `CONNECT`), each block
                    // starting with its status line. Only keep the last one.
                    if header_str.starts_with("HTTP/") {
                        headers.clear();
                    } else if let Some(idx) = header_str.find(':') {
                        let (key, value) = header_str.split_at(idx);
                        let key = key.trim().to_string();
                        let value = value[1..].trim().to_string();

                        if key.to_lowercase() == "set-cookie" {
                            // NOTE: Special handling workaround.
                            // Concatenate multiple `Set-Cookie` headers.
                            match headers.entry(key) {
                                std::collections::hash_map::Entry::Occupied(mut e) => {
                                    let existing = e.get_mut();
                                    existing.push_str("\n");
                                    existing.push_str(&value);
                                }
                                std::collections::hash_map::Entry::Vacant(e) => {
                                    e.insert(value);
                                }
                            }
                        } else {
                            headers.entry(key).or_insert(value);
                        }
                    }
                }
                true
            })
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to set header callback");
                RelayError::Network {
                    message: "Failed to set header callback".into(),
                    cause: Some(e.to_string()),
                }
            })?;

        transfer
            .progress_function(|_, _, _, _| {
                let cancelled = cancel_token.is_cancelled();
                if cancelled {
                    tracing::warn!("Request cancelled by user");
                }
                !cancelled
            })
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to set progress callback");
                RelayError::Network {
                    message: "Failed to set progress callback".into(),
                    cause: Some(e.to_string()),
                }
            })?;

        tracing::debug!("Starting transfer");
        transfer.perform().map_err(|e| {
            tracing::error!(error = %e, "Failed to perform request");
            RelayError::Network {
                message: "Failed to perform request".into(),
                cause: Some(e.to_string()),
            }
        })?;

        tracing::debug!("Transfer completed successfully");
        Ok(())
    }

    pub(crate) fn into_parts(self) -> (Bytes, HashMap<String, String>) {
        (self.body.into(), self.headers)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// Serves `/redirect` as a 302 to `/final`, closing the connection after each response.
    fn redirect_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = stream.unwrap();
                let mut request_line = String::new();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                reader.read_line(&mut request_line).unwrap();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                }
                let response = if request_line.contains("/redirect") {
                    "HTTP/1.1 302 Found\r\nlocation: /final\r\ncontent-type: text/html\r\nx-hop: redirect\r\nset-cookie: hop=1\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nset-cookie: final=1\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}"
                };
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        base
    }

    #[test]
    fn headers_come_from_the_final_response_after_a_redirect() {
        let base = redirect_server();
        let mut handle = Easy::new();
        handle.url(&format!("{base}/redirect")).unwrap();
        handle.follow_location(true).unwrap();

        let mut transfer = TransferHandler::new();
        transfer
            .handle_transfer(&mut handle, &CancellationToken::new())
            .unwrap();
        let (body, headers) = transfer.into_parts();

        assert_eq!(handle.response_code().unwrap(), 200);
        assert_eq!(&body[..], b"{}");
        assert_eq!(
            headers.get("content-type").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(headers.get("content-length").map(String::as_str), Some("2"));
        assert_eq!(headers.get("x-hop"), None);
        assert_eq!(
            headers.get("set-cookie").map(String::as_str),
            Some("final=1")
        );
    }
}
