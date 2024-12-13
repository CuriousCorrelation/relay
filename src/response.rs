use std::{collections::HashMap, time::SystemTime};

use bytes::Bytes;
use http::{StatusCode, Version};

use crate::{
    error::{RelayError, Result},
    interop::{MediaType, Response, ResponseBody, ResponseMeta, SizeInfo, TimingInfo},
};

pub(crate) struct ResponseHandler {
    id: i64,
    headers: HashMap<String, Vec<String>>,
    body: Bytes,
    status: StatusCode,
    header_size: u64,
    start_time: SystemTime,
    end_time: SystemTime,
    version: Version,
}

impl ResponseHandler {
    pub(crate) fn new(
        id: i64,
        headers: HashMap<String, Vec<String>>,
        body: Bytes,
        status: StatusCode,
        header_size: u64,
        start_time: SystemTime,
        end_time: SystemTime,
        version: Version,
    ) -> Self {
        Self {
            id,
            headers,
            body,
            status,
            header_size,
            start_time,
            end_time,
            version,
        }
    }

    #[tracing::instrument(skip(self), fields(request_id = self.id), level = "debug")]
    pub(crate) fn build(self) -> Result<Response> {
        tracing::debug!(status = %self.status, "Building response");
        let media_type = self.determine_media_type();
        let timing = self.calculate_timing()?;
        let size = SizeInfo {
            headers: self.header_size,
            body: self.body.len() as u64,
            total: self.header_size + self.body.len() as u64,
        };

        tracing::debug!(
            status = ?self.status,
            media_type = ?media_type,
            body_size = size.body,
            total_size = size.total,
            version = ?self.version,
            "Response built successfully"
        );

        let body = ResponseBody {
            body: self.body,
            media_type,
        };

        Ok(Response {
            id: self.id,
            status: self.status,
            status_text: self.status.to_string(),
            version: self.version,
            headers: self.headers,
            cookies: None,
            meta: ResponseMeta { timing, size },
            body,
        })
    }

    fn determine_media_type(&self) -> MediaType {
        tracing::trace!("Determining response content type");

        self.headers
            .get("Content-Type")
            .and_then(|t| t.first())
            .map(|content_type| {
                if content_type.starts_with(MediaType::TextPlain.as_ref()) {
                    MediaType::TextPlain
                } else if content_type.starts_with(MediaType::TextHtml.as_ref()) {
                    MediaType::TextHtml
                } else if content_type.starts_with(MediaType::TextCss.as_ref()) {
                    MediaType::TextCss
                } else if content_type.starts_with(MediaType::TextCsv.as_ref()) {
                    MediaType::TextCsv
                } else if content_type.starts_with(MediaType::Json.as_ref()) {
                    MediaType::Json
                } else if content_type.starts_with(MediaType::JsonLd.as_ref()) {
                    MediaType::JsonLd
                } else if content_type.starts_with(MediaType::Xml.as_ref()) {
                    MediaType::Xml
                } else if content_type.starts_with(MediaType::TextXml.as_ref()) {
                    MediaType::TextXml
                } else if content_type.starts_with(MediaType::FormUrlEncoded.as_ref()) {
                    MediaType::FormUrlEncoded
                } else if content_type.starts_with(MediaType::MultipartFormData.as_ref()) {
                    MediaType::MultipartFormData
                } else if content_type.starts_with(MediaType::OctetStream.as_ref()) {
                    MediaType::OctetStream
                } else {
                    MediaType::TextPlain
                }
            })
            .unwrap_or(MediaType::TextPlain)
    }

    fn calculate_timing(&self) -> Result<TimingInfo> {
        let start_ms = self
            .start_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to get start time");
                RelayError::Parse {
                    message: "Failed to get start time".into(),
                    cause: Some(e.to_string()),
                }
            })?
            .as_millis() as u64;

        let end_ms = self
            .end_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to get end time");
                RelayError::Parse {
                    message: "Failed to get end time".into(),
                    cause: Some(e.to_string()),
                }
            })?
            .as_millis() as u64;

        tracing::trace!(
            start_ms = start_ms,
            end_ms = end_ms,
            duration_ms = end_ms - start_ms,
            "Calculated request timing"
        );

        Ok(TimingInfo {
            start: start_ms,
            end: end_ms,
        })
    }
}
