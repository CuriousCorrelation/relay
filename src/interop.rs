use std::collections::HashMap;

use http::{StatusCode, Method};
use serde::{Deserialize, Serialize};
use strum::Display;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Display)]
pub enum Protocol {
    #[serde(rename = "http/1.0")]
    #[strum(to_string = "http/1.0")]
    Http10,
    #[serde(rename = "http/1.1")]
    #[strum(to_string = "http/1.1")]
    Http11,
    #[serde(rename = "http/2")]
    #[strum(to_string = "http/1.2")]
    Http2,
    #[serde(rename = "http/3")]
    #[strum(to_string = "http/3")]
    Http3,
}

impl Protocol {
    pub fn to_curl_version(&self) -> curl::easy::HttpVersion {
        match self {
            Protocol::Http10 => curl::easy::HttpVersion::V10,
            Protocol::Http11 => curl::easy::HttpVersion::V11,
            Protocol::Http2 => curl::easy::HttpVersion::V2,
            Protocol::Http3 => curl::easy::HttpVersion::V3,
        }
    }

    pub fn from_curl_version(version: curl::easy::HttpVersion) -> Self {
        match version {
            curl::easy::HttpVersion::V10 => Protocol::Http10,
            curl::easy::HttpVersion::V11 => Protocol::Http11,
            curl::easy::HttpVersion::V2 => Protocol::Http2,
            curl::easy::HttpVersion::V3 => Protocol::Http3,
            _ => Protocol::Http11,
        }
    }
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Display)]
pub enum MediaType {
    #[serde(rename = "text/plain")]
    #[strum(to_string = "text/plain")]
    TextPlain,
    #[serde(rename = "text/html")]
    #[strum(to_string = "text/html")]
    TextHtml,
    #[serde(rename = "text/css")]
    #[strum(to_string = "text/css")]
    TextCss,
    #[serde(rename = "text/csv")]
    #[strum(to_string = "text/csv")]
    TextCsv,
    #[serde(rename = "application/json")]
    #[strum(to_string = "application/json")]
    Json,
    #[serde(rename = "application/ld+json")]
    #[strum(to_string = "application/ld+json")]
    JsonLd,
    #[serde(rename = "application/xml")]
    #[strum(to_string = "application/xml")]
    Xml,
    #[serde(rename = "text/xml")]
    #[strum(to_string = "text/xml")]
    TextXml,
    #[serde(rename = "application/x-www-form-urlencoded")]
    #[strum(to_string = "application/x-www-form-urlencoded")]
    FormUrlEncoded,
    #[serde(rename = "multipart/form-data")]
    #[strum(to_string = "multipart/form-data")]
    MultipartFormData,
    #[serde(rename = "application/octet-stream")]
    #[strum(to_string = "application/octet-stream")]
    OctetStream,
    #[serde(other)]
    Other,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FormValue {
    Text(String),
    File {
        filename: String,
        content_type: MediaType,
        data: Vec<u8>,
    },
}

pub type FormData = HashMap<String, Vec<FormValue>>;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentType {
    Text {
        content: String,
        #[serde(rename = "mediaType")]
        media_type: MediaType,
    },
    Json {
        content: serde_json::Value,
        #[serde(rename = "mediaType")]
        media_type: MediaType,
    },
    Xml {
        content: String,
        #[serde(rename = "mediaType")]
        media_type: MediaType,
    },
    Form {
        content: FormData,
        #[serde(rename = "mediaType")]
        media_type: MediaType,
    },
    Binary {
        content: Vec<u8>,
        #[serde(rename = "mediaType")]
        media_type: MediaType,
        filename: Option<String>,
    },
    Multipart {
        content: FormData,
        #[serde(rename = "mediaType")]
        media_type: MediaType,
    },
    Urlencoded {
        content: HashMap<String, String>,
        #[serde(rename = "mediaType")]
        media_type: MediaType,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthType {
    None,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    Digest {
        username: String,
        password: String,
        realm: Option<String>,
        nonce: Option<String>,
        opaque: Option<String>,
        algorithm: Option<DigestAlgorithm>,
        qop: Option<DigestQop>,
        nc: Option<String>,
        cnonce: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum DigestAlgorithm {
    #[serde(rename = "MD5")]
    Md5,
    #[serde(rename = "SHA-256")]
    Sha256,
    #[serde(rename = "SHA-512")]
    Sha512,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DigestQop {
    Auth,
    AuthInt,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CertificateType {
    Pem { cert: Vec<u8>, key: Vec<u8> },
    Pfx { data: Vec<u8>, password: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecurityConfig {
    pub certificates: Option<CertificateConfig>,
    #[serde(rename = "validateCertificates")]
    pub validate_certificates: Option<bool>,
    #[serde(rename = "verifyHost")]
    pub verify_host: Option<bool>,
    #[serde(rename = "verifyPeer")]
    pub verify_peer: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CertificateConfig {
    pub client: Option<CertificateType>,
    pub ca: Option<Vec<Vec<u8>>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Request {
    pub id: i64,
    pub url: String,
    #[serde(with = "http_serde::method")]
    pub method: Method,
    pub protocol: Protocol,
    pub headers: Option<HashMap<String, Vec<String>>>,
    pub params: Option<HashMap<String, Vec<String>>>,
    pub content: Option<ContentType>,
    pub auth: Option<AuthType>,
    pub security: Option<SecurityConfig>,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Response {
    pub id: i64,
    #[serde(with = "http_serde::status_code")]
    pub status: StatusCode,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub protocol: Protocol,
    pub headers: HashMap<String, Vec<String>>,
    pub cookies: Option<Vec<Cookie>>,
    pub content: ContentType,
    pub meta: ResponseMeta,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    pub url: String,
    pub auth: Option<ProxyAuth>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<OffsetDateTime>,
    pub secure: Option<bool>,
    #[serde(rename = "httpOnly")]
    pub http_only: Option<bool>,
    #[serde(rename = "sameSite")]
    pub same_site: Option<SameSite>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseMeta {
    pub timing: TimingInfo,
    pub size: SizeInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimingInfo {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SizeInfo {
    pub headers: u64,
    pub body: u64,
    pub total: u64,
}
