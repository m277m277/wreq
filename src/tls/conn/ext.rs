use boring2::{
    error::ErrorStack,
    ssl::{ConnectConfiguration, SslConnectorBuilder, SslSessionRef, SslVerifyMode},
};
use bytes::Bytes;

use crate::{
    Error,
    tls::{
        CertStore, CertificateCompressionAlgorithm,
        conn::cert_compression::{
            BrotliCertificateCompressor, ZlibCertificateCompressor, ZstdCertificateCompressor,
        },
    },
};

/// SslConnectorBuilderExt trait for `SslConnectorBuilder`.
pub trait SslConnectorBuilderExt {
    /// Configure the CertStore for the given `SslConnectorBuilder`.
    fn set_cert_store(self, store: Option<&CertStore>) -> crate::Result<SslConnectorBuilder>;

    /// Configure the certificate verification for the given `SslConnectorBuilder`.
    fn set_cert_verification(self, enable: bool) -> crate::Result<SslConnectorBuilder>;

    /// Configure the certificate compression algorithm for the given `SslConnectorBuilder`.
    fn add_certificate_compression_algorithms(
        self,
        algs: Option<&[CertificateCompressionAlgorithm]>,
    ) -> crate::Result<SslConnectorBuilder>;
}

/// ConnectConfigurationExt trait for `ConnectConfiguration`.
pub trait ConnectConfigurationExt {
    /// Configure the session for the given `ConnectConfiguration`.
    fn set_seesion2(&mut self, session: &SslSessionRef) -> Result<(), ErrorStack>;

    /// Configure the ALPS for the given `ConnectConfiguration`.
    fn set_alps_protos(
        &mut self,
        alps: Option<Bytes>,
        use_new_codepoint: bool,
    ) -> Result<(), ErrorStack>;

    /// Configure the random aes hardware override for the given `ConnectConfiguration`.
    fn set_random_aes_hw_override(&mut self, enable: bool);
}

impl SslConnectorBuilderExt for SslConnectorBuilder {
    #[inline]
    fn set_cert_store(mut self, store: Option<&CertStore>) -> crate::Result<SslConnectorBuilder> {
        if let Some(store) = store {
            store.add_to_tls(&mut self);
        } else {
            self.set_default_verify_paths().map_err(Error::tls)?;
        }

        Ok(self)
    }

    #[inline]
    fn set_cert_verification(mut self, enable: bool) -> crate::Result<SslConnectorBuilder> {
        if enable {
            self.set_verify(SslVerifyMode::PEER);
        } else {
            self.set_verify(SslVerifyMode::NONE);
        }
        Ok(self)
    }

    #[inline]
    fn add_certificate_compression_algorithms(
        mut self,
        algs: Option<&[CertificateCompressionAlgorithm]>,
    ) -> crate::Result<SslConnectorBuilder> {
        if let Some(algs) = algs {
            for algorithm in algs.iter() {
                if algorithm == &CertificateCompressionAlgorithm::ZLIB {
                    self.add_certificate_compression_algorithm(
                        ZlibCertificateCompressor::default(),
                    ).map_err(Error::tls)?;
                }

                if algorithm == &CertificateCompressionAlgorithm::BROTLI {
                    self.add_certificate_compression_algorithm(
                        BrotliCertificateCompressor::default(),
                    )
                    .map_err(Error::tls)?;
                }

                if algorithm == &CertificateCompressionAlgorithm::ZSTD {
                    self.add_certificate_compression_algorithm(
                        ZstdCertificateCompressor::default(),
                    ).map_err(Error::tls)?;
                }
            }
        }

        Ok(self)
    }
}

impl ConnectConfigurationExt for ConnectConfiguration {
    #[inline]
    fn set_alps_protos(
        &mut self,
        alps: Option<Bytes>,
        use_new_codepoint: bool,
    ) -> Result<(), ErrorStack> {
        if let Some(alps) = alps {
            self.add_application_settings(&alps)?;

            // By default, the old endpoint is used. Avoid unnecessary FFI calls.
            if use_new_codepoint {
                self.set_alps_use_new_codepoint(use_new_codepoint);
            }
        }

        Ok(())
    }

    #[inline]
    fn set_random_aes_hw_override(&mut self, enable: bool) {
        if enable {
            let random_bool = (crate::util::fast_random() % 2) == 0;
            self.set_aes_hw_override(random_bool);
        }
    }

    #[inline]
    fn set_seesion2(&mut self, session: &SslSessionRef) -> Result<(), ErrorStack> {
        unsafe {
            self.set_session(session)?;
        }

        Ok(())
    }
}
