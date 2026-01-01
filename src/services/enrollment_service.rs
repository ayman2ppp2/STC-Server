use crate::services::signer::sign;
use openssl::x509::X509;

use crate::{config::crypto_config::Crypto, models::enrollment_DTO::EnrollDTO};

pub async fn handle_enrollment(dto : &EnrollDTO, crypto: Crypto)-> Result<X509,String>{
	let intermediate = dto.parse().await?;

	sign(intermediate.csr, crypto).await.map_err(|e|format!("an error with the creation and signing of the certificate :{}",e))
}