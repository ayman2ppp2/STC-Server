use std::fmt::format;
use std::u8;

use base64::Engine;
use base64::engine::general_purpose;
use openssl::hash::{MessageDigest, hash};
use openssl::pkey::{PKey, Public};
use openssl::x509::X509;
use quick_xml::Reader;
use quick_xml::de::from_str;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use xml_c14n::{CanonicalizationMode, CanonicalizationOptions, canonicalize_xml};

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
            .map_err(|_| "invalid invioce hash")?;
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

use libxml::parser::Parser;
use libxml::xpath::Context;

pub fn canonicalize(raw_xml: &[u8]) -> Result<Vec<u8>, String> {
    /* -------------------------------------------------
     * 1. Parse raw XML
     * ------------------------------------------------- */
    let xml_str = std::str::from_utf8(raw_xml).map_err(|e| format!("invalid UTF-8 XML: {}", e))?;

    let parser = Parser::default();
    let mut doc = parser
        .parse_string(xml_str)
        .map_err(|e| format!("XML parse error: {}", e))?;

    /* -------------------------------------------------
     * 2. Remove ZATCA-excluded nodes (XPath)
     * ------------------------------------------------- */
    let mut ctx = Context::new(&doc).map_err(|e| format!("XPath context error: {:?}", e))?;

    let xpaths = [
        "//*[local-name()='Invoice']//*[local-name()='UBLExtensions']",
        "//*[local-name()='Invoice']//*[local-name()='Signature']",
        "//*[local-name()='AdditionalDocumentReference']
          [*[local-name()='ID' and normalize-space(text())='QR']]",
    ];

    for xp in xpaths {
        let nodes = ctx
            .evaluate(xp)
            .map_err(|e| format!("XPath eval error: {:?}", e))?;

        for mut node in nodes.get_nodes_as_vec() {
            node.unlink();
        }
    }

    /* -------------------------------------------------
     * 3. Serialize WITHOUT XML declaration
     * ------------------------------------------------- */
    let cleaned_xml = doc.to_string(); // libxml omits XML declaration

    /* -------------------------------------------------
     * 4. Canonicalize (C14N 1.1 – ZATCA requirement)
     * ------------------------------------------------- */
    let options = CanonicalizationOptions {
        mode: CanonicalizationMode::Canonical1_1,
        keep_comments: false,
        inclusive_ns_prefixes: vec![],
    };

    let canonical = canonicalize_xml(&cleaned_xml, options)
        .map_err(|e| format!("canonicalization error: {}", e))?;

    Ok(canonical.into_bytes())
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
