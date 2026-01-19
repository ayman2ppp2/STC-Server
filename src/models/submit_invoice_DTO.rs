use std::u8;

use base64::Engine;
use base64::engine::general_purpose;
use openssl::hash::{MessageDigest, hash};
use openssl::pkey::{PKey, Public};
use openssl::x509::X509;
use quick_xml::de::from_str;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use xml_c14n::{CanonicalizationMode, CanonicalizationOptions, canonicalize_xml};
use std::io::Cursor;
use xml_canonicalization::Canonicalizer;

use crate::models::invoice_model::Invoice;
#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct SubmitInvoiceDto {
    uuid: String,
    invoice_hash: String,
    invoice: String,
}

pub struct IntermediateInvoiceDto {
    pub invoice_bytes: Vec<u8>,
    pub invoice_hash: Vec<u8>,
    pub invoice_signature: Vec<u8>,
    pub certificate: X509,
    pub public_key: PKey<Public>,
}

impl SubmitInvoiceDto {
    pub fn parse(self) -> Result<IntermediateInvoiceDto, String> {
        let invoice_bytes = general_purpose::STANDARD
            .decode(self.invoice)
            .map_err(|_| "invalid Base64 invoice")?;

        let (signature, certificate) = extract_sig_crt(
            &String::from_utf8(invoice_bytes.clone())
                .map_err(|_| "failed to parse the invoice bytes")?,
        );
        let invoice_bytes = canonicalize(&invoice_bytes)?;
        let invoice_hash = general_purpose::STANDARD
            .decode(self.invoice_hash)
            .map_err(|_| "invalid invoice hash")?;
        let invoice_signature = general_purpose::STANDARD
            .decode(signature)
            .map_err(|_| "invalid base64 signature")?;

        let certificate = general_purpose::STANDARD
            .decode(certificate)
            .map_err(|_| "invalid Base64 invoice")?;

        let certificate = X509::from_der(&certificate).map_err(|_| "invalid x509 certificate")?;
        let public_key = certificate
            .public_key()
            .map_err(|_| "certificate has no public key")?;
        Ok(IntermediateInvoiceDto {
            invoice_bytes,
            invoice_hash,
            invoice_signature,
            certificate,
            public_key,
        })
    }
}

impl IntermediateInvoiceDto {
    pub fn parse_invoice(&self) -> Result<Invoice, String> {
        let xml_string = std::str::from_utf8(&self.invoice_bytes)
            .map_err(|_| "invoice xml is not valid utf8")?;
        let invoice: Invoice =
            from_str(xml_string).map_err(|e| format!("invalid invoice XML : {}", e))?;
        Ok(invoice)
    }
    pub fn compute_hash(&self) -> Result<Vec<u8>, openssl::error::ErrorStack> {
        let digest = hash(MessageDigest::sha256(), &self.invoice_bytes)?;
        print!("{:?}", digest);
        Ok(digest.to_vec())
    }
}

pub fn canonicalize(raw_xml: &[u8]) -> Result<Vec<u8>, String> {
    // Remove XML declaration if present
    let raw_xml = if raw_xml.starts_with(b"<?xml") {
        let start = raw_xml.iter().position(|&b| b == b'>').ok_or("Invalid XML header")? + 1;
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
                let name = local_name(e.name().local_name().as_ref())?;

                if skip_depth > 0 {
                    skip_depth += 1;
                } else if name == "UBLExtensions" || name == "Signature" {
                    skip_depth = 1;
                } else if name == "AdditionalDocumentReference" {
                    adr_depth = 1;
                    adr_has_qr = false;
                    in_adr_id = false;
                    adr_writer = Writer::new(Vec::new());
                    adr_writer
                        .write_event(Event::Start(e.to_owned()))
                        .map_err(|e| e.to_string())?;
                } else if adr_depth > 0 {
                    adr_depth += 1;
                    if name == "ID" {
                        in_adr_id = true;
                    }
                    adr_writer
                        .write_event(Event::Start(e.to_owned()))
                        .map_err(|e| e.to_string())?;
                } else {
                    writer
                        .write_event(Event::Start(e.to_owned()))
                        .map_err(|e| e.to_string())?;
                }
            }

            /* ---------- END ---------- */
            Ok(Event::End(e)) => {
                let name = local_name(e.name().local_name().as_ref())?;

                if skip_depth > 0 {
                    skip_depth -= 1;
                } else if adr_depth > 0 {
                    if name == "ID" {
                        in_adr_id = false;
                    }

                    adr_depth -= 1;
                    adr_writer
                        .write_event(Event::End(e.to_owned()))
                        .map_err(|e| e.to_string())?;

                    if adr_depth == 0 && !adr_has_qr {
                        let bytes = adr_writer.into_inner();
                        let mut r = Reader::from_reader(Cursor::new(bytes));
                        let mut b = Vec::new();

                        loop {
                            match r.read_event_into(&mut b) {
                                Ok(Event::Eof) => break,
                                Ok(ev) => writer.write_event(ev).map_err(|e| e.to_string())?,
                                Err(e) => return Err(format!("ADR replay error: {}", e)),
                            }
                            b.clear();
                        }

                        // Reinitialize adr_writer after consuming it
                        adr_writer = Writer::new(Vec::new());
                    }
                } else {
                    writer
                        .write_event(Event::End(e.to_owned()))
                        .map_err(|e| e.to_string())?;
                }
            }

            /* ---------- EMPTY ---------- */
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().local_name().as_ref())?;

                if skip_depth > 0 {
                    // skip
                } else if name == "UBLExtensions" || name == "Signature" {
                    // skip
                } else if adr_depth > 0 {
                    adr_writer
                        .write_event(Event::Empty(e.to_owned()))
                        .map_err(|e| e.to_string())?;
                } else {
                    writer
                        .write_event(Event::Empty(e.to_owned()))
                        .map_err(|e| e.to_string())?;
                }
            }

            /* ---------- TEXT ---------- */
            Ok(Event::Text(e)) => {
                let text = std::str::from_utf8(e.as_ref())
                    .map_err(|e| e.to_string())?
                    .trim();

                if adr_depth > 0 && in_adr_id && text == "QR" {
                    adr_has_qr = true;
                }

                if skip_depth == 0 {
                    if adr_depth > 0 {
                        adr_writer
                            .write_event(Event::Text(e.to_owned()))
                            .map_err(|e| e.to_string())?;
                    } else {
                        writer
                            .write_event(Event::Text(e.to_owned()))
                            .map_err(|e| e.to_string())?;
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
                            .map_err(|e| e.to_string())?;
                    } else {
                        writer
                            .write_event(ev.to_owned())
                            .map_err(|e| e.to_string())?;
                    }
                }
            }

            Err(e) => return Err(format!("XML parse error: {}", e)),
        }

        buf.clear();
    }

    let cleaned_xml = writer.into_inner();
    dbg!(String::from_utf8_lossy(&cleaned_xml));

    // /* ---------- C14N ---------- */
    // let xml_str = std::str::from_utf8(&cleaned_xml).map_err(|e| e.to_string())?;
    // let mut result = Vec::new();

    // Canonicalizer::read_from_str(xml_str)
    //     .write_to_writer(Cursor::new(&mut result))
    //     .canonicalize(false)
    //     .map_err(|e| e.to_string())?;
    // dbg!(String::from_utf8_lossy(&result));

    // Add debug print statements for both outputs
    // let xml_canonicalization_output = String::from_utf8_lossy(&result);
    // dbg!("xml-canonicalization output:", &xml_canonicalization_output);

    // // If using xml-c14n, add similar debug print
    // let xml_c14n_output = String::from_utf8_lossy(&cleaned_xml); // Replace with actual xml-c14n output if available
    // dbg!("xml-c14n output:", &xml_c14n_output);

    let options = CanonicalizationOptions {
        mode: CanonicalizationMode::Canonical1_1,
        keep_comments: false,
        inclusive_ns_prefixes: vec![],
    };
 let canonical = canonicalize_xml(
        std::str::from_utf8(&cleaned_xml).map_err(|e| e.to_string())?,
        options,
    ).map_err(|e| e.to_string())?;
    dbg!(&canonical);
    Ok(canonical.into_bytes())

    // Ok(result)
}

/* ---------- helper ---------- */
fn local_name(bytes: &[u8]) -> Result<String, String> {
    let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok(s.rsplit(':').next().unwrap().to_string())
}
fn extract_sig_crt(xml: &str) -> (String, String) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);

    let mut current = 0u8; // 0 = none, 1 = signature, 2 = certificate
    let mut signature = String::with_capacity(2048);
    let mut certificate = String::with_capacity(4096);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
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

    (signature, certificate)
}
