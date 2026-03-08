
use base64::{Engine, engine::general_purpose};

pub fn edit_tlv(qr_b64: &[u8], hash: &[u8], signature: &[u8]) -> anyhow::Result<String> {
    let bytes = general_purpose::STANDARD.decode(qr_b64).unwrap();
    let mut records = extract_records(&bytes);
    for (tag, value) in records.iter_mut() {
        match tag {
            6 => *value = hash.to_vec(),
            7 => *value = signature.to_vec(),
            _ => {}
        }
    }
    new_tlv(records)
}

pub fn extract_records(tlv_bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut pos = 0;
    let mut records = Vec::new();

    while pos < tlv_bytes.len() {
        let tag = tlv_bytes[pos];
        pos += 1;

        // Read length
        let mut len = tlv_bytes[pos] as usize;
        pos += 1;
        if len == 0x81 {
            len = tlv_bytes[pos] as usize;
            pos += 1;
        } else if len == 0x82 {
            len = ((tlv_bytes[pos] as usize) << 8) | (tlv_bytes[pos + 1] as usize);
            pos += 2;
        }

        // Read value
        let value = tlv_bytes[pos..pos + len].to_vec();
        pos += len;
        records.push((tag, value));
    }
    records
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
       edit_tlv(
            b"AQtNeSBTdXBwbGllcgIJMTIzNDU2Nzg5AxoyMDI2LTAyLTI0VDAyOjU1OjI2LjM5NjM1OQQHMzAwMC4wMAUGNDUwLjAwBixINFRlZjBkd08vU3NvanpCRjI0ZUQ3VXIzcHREbFVEVGRZVE9ZQzNSOXpzPQeCAQAewh0lS1z7Jb3Jx7ns8zRuDTmge3VBVPITKezXc5u6Ps1BmsBBZxB3xf+8BSo1mBP2059opJxeJjoJ2AOEFzCts9XaPgjAMk4LMq5kyNGUUycAhF3KIndmG9NvjaklmUVzGhR0fJf2SBk0vrBcRQRojx3WuBQOUL/NOpnzztozdcjw29GoU6WgwZsm5nEd1YFZI9UAtQU6hWxxKz3rdw7+d5u6hKzMATuQ9E2ty0dDWMBDNvgAtW/CICKGPHlfFyUDMNi8JL/mmP51s0/oBMsyZt5pPw7sitKbIUsfviZPpAwCB0MSynWinOA7uQQuwrh6Te9wdF4TM9WZ8I10UXN8CIIDPjCCAzowggIioAMCAQICEDvu4yb8gD8n1LTTTO1gLoswDQYJKoZIhvcNAQELBQAwMTELMAkGA1UEBhMCU0QxDDAKBgNVBAoMA1NUQzEUMBIGA1UEAwwLU1RDIFJvb3QgQ0EwHhcNMjYwMjI0MDAyMTQyWhcNMjcwMjE1MDAyMTQyWjCBgDEXMBUGA1UEAwwOTXkuQ29tcGFueS5jb20xFTATBgNVBAoMDE9yZ2FuaXphdGlvbjELMAkGA1UECwwCSVQxCzAJBgNVBAYTAlNEMREwDwYDVQQIDAhLaGFydG91bTERMA8GA1UEBwwIS2hhcnRvdW0xDjAMBgNVBAUTBTI0NjAwMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAseCmzh40/09eNQEFPqOseZP1+AE9V/1DXi7fm0+bIYBCMIerOG+3MFl7tQrzAypcz30owIUYzTw7eYRqJ6BUeZ+iTZeEmtPJzeeppCfnyMExQId5nOWaPV3bK9YvwYSQXhVur0H3/ga0tTeboImjCfuCLiXYFLuzWfYhVnM3ZHAW9T7xGvG1SMAOnM9WQBlBvrHiiOcJ+JYKo1L4Qg7vn6ONUVsM6f8hBhkIBm+YH/t4Na1OM49nVzQAHo1fXKUlAOguiEgO2QiuEGBRyGCSqfq0L3dSA2WqGmfBDdG6txXabLhl498xx2tviBFRXCKKgO9Hj8+LSrzepWv16+pywQIDAQABMA0GCSqGSIb3DQEBCwUAA4IBAQAxvE7LuKQ/zQulttilllYezq2t2VPMSOxrg3QyEZxTsvxicRnM1WVKsG3BAJhAQVDl9Ey0lOhRE6Z11rSMYPLyTOhjKqHkWXRdnSCwb24E+MmXU1BD7HDDV/4Y150xO7OsEJV+xprbkaZEdX9ULSbjeMTLvVdlSe49HZ0ykUVbJ+wKHDZXx0rtmLvi2e9EyihZvfoONhNvwUF6nkjrT8naMFyr1U+Ms4sKE9wD/kZwvQEggsVsUyqdmF2pfzM/s/n3/rNVTDF+67RDqtU2yn9YBjKYTNLy44zI6w9nnf/+kGPtcrGNowrKDkvBfo1GoUz4SAhIo0g9Ge7Bo5NM8Rv+",&[0u8;9],&[0u8;9]
        ).unwrap();
        // let mut file = fs::File::create("test_result.json").unwrap();
        // // Serialize the Vec<(u8, Vec<u8>)> as debug string for writing
        // let serialized = format!("{:?}", result);
        // file.write_all(serialized.as_bytes()).unwrap();
        // .expect("shit happens");
    }
}
