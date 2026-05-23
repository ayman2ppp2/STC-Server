use std::io::Cursor;

use anyhow::{Context, anyhow, bail};
use base64::{Engine, engine::general_purpose};
use openssl::{
    bn::BigNum,
    memcmp,
    x509::{X509, X509NameRef},
};
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, Event},
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::services::{
    crypto::pki_service::{compute_hash, verify_signature_with_cert},
    xml::{
        c14n11::canonicalize_c14n11,
        extractors::{extract_signed_info, extract_signed_properties},
    },
};

const C14N_11: &str = "http://www.w3.org/2006/12/xml-c14n11#";
const RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
const SHA256: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
const XADES_SIGNED_PROPERTIES: &str = "http://uri.etsi.org/01903#SignedProperties";
const XPATH_TRANSFORM: &str = "http://www.w3.org/TR/1999/REC-xpath-19991116";
const DS_NS: &str = "http://www.w3.org/2000/09/xmldsig#";
const XADES_NS: &str = "http://uri.etsi.org/01903/v1.3.2#";

#[derive(Debug, Default)]
struct SignatureProfile {
    signature_id: Option<String>,
    canonicalization_method: Option<String>,
    signature_method: Option<String>,
    signature_value: Option<Vec<u8>>,
    certificate: Option<X509>,
    qualifying_target: Option<String>,
    signed_properties_id: Option<String>,
    signing_time: Option<String>,
    signing_certificate_seen: bool,
    cert_digest_method: Option<String>,
    cert_digest_value: Option<Vec<u8>>,
    issuer_name: Option<String>,
    serial_number: Option<BigNum>,
    references: Vec<SignedReference>,
}

#[derive(Debug)]
struct SignedReference {
    uri: Option<String>,
    reference_type: Option<String>,
    digest_method: Option<String>,
    digest_value: Option<Vec<u8>>,
    transforms: Vec<String>,
}

impl SignedReference {
    fn new(e: &BytesStart<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            uri: attr_value(e, b"URI")?,
            reference_type: attr_value(e, b"Type")?,
            digest_method: None,
            digest_value: None,
            transforms: Vec::new(),
        })
    }
}

#[derive(Clone, Copy)]
enum TextField {
    SignatureValue,
    Certificate,
    ReferenceDigest,
    CertDigest,
    IssuerName,
    SerialNumber,
    SigningTime,
}

/// Validates the XMLDSig/XAdES-BES subset used by this service.
///
/// Supported profile is intentionally narrow: RSA-SHA256, SHA-256 digests,
/// and C14N 1.1. Unknown algorithms fail closed.
pub fn validate_xades_bes_signature(
    invoice_xml: &[u8],
    canonicalized_invoice: &[u8],
    received_invoice_hash: &[u8],
    expected_certificate: &X509,
) -> anyhow::Result<()> {
    let signature_xml = extract_single_signature(invoice_xml)?;
    let signed_info = extract_signed_info(&signature_xml, Some(DS_NS.as_bytes()))
        .context("failed to extract SignedInfo from signature")?;
    let signed_properties = extract_signed_properties(&signature_xml, Some(XADES_NS.as_bytes()))
        .context("failed to extract SignedProperties from signature")?;
    let profile = parse_signature_profile(&signature_xml)?;

    enforce_profile_structure(&profile)?;

    let invoice_hash = compute_hash(canonicalized_invoice)?;
    if !memcmp::eq(received_invoice_hash, &invoice_hash) {
        bail!("Invoice hash mismatch");
    }

    let invoice_ref = unique_reference(&profile, |r| r.uri.as_deref() == Some(""))
        .context("missing invoice reference")?;
    validate_reference_algorithms(invoice_ref)?;
    let invoice_ref_digest = invoice_ref
        .digest_value
        .as_ref()
        .context("invoice reference is missing DigestValue")?;
    if !memcmp::eq(invoice_ref_digest, &invoice_hash) {
        bail!("Signed invoice digest mismatch");
    }

    let signed_properties_id = profile
        .signed_properties_id
        .as_ref()
        .context("SignedProperties is missing Id")?;
    let signed_properties_uri = format!("#{signed_properties_id}");
    let signed_properties_ref = unique_reference(&profile, |r| {
        r.uri.as_deref() == Some(signed_properties_uri.as_str())
    })
    .context("missing SignedProperties reference")?;
    validate_reference_algorithms(signed_properties_ref)?;
    if signed_properties_ref.reference_type.as_deref() != Some(XADES_SIGNED_PROPERTIES) {
        bail!("SignedProperties reference has invalid Type");
    }

    let signed_properties_hash = compute_hash(&canonicalize_c14n11(signed_properties)?)?;
    let signed_properties_ref_digest = signed_properties_ref
        .digest_value
        .as_ref()
        .context("SignedProperties reference is missing DigestValue")?;
    if !memcmp::eq(signed_properties_ref_digest, &signed_properties_hash) {
        bail!("SignedProperties digest mismatch");
    }

    let certificate = profile
        .certificate
        .as_ref()
        .context("signature is missing X509Certificate")?;
    if certificate.to_der()? != expected_certificate.to_der()? {
        bail!("embedded signature certificate does not match parsed invoice certificate");
    }

    validate_certificate_binding(&profile, certificate)?;

    let signed_info_canonical = canonicalize_c14n11(signed_info)?;
    let signature_value = profile
        .signature_value
        .as_ref()
        .context("signature is missing SignatureValue")?;
    if !verify_signature_with_cert(&signed_info_canonical, signature_value, certificate)? {
        bail!("Invalid invoice signature");
    }

    Ok(())
}

fn enforce_profile_structure(profile: &SignatureProfile) -> anyhow::Result<()> {
    let signature_id = profile
        .signature_id
        .as_ref()
        .context("ds:Signature is missing Id")?;
    if profile.canonicalization_method.as_deref() != Some(C14N_11) {
        bail!("unsupported CanonicalizationMethod");
    }
    if profile.signature_method.as_deref() != Some(RSA_SHA256) {
        bail!("unsupported SignatureMethod");
    }
    let expected_target = format!("#{signature_id}");
    if profile.qualifying_target.as_deref() != Some(expected_target.as_str()) {
        bail!("QualifyingProperties Target does not match Signature Id");
    }
    if profile.signed_properties_id.is_none() {
        bail!("SignedProperties is missing Id");
    }
    let signing_time = profile.signing_time.as_deref().unwrap_or_default().trim();
    if signing_time.is_empty() {
        bail!("SignedSignatureProperties is missing SigningTime");
    }
    OffsetDateTime::parse(signing_time, &Rfc3339)
        .context("invalid SigningTime format, expected RFC 3339 dateTime")?;
    if !profile.signing_certificate_seen {
        bail!("SignedSignatureProperties is missing SigningCertificate");
    }
    if profile.references.len() != 2 {
        bail!(
            "SignedInfo must contain exactly 2 references, found {}",
            profile.references.len()
        );
    }
    let invoice_refs: Vec<_> = profile
        .references
        .iter()
        .filter(|r| r.uri.as_deref() == Some("") && r.reference_type.is_none())
        .collect();
    if invoice_refs.len() != 1 {
        bail!("SignedInfo must contain exactly one invoice reference (URI='')");
    }
    let signed_props_refs: Vec<_> = profile
        .references
        .iter()
        .filter(|r| {
            r.uri.as_deref() == Some("#xadesSignedProperties")
                && r.reference_type.as_deref() == Some(XADES_SIGNED_PROPERTIES)
        })
        .collect();
    if signed_props_refs.len() != 1 {
        bail!("SignedInfo must contain exactly one SignedProperties reference with correct Type");
    }
    Ok(())
}

fn validate_reference_algorithms(reference: &SignedReference) -> anyhow::Result<()> {
    if reference.digest_method.as_deref() != Some(SHA256) {
        bail!("unsupported Reference DigestMethod");
    }
    for transform in &reference.transforms {
        if transform != XPATH_TRANSFORM && transform != C14N_11 {
            bail!("unsupported Reference Transform");
        }
    }
    Ok(())
}

fn validate_certificate_binding(
    profile: &SignatureProfile,
    certificate: &X509,
) -> anyhow::Result<()> {
    if profile.cert_digest_method.as_deref() != Some(SHA256) {
        bail!("unsupported certificate DigestMethod");
    }
    let cert_der = certificate.to_der()?;
    let cert_hash = compute_hash(&cert_der)?;
    let cert_digest = profile
        .cert_digest_value
        .as_ref()
        .context("SigningCertificate is missing CertDigest DigestValue")?;
    if !memcmp::eq(cert_digest, &cert_hash) {
        bail!("SigningCertificate CertDigest mismatch");
    }

    let expected_serial = profile
        .serial_number
        .as_ref()
        .context("SigningCertificate is missing issuer serial")?;
    let actual_serial = certificate.serial_number().to_bn()?;
    if actual_serial != *expected_serial {
        bail!("SigningCertificate serial mismatch");
    }

    let issuer_name = profile
        .issuer_name
        .as_deref()
        .context("SigningCertificate is missing issuer name")?;
    if issuer_name.trim().is_empty() {
        bail!("SigningCertificate is missing issuer name");
    }
    validate_issuer_name(certificate.issuer_name(), issuer_name)?;

    Ok(())
}

fn validate_issuer_name(actual: &X509NameRef, expected: &str) -> anyhow::Result<()> {
    let actual_parts = actual
        .entries()
        .map(|entry| {
            let name = entry
                .object()
                .nid()
                .short_name()
                .unwrap_or("UNKNOWN")
                .to_owned();
            let value = entry
                .data()
                .as_utf8()
                .map(|value| value.to_string())
                .unwrap_or_default();
            format!("{name}={value}")
        })
        .collect::<Vec<_>>();

    for expected_part in expected
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if !actual_parts
            .iter()
            .any(|actual_part| actual_part == expected_part)
        {
            bail!("SigningCertificate issuer name mismatch");
        }
    }

    Ok(())
}

fn unique_reference<F>(profile: &SignatureProfile, predicate: F) -> anyhow::Result<&SignedReference>
where
    F: Fn(&SignedReference) -> bool,
{
    let mut matches = profile.references.iter().filter(|r| predicate(r));
    let reference = matches.next().context("reference not found")?;
    if matches.next().is_some() {
        bail!("duplicate matching SignedInfo reference");
    }
    Ok(reference)
}

fn parse_signature_profile(signature_xml: &[u8]) -> anyhow::Result<SignatureProfile> {
    let mut reader = Reader::from_reader(Cursor::new(signature_xml));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut profile = SignatureProfile::default();
    let mut in_signed_info = false;
    let mut in_cert_digest = false;
    let mut current_reference: Option<SignedReference> = None;
    let mut text_field: Option<TextField> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"ds:Signature" => set_once(
                        &mut profile.signature_id,
                        attr_value(&e, b"Id")?,
                        "Signature Id",
                    )?,
                    b"ds:SignedInfo" => in_signed_info = true,
                    b"ds:CanonicalizationMethod" if in_signed_info => {
                        set_once(
                            &mut profile.canonicalization_method,
                            attr_value(&e, b"Algorithm")?,
                            "CanonicalizationMethod",
                        )?;
                    }
                    b"ds:SignatureMethod" if in_signed_info => {
                        set_once(
                            &mut profile.signature_method,
                            attr_value(&e, b"Algorithm")?,
                            "SignatureMethod",
                        )?;
                    }
                    b"ds:Reference" if in_signed_info => {
                        if current_reference.is_some() {
                            bail!("nested Reference is not supported");
                        }
                        current_reference = Some(SignedReference::new(&e)?);
                    }
                    b"ds:Transform" if in_signed_info => {
                        if let Some(reference) = current_reference.as_mut() {
                            let algorithm = required_attr(&e, b"Algorithm", "Transform Algorithm")?;
                            reference.transforms.push(algorithm);
                        }
                    }
                    b"ds:DigestMethod" if in_signed_info => {
                        if let Some(reference) = current_reference.as_mut() {
                            set_once(
                                &mut reference.digest_method,
                                attr_value(&e, b"Algorithm")?,
                                "Reference DigestMethod",
                            )?;
                        }
                    }
                    b"ds:DigestValue" if in_signed_info && current_reference.is_some() => {
                        text_field = Some(TextField::ReferenceDigest);
                    }
                    b"ds:SignatureValue" => text_field = Some(TextField::SignatureValue),
                    b"ds:X509Certificate" => text_field = Some(TextField::Certificate),
                    b"xades:QualifyingProperties" => {
                        set_once(
                            &mut profile.qualifying_target,
                            attr_value(&e, b"Target")?,
                            "QualifyingProperties Target",
                        )?;
                    }
                    b"xades:SignedProperties" => {
                        set_once(
                            &mut profile.signed_properties_id,
                            attr_value(&e, b"Id")?,
                            "SignedProperties Id",
                        )?;
                    }
                    b"xades:SigningTime" => text_field = Some(TextField::SigningTime),
                    b"xades:SigningCertificate" => profile.signing_certificate_seen = true,
                    b"xades:CertDigest" => in_cert_digest = true,
                    b"ds:DigestMethod" if in_cert_digest => {
                        set_once(
                            &mut profile.cert_digest_method,
                            attr_value(&e, b"Algorithm")?,
                            "certificate DigestMethod",
                        )?;
                    }
                    b"ds:DigestValue" if in_cert_digest => text_field = Some(TextField::CertDigest),
                    b"ds:X509IssuerName" => text_field = Some(TextField::IssuerName),
                    b"ds:X509SerialNumber" => text_field = Some(TextField::SerialNumber),
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"ds:CanonicalizationMethod" if in_signed_info => {
                        set_once(
                            &mut profile.canonicalization_method,
                            attr_value(&e, b"Algorithm")?,
                            "CanonicalizationMethod",
                        )?;
                    }
                    b"ds:SignatureMethod" if in_signed_info => {
                        set_once(
                            &mut profile.signature_method,
                            attr_value(&e, b"Algorithm")?,
                            "SignatureMethod",
                        )?;
                    }
                    b"ds:Transform" if in_signed_info => {
                        if let Some(reference) = current_reference.as_mut() {
                            let algorithm = required_attr(&e, b"Algorithm", "Transform Algorithm")?;
                            reference.transforms.push(algorithm);
                        }
                    }
                    b"ds:DigestMethod" if in_signed_info => {
                        if let Some(reference) = current_reference.as_mut() {
                            set_once(
                                &mut reference.digest_method,
                                attr_value(&e, b"Algorithm")?,
                                "Reference DigestMethod",
                            )?;
                        }
                    }
                    b"ds:DigestMethod" if in_cert_digest => {
                        set_once(
                            &mut profile.cert_digest_method,
                            attr_value(&e, b"Algorithm")?,
                            "certificate DigestMethod",
                        )?;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(field) = text_field {
                    let text = e.decode().context("failed to decode signature XML text")?;
                    apply_text_field(&mut profile, current_reference.as_mut(), field, text.trim())?;
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"ds:SignedInfo" => in_signed_info = false,
                b"ds:Reference" if in_signed_info => {
                    let reference = current_reference
                        .take()
                        .context("Reference ended without state")?;
                    profile.references.push(reference);
                }
                b"xades:CertDigest" => in_cert_digest = false,
                b"ds:SignatureValue"
                | b"ds:X509Certificate"
                | b"ds:DigestValue"
                | b"ds:X509IssuerName"
                | b"ds:X509SerialNumber"
                | b"xades:SigningTime" => text_field = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => bail!("signature XML error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(profile)
}

fn apply_text_field(
    profile: &mut SignatureProfile,
    current_reference: Option<&mut SignedReference>,
    field: TextField,
    text: &str,
) -> anyhow::Result<()> {
    match field {
        TextField::SignatureValue => set_once(
            &mut profile.signature_value,
            Some(
                general_purpose::STANDARD
                    .decode(text)
                    .context("invalid SignatureValue base64")?,
            ),
            "SignatureValue",
        ),
        TextField::Certificate => {
            let der = general_purpose::STANDARD
                .decode(text)
                .context("invalid X509Certificate base64")?;
            let certificate = X509::from_der(&der).context("invalid X509Certificate DER")?;
            set_once(
                &mut profile.certificate,
                Some(certificate),
                "X509Certificate",
            )
        }
        TextField::ReferenceDigest => {
            let reference = current_reference.context("DigestValue outside Reference")?;
            set_once(
                &mut reference.digest_value,
                Some(
                    general_purpose::STANDARD
                        .decode(text)
                        .context("invalid Reference DigestValue base64")?,
                ),
                "Reference DigestValue",
            )
        }
        TextField::CertDigest => set_once(
            &mut profile.cert_digest_value,
            Some(
                general_purpose::STANDARD
                    .decode(text)
                    .context("invalid CertDigest DigestValue base64")?,
            ),
            "CertDigest DigestValue",
        ),
        TextField::IssuerName => set_once(
            &mut profile.issuer_name,
            Some(text.to_owned()),
            "X509IssuerName",
        ),
        TextField::SerialNumber => {
            let serial = BigNum::from_dec_str(text).context("invalid X509SerialNumber")?;
            set_once(&mut profile.serial_number, Some(serial), "X509SerialNumber")
        }
        TextField::SigningTime => set_once(
            &mut profile.signing_time,
            Some(text.to_owned()),
            "SigningTime",
        ),
    }
}

fn extract_single_signature(xml: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut signatures = Vec::new();
    let mut writer: Option<Writer<Vec<u8>>> = None;
    let mut depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if writer.is_none() && e.local_name().as_ref() == b"Signature" {
                    let mut w = Writer::new(Vec::new());
                    w.write_event(Event::Start(e.to_owned()))?;
                    writer = Some(w);
                    depth = 1;
                } else if let Some(w) = writer.as_mut() {
                    depth += 1;
                    w.write_event(Event::Start(e.to_owned()))?;
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(w) = writer.as_mut() {
                    w.write_event(Event::Empty(e.to_owned()))?;
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(w) = writer.as_mut() {
                    w.write_event(Event::Text(e.to_owned()))?;
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(w) = writer.as_mut() {
                    w.write_event(Event::CData(e.to_owned()))?;
                }
            }
            Ok(Event::Comment(e)) => {
                if let Some(w) = writer.as_mut() {
                    w.write_event(Event::Comment(e.to_owned()))?;
                }
            }
            Ok(Event::End(e)) => {
                if let Some(mut w) = writer.take() {
                    w.write_event(Event::End(e.to_owned()))?;
                    depth -= 1;
                    if depth == 0 {
                        let candidate = w.into_inner();
                        if contains_signed_info(&candidate)? {
                            signatures.push(candidate);
                        }
                    } else {
                        writer = Some(w);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("XML error while extracting Signature: {e}"),
            _ => {}
        }
        buf.clear();
    }

    match signatures.len() {
        1 => Ok(signatures.remove(0)),
        0 => Err(anyhow!("ds:Signature not found")),
        _ => Err(anyhow!("multiple ds:Signature elements found")),
    }
}

fn contains_signed_info(xml: &[u8]) -> anyhow::Result<bool> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"ds:SignedInfo" {
                    return Ok(true);
                }
            }
            Ok(Event::Eof) => return Ok(false),
            Err(e) => bail!("XML error while checking Signature: {e}"),
            _ => {}
        }
        buf.clear();
    }
}

fn required_attr(e: &BytesStart<'_>, key: &[u8], name: &str) -> anyhow::Result<String> {
    attr_value(e, key)?.with_context(|| format!("missing {name}"))
}

fn attr_value(e: &BytesStart<'_>, key: &[u8]) -> anyhow::Result<Option<String>> {
    for attr in e.attributes() {
        let attr = attr.context("invalid XML attribute")?;
        if attr.key.as_ref() == key {
            return Ok(Some(std::str::from_utf8(attr.value.as_ref())?.to_owned()));
        }
    }
    Ok(None)
}

fn set_once<T>(slot: &mut Option<T>, value: Option<T>, name: &str) -> anyhow::Result<()> {
    let value = value.with_context(|| format!("missing {name}"))?;
    if slot.is_some() {
        bail!("duplicate {name}");
    }
    *slot = Some(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::xml::extractors::{extract_invoice, extract_sig_crt};
    use std::fs;

    fn valid_profile() -> SignatureProfile {
        SignatureProfile {
            signature_id: Some("sig".to_owned()),
            canonicalization_method: Some(C14N_11.to_owned()),
            signature_method: Some(RSA_SHA256.to_owned()),
            signature_value: None,
            certificate: None,
            qualifying_target: Some("#sig".to_owned()),
            signed_properties_id: Some("xadesSignedProperties".to_owned()),
            signing_time: Some("2026-03-18T20:08:12Z".to_owned()),
            signing_certificate_seen: true,
            cert_digest_method: None,
            cert_digest_value: None,
            issuer_name: None,
            serial_number: None,
            references: vec![
                SignedReference {
                    uri: Some(String::new()),
                    reference_type: None,
                    digest_method: None,
                    digest_value: None,
                    transforms: vec![],
                },
                SignedReference {
                    uri: Some("#xadesSignedProperties".to_owned()),
                    reference_type: Some(XADES_SIGNED_PROPERTIES.to_owned()),
                    digest_method: None,
                    digest_value: None,
                    transforms: vec![],
                },
            ],
        }
    }

    #[test]
    fn duplicate_signature_blocks_are_rejected() {
        let xml = br#"<Root><ds:Signature Id="signature"><ds:SignedInfo/></ds:Signature><ds:Signature Id="signature2"><ds:SignedInfo/></ds:Signature></Root>"#;
        let err = extract_single_signature(xml).unwrap_err().to_string();
        assert!(err.contains("multiple ds:Signature"));
    }

    #[test]
    fn unsupported_reference_transform_is_rejected() {
        let reference = SignedReference {
            uri: Some(String::new()),
            reference_type: None,
            digest_method: Some(SHA256.to_owned()),
            digest_value: Some(vec![0; 32]),
            transforms: vec!["http://example.com/unsupported".to_owned()],
        };
        assert!(validate_reference_algorithms(&reference).is_err());
    }

    #[test]
    fn test_invoice_fixture_rejects_rsa_signature_mismatch() {
        let xml = fs::read("test.xml").expect("failed to read test.xml");
        let canonicalized_invoice = canonicalize_c14n11(extract_invoice(&xml).unwrap()).unwrap();
        let invoice_hash = compute_hash(&canonicalized_invoice).unwrap();
        let (_, certificate_b64) = extract_sig_crt(&xml).unwrap();
        let certificate_der = general_purpose::STANDARD.decode(certificate_b64).unwrap();
        let certificate = X509::from_der(&certificate_der).unwrap();
        let err =
            validate_xades_bes_signature(&xml, &canonicalized_invoice, &invoice_hash, &certificate)
                .unwrap_err()
                .to_string();
        assert!(err.contains("Invalid invoice signature"));
    }

    // ── enforce_profile_structure negative tests ──

    #[test]
    fn missing_signing_time_rejected() {
        let mut profile = valid_profile();
        profile.signing_time = None;
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("SigningTime"));
    }

    #[test]
    fn invalid_signing_time_format_rejected() {
        let mut profile = valid_profile();
        profile.signing_time = Some("not-a-date".to_owned());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("SigningTime") || err.contains("RFC"));
    }

    #[test]
    fn extra_reference_rejected() {
        let mut profile = valid_profile();
        profile.references.push(SignedReference {
            uri: Some("extra".to_owned()),
            reference_type: None,
            digest_method: None,
            digest_value: None,
            transforms: vec![],
        });
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("exactly 2"));
    }

    #[test]
    fn only_one_reference_rejected() {
        let mut profile = valid_profile();
        profile.references.pop();
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("exactly 2"));
    }

    #[test]
    fn wrong_invoice_reference_uri_rejected() {
        let mut profile = valid_profile();
        profile.references[0].uri = Some("https://example.com".to_owned());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("invoice reference"));
    }

    #[test]
    fn wrong_signed_properties_reference_type_rejected() {
        let mut profile = valid_profile();
        profile.references[1].reference_type = Some("http://wrong.type".to_owned());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("SignedProperties reference"));
    }

    #[test]
    fn wrong_signed_properties_reference_uri_rejected() {
        let mut profile = valid_profile();
        profile.references[1].uri = Some("#wrongId".to_owned());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("SignedProperties reference"));
    }

    #[test]
    fn invoice_ref_with_type_rejected() {
        let mut profile = valid_profile();
        profile.references[0].reference_type = Some("http://some.type".to_owned());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("invoice reference"));
    }

    #[test]
    fn qualified_invoice_ref_rejected() {
        let mut profile = valid_profile();
        profile.references[0].uri = Some("#invoice".to_owned());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("invoice reference"));
    }

    // ── parse_signature_profile negative tests ──

    #[test]
    fn wrong_prefix_on_signature_rejected() {
        let xml = br#"<foo:Signature xmlns:foo="http://wrong" Id="sig">
            <ds:SignedInfo>
                <ds:CanonicalizationMethod Algorithm="http://www.w3.org/2006/12/xml-c14n11#"/>
                <ds:SignatureMethod Algorithm="http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"/>
            </ds:SignedInfo>
        </foo:Signature>"#;
        let sig = extract_single_signature(xml).unwrap();
        let profile = parse_signature_profile(&sig).unwrap();
        assert!(profile.signature_id.is_none());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("Signature") && err.contains("Id"));
    }

    #[test]
    fn wrong_prefix_on_signed_properties_rejected() {
        let xml = br##"<ds:Signature Id="sig">
            <ds:SignedInfo>
                <ds:CanonicalizationMethod Algorithm="http://www.w3.org/2006/12/xml-c14n11#"/>
                <ds:SignatureMethod Algorithm="http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"/>
                <ds:Reference URI=""></ds:Reference>
                <ds:Reference URI="#xadesSignedProperties" Type="http://uri.etsi.org/01903#SignedProperties"></ds:Reference>
            </ds:SignedInfo>
            <xades:QualifyingProperties Target="#sig">
                <foo:SignedProperties xmlns:foo="http://wrong" Id="xadesSignedProperties"></foo:SignedProperties>
            </xades:QualifyingProperties>
        </ds:Signature>"##;
        let profile = parse_signature_profile(xml).unwrap();
        assert!(profile.signed_properties_id.is_none());
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("SignedProperties") || err.contains("signed"));
    }

    #[test]
    fn duplicate_signed_properties_rejected() {
        let xml = br##"<ds:Signature Id="sig">
            <xades:QualifyingProperties Target="#sig">
                <xades:SignedProperties Id="sp1"></xades:SignedProperties>
                <xades:SignedProperties Id="sp2"></xades:SignedProperties>
            </xades:QualifyingProperties>
        </ds:Signature>"##;
        let err = parse_signature_profile(xml).unwrap_err().to_string();
        assert!(err.contains("duplicate") || err.contains("SignedProperties"));
    }

    #[test]
    fn wrong_prefix_on_signing_time_ignored() {
        let xml = br##"<ds:Signature Id="sig">
            <xades:QualifyingProperties Target="#sig">
                <xades:SignedProperties Id="xadesSignedProperties">
                    <xades:SignedSignatureProperties>
                        <foo:SigningTime xmlns:foo="http://wrong">2026-03-18T20:08:12Z</foo:SigningTime>
                    </xades:SignedSignatureProperties>
                </xades:SignedProperties>
            </xades:QualifyingProperties>
        </ds:Signature>"##;
        let profile = parse_signature_profile(xml).unwrap();
        assert!(profile.signing_time.is_none());
    }

    #[test]
    fn extra_reference_in_xml_rejected() {
        let xml = br##"<ds:Signature Id="sig">
            <ds:SignedInfo>
                <ds:CanonicalizationMethod Algorithm="http://www.w3.org/2006/12/xml-c14n11#"/>
                <ds:SignatureMethod Algorithm="http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"/>
                <ds:Reference URI=""></ds:Reference>
                <ds:Reference URI="#xadesSignedProperties" Type="http://uri.etsi.org/01903#SignedProperties"></ds:Reference>
                <ds:Reference URI="#extra"></ds:Reference>
            </ds:SignedInfo>
            <xades:QualifyingProperties Target="#sig">
                <xades:SignedProperties Id="xadesSignedProperties">
                    <xades:SignedSignatureProperties>
                        <xades:SigningTime>2026-03-18T20:08:12Z</xades:SigningTime>
                        <xades:SigningCertificate></xades:SigningCertificate>
                    </xades:SignedSignatureProperties>
                </xades:SignedProperties>
            </xades:QualifyingProperties>
        </ds:Signature>"##;
        let profile = parse_signature_profile(xml).unwrap();
        assert_eq!(profile.references.len(), 3);
        let err = enforce_profile_structure(&profile).unwrap_err().to_string();
        assert!(err.contains("exactly 2"));
    }

    // ── extract / round-trip negative tests ──

    #[test]
    fn wrong_signature_namespace_rejected_by_extract_single() {
        let xml = br#"<ds:Signature Id="sig"><ds:SignedInfo/></ds:Signature>"#;
        assert!(extract_single_signature(xml).is_ok());

        let xml =
            br#"<foo:Signature xmlns:foo="http://wrong" Id="sig"><ds:SignedInfo/></foo:Signature>"#;
        assert!(extract_single_signature(xml).is_ok());
    }

    #[test]
    fn namespace_aware_extract_signed_info() {
        let valid = br"<ds:SignedInfo><ds:CanonicalizationMethod Algorithm='http://www.w3.org/2006/12/xml-c14n11#'/></ds:SignedInfo>";
        assert!(extract_signed_info(valid, Some(DS_NS.as_bytes())).is_ok());

        let invalid = br"<foo:SignedInfo xmlns:foo='http://wrong'><ds:CanonicalizationMethod Algorithm='http://www.w3.org/2006/12/xml-c14n11#'/></foo:SignedInfo>";
        assert!(extract_signed_info(invalid, Some(DS_NS.as_bytes())).is_err());
    }

    #[test]
    fn namespace_aware_extract_signed_properties() {
        let valid = br"<xades:SignedProperties xmlns:xades='http://uri.etsi.org/01903/v1.3.2#' Id='sp'><xades:SignedSignatureProperties/></xades:SignedProperties>";
        assert!(extract_signed_properties(valid, Some(XADES_NS.as_bytes())).is_ok());

        let invalid = br"<foo:SignedProperties xmlns:foo='http://wrong' Id='sp'><xades:SignedSignatureProperties/></foo:SignedProperties>";
        assert!(extract_signed_properties(invalid, Some(XADES_NS.as_bytes())).is_err());
    }
}
