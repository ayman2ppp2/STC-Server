use std::io::Cursor;
use crate::services::c14n11::canonicalize_c14n11;
use anyhow:: anyhow;
use quick_xml::{Reader, Writer, events::Event};
pub fn extract_invoice(raw_xml: &[u8]) -> anyhow::Result<Vec<u8>> {
    // Remove XML declaration if present
    let raw_xml = if raw_xml.starts_with(b"<?xml") {
        let start = raw_xml
            .iter()
            .position(|&b| b == b'>')
            .ok_or(anyhow!("Invalid XML header"))?
            + 1;
        &raw_xml[start..]
    } else {
        raw_xml
    };

    let mut reader = Reader::from_reader(Cursor::new(raw_xml));
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    let mut skip_depth = 0usize;

    let mut adr_depth = 0usize;
    let mut adr_has_qr = false;
    let mut in_adr_id = false;
    let mut adr_writer = Writer::new(Vec::new());

    loop {
        match reader.read_event_into(&mut buf) {
            /* ---------- START ---------- */
            Ok(Event::Start(e)) => {


                if skip_depth > 0 {
                    skip_depth += 1;
                } else if e.local_name().as_ref() == b"UBLExtensions" || e.local_name().as_ref() == b"Signature" {
                    skip_depth = 1;
                } else if e.local_name().as_ref() == b"AdditionalDocumentReference" {
                    adr_depth = 1;
                    adr_has_qr = false;
                    in_adr_id = false;
                    adr_writer = Writer::new(Vec::new());
                    adr_writer
                        .write_event(Event::Start(e.to_owned()))
                        ?;
                } else if adr_depth > 0 {
                    adr_depth += 1;
                    if e.local_name().as_ref() == b"bID" {
                        in_adr_id = true;
                    }
                    adr_writer
                        .write_event(Event::Start(e.to_owned()))
                         ?;
                } else {
                    writer
                        .write_event(Event::Start(e.to_owned()))
                         ?;
                }
            }

            /* ---------- END ---------- */
            Ok(Event::End(e)) => {


                if skip_depth > 0 {
                    skip_depth -= 1;
                } else if adr_depth > 0 {
                    if e.local_name().as_ref() == b"ID" {
                        in_adr_id = false;
                    }

                    adr_depth -= 1;
                    adr_writer
                        .write_event(Event::End(e.to_owned()))
                         ?;

                    if adr_depth == 0 && !adr_has_qr {
                        let bytes = adr_writer.into_inner();
                        let mut r = Reader::from_reader(Cursor::new(bytes));
                        let mut b = Vec::new();

                        loop {
                            match r.read_event_into(&mut b) {
                                Ok(Event::Eof) => break,
                                Ok(ev) => writer.write_event(ev) ?,
                                Err(e) => return Err( e.into()),
                            }
                            b.clear();
                        }

                        // Reinitialize adr_writer after consuming it
                        adr_writer = Writer::new(Vec::new());
                    }
                } else {
                    writer
                        .write_event(Event::End(e.to_owned()))
                         ?;
                }
            }

            /* ---------- EMPTY ---------- */
            Ok(Event::Empty(e)) => {


                if skip_depth > 0 {
                    // skip
                } else if e.local_name().as_ref() == b"UBLExtensions" || e.local_name().as_ref() == b"Signature" {
                    // skip
                } else if adr_depth > 0 {
                    adr_writer
                        .write_event(Event::Empty(e.to_owned()))
                         ?;
                } else {
                    writer
                        .write_event(Event::Empty(e.to_owned()))
                         ?;
                }
            }

            /* ---------- TEXT ---------- */
            Ok(Event::Text(e)) => {
                let text = std::str::from_utf8(e.as_ref())
                     ?
                    .trim();

                if adr_depth > 0 && in_adr_id && text == "QR" {
                    adr_has_qr = true;
                }

                if skip_depth == 0 {
                    if adr_depth > 0 {
                        adr_writer
                            .write_event(Event::Text(e.to_owned()))
                             ?;
                    } else {
                        writer
                            .write_event(Event::Text(e.to_owned()))
                             ?;
                    }
                }
            }

            /* ---------- OTHER ---------- */
            Ok(Event::Eof) => break,

            Ok(ev) => {
                if skip_depth == 0 {
                    if adr_depth > 0 {
                        adr_writer
                            .write_event(ev.to_owned())
                             ?;
                    } else {
                        writer
                            .write_event(ev.to_owned())
                             ?;
                    }
                }
            }

            Err(e) => return Err(e.into()),
        }

        buf.clear();
    }

    let cleaned_xml = writer.into_inner();
    Ok(cleaned_xml)
}
pub fn extract_sig_crt(xml: &Vec<u8>) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);

    let mut current = 0u8; // 0 = none, 1 = signature, 2 = certificate
    let mut signature = String::with_capacity(2048);
    let mut certificate = String::with_capacity(4096);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"ds:SignatureValue" => current = 1,
                b"ds:X509Certificate" => current = 2,
                _ => {}
            },

            Ok(Event::Text(e)) => {
                let text = e.decode().expect("failed to decode the xml");
                match current {
                    1 => signature.push_str(&text),
                    2 => certificate.push_str(&text),
                    _ => {}
                }
            }

            Ok(Event::End(_)) => current = 0,
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok((signature.into(), certificate.into()))
}
pub fn extract_signed_properties(xml: &Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    let mut capturing = false;
    let mut depth: usize = 0;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                if e.local_name().as_ref()== b"SignedProperties" {
                    capturing = true;
                    depth = 1;
                } else if capturing {
                    depth += 1;
                }

                if capturing {
                    writer.write_event(Event::Start(e.to_owned()))?;
                }
            }

            Event::Empty(e) => {
                if capturing {
                    writer.write_event(Event::Empty(e.to_owned()))?;
                }
            }

            Event::Text(e) => {
                if capturing {
                    writer.write_event(Event::Text(e.to_owned()))?;
                }
            }

            Event::CData(e) => {
                if capturing {
                    writer.write_event(Event::CData(e.to_owned()))?;
                }
            }

            Event::End(e) => {
                if capturing {
                    writer.write_event(Event::End(e.to_owned()))?;
                    depth -= 1;

                    if depth == 0 {
                        break;
                    }
                }
            }

            Event::Eof => break,
            _ => {}
        }

        buf.clear();
    }

    Ok(writer.into_inner().into_inner())
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_extract_signed_properties() {
        let xml = br#"<xades:QualifyingProperties xmlns:xades="http://uri.etsi.org/01903/v1.3.2#" Target="signature">
                                <xades:SignedProperties Id="xadesSignedProperties">
                                    <xades:SignedSignatureProperties>
                                        <xades:SigningTime>109384180981</xades:SigningTime> //the signging time (will change)
                                        <xades:SigningCertificate>
                                            <xades:Cert>
                                                <xades:CertDigest>
                                                    <ds:DigestMethod Algorithm="http://www.w3.org/2001/04/xmlenc#sha256"/>
                                                    <ds:DigestValue>ZDMwMmI0MTE1NzVjOTU2NTk4YzVlODhhYmI0ODU2NDUyNTU2YTVhYjhhMDFmN2FjYjk1YTA2OWQ0NjY2MjQ4NQ==</ds:DigestValue> cer hash (no change)
                                                </xades:CertDigest>
                                                <xades:IssuerSerial>
                                                    <ds:X509IssuerName>CN=PRZEINVOICESCA4-CA, DC=extgazt, DC=gov, DC=local</ds:X509IssuerName>
                                                    <ds:X509SerialNumber>379112742831380471835263969587287663520528387</ds:X509SerialNumber>
                                                </xades:IssuerSerial>
                                            </xades:Cert>
                                        </xades:SigningCertificate>
                                    </xades:SignedSignatureProperties>
                                </xades:SignedProperties>
                            </xades:QualifyingProperties>"#;

        assert_eq!(extract_signed_properties(&xml.to_vec()).unwrap(),br#"<xades:SignedProperties Id="xadesSignedProperties">
                                    <xades:SignedSignatureProperties>
                                        <xades:SigningTime>109384180981</xades:SigningTime> //the signging time (will change)
                                        <xades:SigningCertificate>
                                            <xades:Cert>
                                                <xades:CertDigest>
                                                    <ds:DigestMethod Algorithm="http://www.w3.org/2001/04/xmlenc#sha256"/>
                                                    <ds:DigestValue>ZDMwMmI0MTE1NzVjOTU2NTk4YzVlODhhYmI0ODU2NDUyNTU2YTVhYjhhMDFmN2FjYjk1YTA2OWQ0NjY2MjQ4NQ==</ds:DigestValue> cer hash (no change)
                                                </xades:CertDigest>
                                                <xades:IssuerSerial>
                                                    <ds:X509IssuerName>CN=PRZEINVOICESCA4-CA, DC=extgazt, DC=gov, DC=local</ds:X509IssuerName>
                                                    <ds:X509SerialNumber>379112742831380471835263969587287663520528387</ds:X509SerialNumber>
                                                </xades:IssuerSerial>
                                            </xades:Cert>
                                        </xades:SigningCertificate>
                                    </xades:SignedSignatureProperties>
                                </xades:SignedProperties>"#);
    }
}
