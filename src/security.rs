use std::sync::{Once, OnceLock};

use bytes::Bytes;
use curl::easy::Easy;

use openssl::pkcs12::Pkcs12;

use crate::{
    error::{RelayError, Result},
    interop::{CertificateConfig, CertificateType, SecurityConfig},
};

// Vendored OpenSSL is built without a CA bundle or a compiled-in path to the
// host trust store. `init_ssl_cert_env_vars` exports `SSL_CERT_FILE` and
// `SSL_CERT_DIR` from the platform probe, and curl reads them through
// `SSL_CTX_set_default_verify_paths` whenever no explicit blob is set. curl
// already depends on `openssl-probe`, and declaring it here keeps this call
// compiling if a later curl release drops that dependency.
static SSL_ENV_INIT: Once = Once::new();

pub(crate) fn ensure_system_ssl_env() {
    SSL_ENV_INIT.call_once(|| {
        openssl_probe::init_ssl_cert_env_vars();
    });
}

// Cached so the probe and the file read each happen once per process. A host
// with no system bundle caches the empty `Vec` as its result, so later requests
// skip the probe as well.
static SYSTEM_CA_BUNDLE: OnceLock<Vec<u8>> = OnceLock::new();

fn system_ca_bundle() -> &'static [u8] {
    SYSTEM_CA_BUNDLE
        .get_or_init(|| {
            ensure_system_ssl_env();
            let probe = openssl_probe::probe();
            if let Some(path) = probe.cert_file {
                std::fs::read(&path).unwrap_or_default()
            } else {
                Vec::new()
            }
        })
        .as_slice()
}

pub(crate) struct SecurityHandler<'a> {
    handle: &'a mut Easy,
}

impl<'a> SecurityHandler<'a> {
    pub(crate) fn new(handle: &'a mut Easy) -> Self {
        Self { handle }
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn configure(&mut self, security: &SecurityConfig) -> Result<()> {
        tracing::info!("Configuring security settings");

        if let Some(verify) = security.verify_peer {
            tracing::debug!(verify = verify, "Setting SSL verify peer");
            self.handle.ssl_verify_peer(verify).map_err(|e| {
                tracing::error!(error = %e, "Failed to set SSL verify peer");
                RelayError::Certificate {
                    message: "Failed to set SSL verify peer".into(),
                    cause: Some(e.to_string()),
                }
            })?;
        }

        if let Some(verify) = security.verify_host {
            tracing::debug!(verify = verify, "Setting SSL verify host");
            self.handle.ssl_verify_host(verify).map_err(|e| {
                tracing::error!(error = %e, "Failed to set SSL verify host");
                RelayError::Certificate {
                    message: "Failed to set SSL verify host".into(),
                    cause: Some(e.to_string()),
                }
            })?;
        }

        if let Some(ref certs) = security.certificates {
            self.configure_certificates(certs)?;
        }

        tracing::debug!("Security configuration complete");
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    fn configure_certificates(&mut self, certs: &CertificateConfig) -> Result<()> {
        if let Some(ref client_cert) = certs.client {
            match client_cert {
                CertificateType::Pem { cert, key } => {
                    tracing::info!("Configuring PEM certificate");
                    self.configure_pem_certificate(cert, key)?;
                }
                CertificateType::Pfx { data, password } => {
                    tracing::info!("Configuring PKCS#12 certificate");
                    self.configure_pfx_certificate(data, password)?;
                }
            }
        }

        if let Some(ref ca_certs) = certs.ca {
            self.configure_ca_certificates(ca_certs)?;
        }

        Ok(())
    }

    fn configure_pem_certificate(&mut self, cert: &[u8], key: &[u8]) -> Result<()> {
        tracing::debug!("Setting PEM certificate type");
        self.handle.ssl_cert_type("PEM").map_err(|e| {
            tracing::error!(error = %e, "Failed to set certificate type");
            RelayError::Certificate {
                message: "Failed to set certificate type".into(),
                cause: Some(e.to_string()),
            }
        })?;

        tracing::debug!("Setting PEM certificate data");
        self.handle.ssl_cert_blob(cert).map_err(|e| {
            tracing::error!(error = %e, "Failed to set client certificate");
            RelayError::Certificate {
                message: "Failed to set client certificate".into(),
                cause: Some(e.to_string()),
            }
        })?;

        tracing::debug!("Setting PEM key type");
        self.handle.ssl_key_type("PEM").map_err(|e| {
            tracing::error!(error = %e, "Failed to set key type");
            RelayError::Certificate {
                message: "Failed to set key type".into(),
                cause: Some(e.to_string()),
            }
        })?;

        tracing::debug!("Setting PEM key data");
        self.handle.ssl_key_blob(key).map_err(|e| {
            tracing::error!(error = %e, "Failed to set client key");
            RelayError::Certificate {
                message: "Failed to set client key".into(),
                cause: Some(e.to_string()),
            }
        })?;

        Ok(())
    }

    fn configure_pfx_certificate(&mut self, data: &[u8], password: &str) -> Result<()> {
        let pkcs12 = Pkcs12::from_der(data).map_err(|e| {
            tracing::error!(error = %e, "Failed to parse PKCS#12 data");
            RelayError::Certificate {
                message: "Failed to parse PKCS#12 data".into(),
                cause: Some(e.to_string()),
            }
        })?;

        let parsed = pkcs12.parse2(password).map_err(|e| {
            tracing::error!(error = %e, "Failed to parse PKCS#12 password");
            RelayError::Certificate {
                message: "Failed to parse PKCS#12 password".into(),
                cause: Some(e.to_string()),
            }
        })?;

        if let (Some(cert), Some(key)) = (parsed.cert, parsed.pkey) {
            let cert_pem = cert.to_pem().map_err(|e| {
                tracing::error!(error = %e, "Failed to convert certificate to PEM");
                RelayError::Certificate {
                    message: "Failed to convert certificate to PEM".into(),
                    cause: Some(e.to_string()),
                }
            })?;

            let key_pem = key.private_key_to_pem_pkcs8().map_err(|e| {
                tracing::error!(error = %e, "Failed to convert private key to PEM");
                RelayError::Certificate {
                    message: "Failed to convert private key to PEM".into(),
                    cause: Some(e.to_string()),
                }
            })?;

            self.configure_pem_certificate(&cert_pem, &key_pem)
        } else {
            tracing::error!("PKCS#12 file missing certificate or private key");
            Err(RelayError::Certificate {
                message: "PKCS#12 file missing certificate or private key".into(),
                cause: None,
            })
        }
    }

    // `CURLOPT_CAINFO_BLOB` replaces its previous value on every call and
    // overrides `CURLOPT_CAINFO`, so setting one blob per cert would keep only
    // the last cert and drop the system trust store as well. Concatenating the
    // system anchors first and the user CAs after them into one blob, set once,
    // extends the host store with every user CA.
    fn configure_ca_certificates(&mut self, ca_certs: &[Bytes]) -> Result<()> {
        let mut combined: Vec<u8> = Vec::new();
        let system = system_ca_bundle();
        combined.extend_from_slice(system);
        if !system.is_empty() && !combined.ends_with(b"\n") {
            combined.push(b'\n');
        }
        for (index, cert) in ca_certs.iter().enumerate() {
            tracing::debug!(cert_index = index, "Appending CA certificate");
            combined.extend_from_slice(cert);
            if !combined.ends_with(b"\n") {
                combined.push(b'\n');
            }
        }

        tracing::debug!(
            user_certs = ca_certs.len(),
            system_bytes = system.len(),
            "Setting combined CA bundle"
        );
        self.handle.ssl_cainfo_blob(&combined).map_err(|e| {
            tracing::error!(error = %e, "Failed to set combined CA bundle");
            RelayError::Certificate {
                message: "Failed to set combined CA bundle".into(),
                cause: Some(e.to_string()),
            }
        })?;
        Ok(())
    }
}
