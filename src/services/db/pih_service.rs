use anyhow::Context;
use base64::{Engine, engine::general_purpose};
use openssl::memcmp;
use tracing::instrument;

use crate::services::xml::extractors::extract_pih;

#[instrument(skip(invoice, expected_pih), fields(expected_pih_len = expected_pih.len()))]
pub fn verify_pih(invoice: &[u8], expected_pih: &[u8]) -> anyhow::Result<()> {
    let ex_pih_b64 = extract_pih(invoice).context("failed to extract the PIH")?;
    let ex_pih = general_purpose::STANDARD
        .decode(ex_pih_b64)
        .context("failed to decode the extracted PIH")?;

    if ex_pih.len() != expected_pih.len() {
        anyhow::bail!(
            "PIH length mismatch: expected {}, got {}",
            expected_pih.len(),
            ex_pih.len()
        );
    }

    if !memcmp::eq(&ex_pih, expected_pih) {
        anyhow::bail!("PIH hash mismatch: chains do not match ")
    }

    Ok(())
}
