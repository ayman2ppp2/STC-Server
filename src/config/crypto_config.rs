use std::env;

use base64::{Engine, engine::general_purpose};
use openssl::{pkey::Private, x509::X509};

pub struct Crypto {
    pub private_key: openssl::pkey::PKey<Private>,
    pub certificate: X509,
}

impl Crypto {
   pub async fn from_env() -> Result<Self, String> {
        // get the base64 and then parse the base64 to binary then to openssl shit
        let private_key_base64 =
            env::var("SEC_PRIVATE_KEY").map_err(|_| "failed to read the private_key from env")?;
        let certificate_base64 =
            env::var("SEC_CERTIFICATE").map_err(|_| "failed to read the certificate from env")?;
        let private_key_binary = general_purpose::STANDARD
            .decode(&private_key_base64)
            .map_err(|_| "failed to parse the private_key_binary")?;
        let certificate_binary = general_purpose::STANDARD
            .decode(&certificate_base64)
            .map_err(|_| "failed to parse the certificate_binary")?;
        let private_key = openssl::pkey::PKey::private_key_from_raw_bytes(
            &private_key_binary,
            openssl::pkey::Id::RSA,
        ).map_err(|_| "failed to convert the binary into a private key")?;
        let certificate = X509::from_der(&certificate_binary).map_err(|_| "failed to convert the binary into a certificate")?;
        Ok(Crypto { private_key, certificate })
    }
}
