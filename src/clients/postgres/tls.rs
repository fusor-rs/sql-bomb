use crate::Error;
use rustls::{
    ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
    client::{
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        verify_server_cert_signed_by_trust_anchor, verify_server_name,
    },
    crypto::{WebPkiSupportedAlgorithms, ring, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
    server::ParsedCertificate,
};
use std::{ops::Range, sync::Arc};
use tokio_postgres_rustls::MakeRustlsConnect;

const REQUIRE: &str = "sslmode=require";
const VERIFYING_MODES: [(&str, TrustRoots); 2] = [
    ("sslmode=verify-ca", Trust::Authority),
    ("sslmode=verify-full", Trust::AuthorityAndHost),
];

type TrustRoots = fn(RootCertStore) -> Trust;

// As in psql, `prefer` and `require` encrypt without checking the server's certificate.
#[derive(Debug)]
enum Trust {
    Any,
    Authority(RootCertStore),
    AuthorityAndHost(RootCertStore),
}

#[derive(Debug)]
struct ServerCertificate {
    trust: Trust,
    algorithms: WebPkiSupportedAlgorithms,
}

// tokio-postgres only parses `disable`, `prefer`, and `require`, so a verifying `sslmode` is
// returned in the address as `require`, with the check moved into the connector.
pub(super) fn connector(address: &str) -> Result<(String, MakeRustlsConnect), Error> {
    let verifying_mode = VERIFYING_MODES
        .iter()
        .find_map(|(parameter, trust)| Some((parameter_range(address, parameter)?, trust)));
    let mut address = address.to_owned();
    let trust = match verifying_mode {
        Some((range, trust)) => {
            address.replace_range(range, REQUIRE);
            trust(native_roots()?)
        }
        None => Trust::Any,
    };
    let provider = Arc::new(ring::default_provider());
    let verifier = ServerCertificate {
        trust,
        algorithms: provider.signature_verification_algorithms,
    };
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("the ring provider supports the default TLS versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok((address, MakeRustlsConnect::new(config)))
}

// Other spellings, such as quoted values, reach tokio-postgres unchanged and fail to parse.
fn parameter_range(address: &str, parameter: &str) -> Option<Range<usize>> {
    let is_separator =
        |character: char| matches!(character, '?' | '&') || character.is_whitespace();
    address
        .match_indices(parameter)
        .map(|(start, _)| start..start + parameter.len())
        .find(|range| {
            address[..range.start]
                .chars()
                .next_back()
                .is_none_or(is_separator)
                && address[range.end..].chars().next().is_none_or(is_separator)
        })
}

fn native_roots() -> Result<RootCertStore, Error> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        return Err(Error::Certificates);
    }
    let mut roots = RootCertStore::empty();
    // Certificates that fail to parse are skipped; the remaining roots still verify servers.
    roots.add_parsable_certificates(native.certs);
    Ok(roots)
}

impl ServerCertVerifier for ServerCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let roots = match &self.trust {
            Trust::Any => return Ok(ServerCertVerified::assertion()),
            Trust::Authority(roots) | Trust::AuthorityAndHost(roots) => roots,
        };
        let certificate = ParsedCertificate::try_from(end_entity)?;
        verify_server_cert_signed_by_trust_anchor(
            &certificate,
            roots,
            intermediates,
            now,
            self.algorithms.all,
        )?;
        if matches!(self.trust, Trust::AuthorityAndHost(_)) {
            verify_server_name(&certificate, server_name)?;
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, certificate, signature, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, certificate, signature, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}
