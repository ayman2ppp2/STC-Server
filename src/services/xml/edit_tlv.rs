use anyhow::{Context, bail};
use base64::{Engine, engine::general_purpose};

/// Edits TLV-encoded QR code data by replacing hash, signature, and certificate values.
/// Returns the modified data as a base64-encoded string.
pub fn edit_tlv(
    qr_b64: &[u8],
    hash: &[u8],
    signature: &[u8],
    certificate: &[u8],
) -> anyhow::Result<String> {
    let bytes = general_purpose::STANDARD.decode(qr_b64)?;
    let mut records = extract_records(&bytes)?;
    let mut hash_found = false;
    let mut signature_found = false;
    let mut certificate_found = false;
    for (tag, value) in records.iter_mut() {
        match tag {
            6 => {
                *value = hash.to_vec();
                hash_found = true;
            }
            7 => {
                *value = signature.to_vec();
                signature_found = true;
            }
            8 => {
                *value = certificate.to_vec();
                certificate_found = true;
            }
            _ => {}
        }
    }
    if !hash_found {
        bail!("QR TLV is missing invoice hash tag");
    }
    if !signature_found {
        bail!("QR TLV is missing signature tag");
    }
    if !certificate_found {
        bail!("QR TLV is missing certificate tag");
    }
    new_tlv(records)
}

/// Extracts TLV records from raw bytes. Returns a vector of (tag, value) tuples.
pub fn extract_records(tlv_bytes: &[u8]) -> anyhow::Result<Vec<(u8, Vec<u8>)>> {
    let mut pos = 0;
    let mut records = Vec::new();

    while pos < tlv_bytes.len() {
        let tag = tlv_bytes[pos];
        pos += 1;

        let len = *tlv_bytes
            .get(pos)
            .context("truncated TLV record: missing length")? as usize;
        pos += 1;

        let actual_len = if len <= 0x7f {
            len
        } else if len == 0x81 {
            let actual_len = *tlv_bytes
                .get(pos)
                .context("truncated TLV record: missing long-form length")?
                as usize;
            pos += 1;
            actual_len
        } else if len == 0x82 {
            let high = *tlv_bytes
                .get(pos)
                .context("truncated TLV record: missing long-form length high byte")?
                as usize;
            let low = *tlv_bytes
                .get(pos + 1)
                .context("truncated TLV record: missing long-form length low byte")?
                as usize;
            pos += 2;
            (high << 8) | low
        } else {
            bail!("unsupported TLV length form");
        };

        let end = pos.checked_add(actual_len).context("TLV length overflow")?;
        if end > tlv_bytes.len() {
            bail!("truncated TLV record: value shorter than declared length");
        }
        records.push((tag, tlv_bytes[pos..end].to_vec()));
        pos = end;
    }
    Ok(records)
}
fn new_tlv(records: Vec<(u8, Vec<u8>)>) -> anyhow::Result<String> {
    let mut new_tlv = Vec::new();
    for (tag, value) in records {
        new_tlv.push(tag);
        push_tlv_len(&mut new_tlv, value.len());
        new_tlv.extend_from_slice(&value);
    }
    Ok(general_purpose::STANDARD.encode(&new_tlv))
}

// Helper to push length in ASN.1 DER format
fn push_tlv_len(buf: &mut Vec<u8>, len: usize) {
    if len <= 127 {
        buf.push(len as u8);
    } else if len <= 255 {
        buf.push(0x81);
        buf.push(len as u8);
    } else {
        buf.push(0x82);
        buf.push((len >> 8) as u8);
        buf.push((len & 0xFF) as u8);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_edit_tlv() {
        let qr = general_purpose::STANDARD.encode([6, 1, 1, 7, 1, 2, 8, 1, 3]);
        edit_tlv(qr.as_bytes(), &[0u8; 9], &[0u8; 9], &[0u8; 9]).unwrap();
    }

    #[test]
    fn test_extract_records_rejects_missing_length() {
        let err = extract_records(&[6]).unwrap_err().to_string();
        assert!(err.contains("missing length"));
    }

    #[test]
    fn test_extract_records_rejects_truncated_value() {
        let err = extract_records(&[6, 4, 1, 2]).unwrap_err().to_string();
        assert!(err.contains("value shorter"));
    }

    #[test]
    fn test_extract_records_rejects_unsupported_length_form() {
        let err = extract_records(&[6, 0x83, 0, 0, 1, 0])
            .unwrap_err()
            .to_string();
        assert!(err.contains("unsupported TLV length form"));
    }

    #[test]
    fn test_edit_tlv_replaces_hash_and_signature() {
        let qr = general_purpose::STANDARD.encode([1, 1, b'a', 6, 1, 1, 7, 1, 2, 8, 1, 3]);
        let edited = edit_tlv(qr.as_bytes(), &[9, 8], &[7, 6, 5], &[4, 3]).unwrap();
        let edited_bytes = general_purpose::STANDARD.decode(edited).unwrap();
        let records = extract_records(&edited_bytes).unwrap();

        assert_eq!(records[1], (6, vec![9, 8]));
        assert_eq!(records[2], (7, vec![7, 6, 5]));
        assert_eq!(records[3], (8, vec![4, 3]));
    }

    #[test]
    fn test_edit_tlv_rejects_missing_hash_tag() {
        let qr = general_purpose::STANDARD.encode([7, 1, 2, 8, 1, 3]);
        let err = edit_tlv(qr.as_bytes(), &[9], &[8], &[7])
            .unwrap_err()
            .to_string();
        assert!(err.contains("invoice hash tag"));
    }

    #[test]
    fn test_edit_tlv_rejects_missing_signature_tag() {
        let qr = general_purpose::STANDARD.encode([6, 1, 1, 8, 1, 3]);
        let err = edit_tlv(qr.as_bytes(), &[9], &[8], &[7])
            .unwrap_err()
            .to_string();
        assert!(err.contains("signature tag"));
    }

    #[test]
    fn test_edit_tlv_rejects_missing_certificate_tag() {
        let qr = general_purpose::STANDARD.encode([6, 1, 1, 7, 1, 2]);
        let err = edit_tlv(qr.as_bytes(), &[9], &[8], &[7])
            .unwrap_err()
            .to_string();
        assert!(err.contains("certificate tag"));
    }
}
