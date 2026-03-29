use anyhow::{Context, bail};
use quick_xml::{events::Event, Reader, Writer};
use std::io::Cursor;

#[derive(PartialEq)]
enum State {
    Default,
    Skipping,
    InAdditionalDocRef,
    InAdditionalDocRefId,
}

pub fn extract_invoice(raw_xml: &[u8]) -> anyhow::Result<Vec<u8>> {
    // 1. Remove XML declaration if present
    let xml_to_parse = if raw_xml.starts_with(b"<?xml") {
        let pos = raw_xml
            .iter()
            .position(|&b| b == b'>')
            .ok_or_else(|| anyhow::anyhow!("Invalid XML"))?;
        &raw_xml[pos + 1..]
    } else {
        raw_xml
    };

    let mut reader = Reader::from_reader(Cursor::new(xml_to_parse));
    let mut writer = Writer::new(Vec::new());
    let mut adr_buffer: Vec<u8> = Vec::new();

    let mut state = State::Default;
    let mut skip_depth = 0;
    let mut adr_depth = 0;
    let mut is_qr_reference = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local_name = e.local_name();
                let tag_bytes = local_name.as_ref();

                match state {
                    State::Skipping => {
                        skip_depth += 1;
                    }
                    State::Default => {
                        if tag_bytes == b"UBLExtensions" || tag_bytes == b"Signature" {
                            state = State::Skipping;
                            skip_depth = 1;
                        } else if tag_bytes == b"AdditionalDocumentReference" {
                            state = State::InAdditionalDocRef;
                            adr_depth = 1;
                            is_qr_reference = false;
                            adr_buffer.clear();
                            // In latest quick-xml, Event implements AsRef<[u8]>
                            // We wrap in < > because as_ref() on Start event usually
                            // contains the tag content without the brackets.
                            adr_buffer.push(b'<');
                            adr_buffer.extend_from_slice(e.as_ref());
                            adr_buffer.push(b'>');
                        } else {
                            writer.write_event(Event::Start(e))?;
                        }
                    }
                    State::InAdditionalDocRef | State::InAdditionalDocRefId => {
                        adr_depth += 1;
                        if tag_bytes == b"ID" && state == State::InAdditionalDocRef {
                            state = State::InAdditionalDocRefId;
                        }
                        adr_buffer.push(b'<');
                        adr_buffer.extend_from_slice(e.as_ref());
                        adr_buffer.push(b'>');
                    }
                }
            }

            Ok(Event::Text(e)) => {
                match state {
                    State::Skipping => {}
                    State::InAdditionalDocRefId => {
                        // Check if the unescaped content is exactly "QR"
                        if e.as_ref().trim_ascii() == b"QR" {
                            is_qr_reference = true;
                        }
                        adr_buffer.extend_from_slice(e.as_ref());
                    }
                    State::InAdditionalDocRef => {
                        adr_buffer.extend_from_slice(e.as_ref());
                    }
                    State::Default => {
                        writer.write_event(Event::Text(e))?;
                    }
                }
            }

            Ok(Event::End(e)) => match state {
                State::Skipping => {
                    skip_depth -= 1;
                    if skip_depth == 0 {
                        state = State::Default;
                    }
                }
                State::InAdditionalDocRefId | State::InAdditionalDocRef => {
                    adr_depth -= 1;
                    adr_buffer.extend_from_slice(b"</");
                    adr_buffer.extend_from_slice(e.as_ref());
                    adr_buffer.push(b'>');

                    if state == State::InAdditionalDocRefId {
                        state = State::InAdditionalDocRef;
                    }

                    if adr_depth == 0 {
                        if !is_qr_reference {
                            writer.get_mut().extend_from_slice(&adr_buffer);
                        }
                        state = State::Default;
                    }
                }
                State::Default => {
                    writer.write_event(Event::End(e))?;
                }
            },

            Ok(Event::Empty(e)) => match state {
                State::Skipping => {}
                State::Default => {
                    if e.local_name().as_ref() != b"UBLExtensions"
                        && e.local_name().as_ref() != b"Signature"
                    {
                        writer.write_event(Event::Empty(e))?;
                    }
                }
                _ => {
                    adr_buffer.push(b'<');
                    adr_buffer.extend_from_slice(e.as_ref());
                    adr_buffer.extend_from_slice(b"/>");
                }
            },

            Ok(Event::Eof) => break,

            // Handle CData, Comments, Decl, PI explicitly to ensure byte-fidelity
            Ok(ev) => match state {
                State::Default => {
                    writer.write_event(ev)?;
                }
                State::Skipping => {}
                _ => {
                    adr_buffer.extend_from_slice(ev.as_ref());
                }
            },
            Err(e) => return Err(e.into()),
        }
        buf.clear();
    }

    Ok(writer.into_inner())
}

pub fn extract_sig_crt(xml: &[u8]) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);

    let mut current = 0u8; // 0 = none, 1 = signature, 2 = certificate
    let mut signature = String::with_capacity(2048);
    let mut certificate = String::with_capacity(4096);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"SignatureValue" => current = 1,
                b"X509Certificate" => current = 2,
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
            Err(e) => bail!("XML error: {}",e),
            _ => {}
        }
        buf.clear();
    }

    Ok((signature.into(), certificate.into()))
}
pub fn extract_signed_properties(xml: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    let mut capturing = false;
    let mut depth: usize = 0;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                if e.local_name().as_ref() == b"SignedProperties" {
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

            // Event::CData(e) => {
            //     if capturing {
            //         writer.write_event(Event::CData(e.to_owned()))?;
            //     }
            // }
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

pub fn extract_icv(invoice: &[u8]) -> anyhow::Result<i32> {
    let mut reader = Reader::from_reader(Cursor::new(invoice));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);
    let mut in_icv_block = false;
    let mut icv_value = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.local_name();
                let tag = std::str::from_utf8(name.as_ref())?;

                if tag == "ID" {
                    in_icv_block = true;
                }
            }

            Ok(Event::Text(e)) => {
                if in_icv_block {
                    let text = e.decode().context("failed to read ICV from invoice")?;
                    if text == "ICV" {
                        // Found ICV marker, next UUID will have the value
                    } else if !text.is_empty() && icv_value.is_empty() {
                        // This is the value we want
                        icv_value = text.into_owned();
                    }
                }
            }

            Ok(Event::End(e)) => {
                let tag = e.local_name();
                if tag.as_ref() == b"AdditionalDocumentReference" {
                    if !icv_value.is_empty() {
                        break;
                    }
                    in_icv_block = false;
                }
            }

            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    if icv_value.is_empty() {
        return Err(anyhow::anyhow!("ICV not found in invoice"));
    }

    icv_value
        .trim()
        .parse::<i32>()
        .map_err(|_| anyhow::anyhow!("Invalid ICV value: {}", icv_value))
}

pub fn extract_customer_id(invoice: &[u8]) -> anyhow::Result<String> {
    let mut reader = Reader::from_reader(Cursor::new(invoice));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);

    let mut current = 0u8;
    let mut in_accounting_customer_party = false;
    let mut company_id = String::with_capacity(2048);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == b"AccountingCustomerParty" {
                    in_accounting_customer_party = true
                }
                if e.local_name().as_ref() == b"CompanyID" {
                    current = 1
                }
            }

            Ok(Event::Text(e)) => {
                if in_accounting_customer_party {
                    let text = e
                        .decode()
                        .context("failed to read the customer TIN from invoice")?;
                    if current == 1 {
                        company_id.push_str(&text)
                    }
                }
            }

            Ok(Event::End(_)) => current = 0,
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(company_id)
}
pub fn extract_supplier_id(invoice: &[u8]) -> anyhow::Result<String> {
    let mut reader = Reader::from_reader(Cursor::new(invoice));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);

    let mut current = 0u8;
    let mut in_accounting_supplier_party = false;
    let mut company_id = String::with_capacity(2048);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == b"AccountingSupplierParty" {
                    in_accounting_supplier_party = true
                }
                if e.local_name().as_ref() == b"CompanyID" {
                    current = 1
                }
            }

            Ok(Event::Text(e)) => {
                if in_accounting_supplier_party {
                    let text = e
                        .decode()
                        .context("failed to read the supplier TIN from invoice")?;
                    if current == 1 {
                        company_id.push_str(&text)
                    }
                }
            }

            Ok(Event::End(_)) => current = 0,
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(company_id)
}

#[derive(Debug, PartialEq)]
enum PihState {
    Searching,     // Looking for AdditionalDocumentReference
    InsideDocRef,  // Inside AdditionalDocumentReference, looking for ID
    FoundPihBlock, // Confirmed this is the PIH block, looking for DigestValue
}

pub fn extract_pih(invoice: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut reader = Reader::from_reader(Cursor::new(invoice));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);
    let mut state = PihState::Searching;
    let mut current_tag = String::new();
    let mut pih_hash = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.local_name();
                let tag = std::str::from_utf8(name.as_ref())?;
                current_tag = tag.to_string();

                if tag == "ID" {
                    state = PihState::InsideDocRef;
                }
            }

            Ok(Event::Text(e)) => {
                let text = e.decode().context("failed to read the PIH from invoice")?;

                match state {
                    PihState::InsideDocRef => {
                        // We are inside a DocRef, checking if this is the "PIH" one
                        if current_tag == "ID" && text == "PIH" {
                            state = PihState::FoundPihBlock;
                        }
                    }
                    PihState::FoundPihBlock => {
                        // We confirmed we are in PIH, now look for the digest
                        if current_tag == "EmbeddedDocumentBinaryObject" {
                            pih_hash = text.into_owned();
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::End(e)) => {
                let tag = e.local_name();
                if tag.as_ref() == b"AdditionalDocumentReference" {
                    // If we found the hash, we're done.
                    // If not, reset to search for the next DocRef (e.g., the QR one)
                    if !pih_hash.is_empty() {
                        break;
                    }
                    state = PihState::Searching;
                }
                current_tag.clear();
            }

            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    if pih_hash.is_empty() {
        return Err(anyhow::anyhow!("PIH DigestValue not found in valid block"));
    }

    Ok(pih_hash.into())
}

#[derive(Debug, PartialEq)]
enum ProfileState {
    Searching,
    InsideProfileId,
}

pub fn extract_profile_id(invoice: &[u8]) -> anyhow::Result<String> {
    let mut reader = Reader::from_reader(Cursor::new(invoice));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(1024);
    let mut state = ProfileState::Searching;
    let mut profile_id = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                // local_name() handles "cbc:ProfileID" as just "ProfileID"
                if e.local_name().as_ref() == b"ProfileID" {
                    state = ProfileState::InsideProfileId;
                }
            }

            Ok(Event::Text(e)) => {
                if state == ProfileState::InsideProfileId {
                    profile_id = e.decode()?.into_owned();
                }
            }

            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"ProfileID" {
                    // Once we close the tag, we can stop reading
                    if !profile_id.is_empty() {
                        break;
                    }
                    state = ProfileState::Searching;
                }
            }

            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    if profile_id.is_empty() {
        return Err(anyhow::anyhow!("cbc:ProfileID not found in invoice"));
    }

    Ok(profile_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn create_invoice_xml(profile_id: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
                xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
                <cbc:ProfileID>{}</cbc:ProfileID>
                <cbc:ID>SME00015</cbc:ID>
            </Invoice>"#,
            profile_id
        )
    }

    #[test]
    fn test_extract_reporting_profile() {
        let xml = create_invoice_xml("reporting:1.0");
        let result = extract_profile_id(xml.as_bytes()).unwrap();
        assert_eq!(result, "reporting:1.0");
    }

    #[test]
    fn test_extract_clearance_profile() {
        let xml = create_invoice_xml("clearance:1.0");
        let result = extract_profile_id(xml.as_bytes()).unwrap();
        assert_eq!(result, "clearance:1.0");
    }

    #[test]
    fn test_extract_profile_id_missing_tag() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <Invoice>
                <cbc:ID>SME00015</cbc:ID>
            </Invoice>"#;

        let result = extract_profile_id(xml.as_bytes());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "cbc:ProfileID not found in invoice"
        );
    }

    #[test]
    fn test_extract_profile_id_empty_tag() {
        let xml = r#"<Invoice><cbc:ProfileID></cbc:ProfileID></Invoice>"#;
        let result = extract_profile_id(xml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_malformed_xml() {
        let xml = r#"<Invoice><cbc:ProfileID>reporting:1.0</Invoice>"#; // Missing closing tag
        let result = extract_profile_id(xml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_with_enum_logic() {
        // This simulates how you'd use it with the InvoiceType enum logic
        let xml = create_invoice_xml("reporting:1.0");
        let raw_id = extract_profile_id(xml.as_bytes()).unwrap();

        let inv_type = if raw_id.contains("reporting") {
            "Reporting"
        } else {
            "Clearance"
        };

        assert_eq!(inv_type, "Reporting");
    }
    #[test]
    fn test_extract_icv() {
        let xml = br#"<cac:AdditionalDocumentReference>
        <cbc:ID>ICV</cbc:ID>
        <cbc:UUID>23</cbc:UUID>
    </cac:AdditionalDocumentReference>
    <cac:AdditionalDocumentReference>
        <cbc:ID>PIH</cbc:ID>
        <cac:Attachment>
            <cbc:EmbeddedDocumentBinaryObject mimeCode="text/plain">NWZlY2ViNjZmZmM4NmYzOGQ5NTI3ODZjNmQ2OTZjNzljMmRiYzIzOWRkNGU5MWI0NjcyOWQ3M2EyN2ZiNTdlOQ==</cbc:EmbeddedDocumentBinaryObject>
        </cac:Attachment>
    </cac:AdditionalDocumentReference>"#;
        assert_eq!(extract_icv(xml).unwrap(), 23);
    }

    #[test]
    fn test_extract_pih() {
        let xml = br#"<cac:AdditionalDocumentReference>
        <cbc:ID>ICV</cbc:ID>
        <cbc:UUID>23</cbc:UUID>
    </cac:AdditionalDocumentReference>
    <cac:AdditionalDocumentReference>
        <cbc:ID>PIH</cbc:ID>
        <cac:Attachment>
            <cbc:EmbeddedDocumentBinaryObject mimeCode="text/plain">NWZlY2ViNjZmZmM4NmYzOGQ5NTI3ODZjNmQ2OTZjNzljMmRiYzIzOWRkNGU5MWI0NjcyOWQ3M2EyN2ZiNTdlOQ==</cbc:EmbeddedDocumentBinaryObject>
        </cac:Attachment>
    </cac:AdditionalDocumentReference>
    <cac:AdditionalDocumentReference>
        <cbc:ID>QR</cbc:ID>
        <cac:Attachment>
            <cbc:EmbeddedDocumentBinaryObject mimeCode="text/plain">AW/YtNix2YPYqSDYqtmI2LHZitivINin2YTYqtmD2YbZiNmE2YjYrNmK2Kcg2KjYo9mC2LXZiSDYs9ix2LnYqSDYp9mE2YXYrdiv2YjYr9ipIHwgTWF4aW11bSBTcGVlZCBUZWNoIFN1cHBseSBMVEQCDzM5OTk5OTk5OTkwMDAwMwMTMjAyMi0wOS0wN1QxMjoyMToyOAQENC42MAUDMC42BixmKzBXQ3FuUGtJbkkrZUw5RzNMQXJ5MTJmVFBmK3RvQzlVWDA3RjRmSStzPQdgTUVVQ0lCeHlSOHJjNEs4NzI4d2RTRjRYU0RxUHMrcklMKzNURmg5bSthTnhRUHRTQWlFQTZjSGFwSXR2cDEzeU1TdTY2TmJPZzJDcG9tSHdVU25ZSjloNnVHUTY1YVk9CFgwVjAQBgcqhkjOPQIBBgUrgQQACgNCAAShYIprRJr0UgStM6/S4CQLVUgpfFT2c+nHa+V/jKEx6PLxzTZcluUOru0/J2jyarRqE4yY2jyDCeLte3UpP1R4</cbc:EmbeddedDocumentBinaryObject>
        </cac:Attachment>
    </cac:AdditionalDocumentReference>"#;
        assert_eq!(extract_pih(xml).expect("shit happend in the test"),b"NWZlY2ViNjZmZmM4NmYzOGQ5NTI3ODZjNmQ2OTZjNzljMmRiYzIzOWRkNGU5MWI0NjcyOWQ3M2EyN2ZiNTdlOQ==");
    }
    #[test]
    fn test_extract_customer_id() {
        let xml = br#"<cac:AccountingSupplierParty>
        <cac:Party>
            <cac:PartyIdentification>
                <cbc:ID schemeID="CRN">1010010000</cbc:ID>
            </cac:PartyIdentification>
            <cac:PostalAddress>
                <cbc:StreetName>  | Prince Sultan</cbc:StreetName>
                <cbc:BuildingNumber>2322</cbc:BuildingNumber>
                <cbc:CitySubdivisionName> | Al-Murabba</cbc:CitySubdivisionName>
                <cbc:CityName> | Riyadh</cbc:CityName>
                <cbc:PostalZone>23333</cbc:PostalZone>
                <cac:Country>
                    <cbc:IdentificationCode>SA</cbc:IdentificationCode>
                </cac:Country>
            </cac:PostalAddress>
            <cac:PartyTaxScheme>
                <cbc:CompanyID>399999999900003</cbc:CompanyID>
                <cac:TaxScheme>
                    <cbc:ID>VAT</cbc:ID>
                </cac:TaxScheme>
            </cac:PartyTaxScheme>
            <cac:PartyLegalEntity>
                <cbc:RegistrationName> | Maximum Speed Tech Supply LTD</cbc:RegistrationName>
            </cac:PartyLegalEntity>
        </cac:Party>
    </cac:AccountingSupplierParty>
    <cac:AccountingCustomerParty>
        <cac:Party>
            <cac:PostalAddress>
                <cbc:StreetName> | Salah Al-Din</cbc:StreetName>
                <cbc:BuildingNumber>1111</cbc:BuildingNumber>
                <cbc:CitySubdivisionName> | Al-Murooj</cbc:CitySubdivisionName>
                <cbc:CityName> | Riyadh</cbc:CityName>
                <cbc:PostalZone>12222</cbc:PostalZone>
                <cac:Country>
                    <cbc:IdentificationCode>SA</cbc:IdentificationCode>
                </cac:Country>
            </cac:PostalAddress>
            <cac:PartyTaxScheme>
                <cbc:CompanyID>399999999800003</cbc:CompanyID>
                <cac:TaxScheme>
                    <cbc:ID>VAT</cbc:ID>
                </cac:TaxScheme>
            </cac:PartyTaxScheme>
            <cac:PartyLegalEntity>
                <cbc:RegistrationName>    | Fatoora Samples LTD</cbc:RegistrationName>
            </cac:PartyLegalEntity>
        </cac:Party>
    </cac:AccountingCustomerParty>"#;
        assert_eq!(extract_customer_id(xml).unwrap(), "399999999800003")
    }
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

        assert_eq!(extract_signed_properties(xml.as_ref()).unwrap(),br#"<xades:SignedProperties Id="xadesSignedProperties">
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
