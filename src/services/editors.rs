use anyhow::anyhow;
use chrono::Utc;
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

use crate::services::edit_tlv::edit_tlv;

/*
1. Canonicalize invoice → hash invoice
2. Build SignedProperties
3. Canonicalize SignedProperties → hash SignedProperties
4. Build SignedInfo (with both digests)
5. Canonicalize SignedInfo -> hash signedInfo
6. ECDSA sign → SignatureValue
7. Build QR using invoice hash + signature
*/

/*
get canonical invoice hash then build the signed poperties
then canonicalize signed properties to get its hash
then build signed info with both hashes
canonicalize signed info to get its hash
sign the signed info hash to get signature value
build QR using invoice hash + signature
*/

pub fn edit_signing_time(xml: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));

    // Example: trim whitespace around text if you want
    {
        let cfg = reader.config_mut();
        cfg.trim_text_start = false;
        cfg.trim_text_end = false;
    }

    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    // State — whether we are inside a <SigningTime> tag
    let mut in_signature = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == b"SigningTime" {
                    in_signature = true;
                }

                writer.write_event(Event::Start(e.to_owned()))?;
            }

            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"SigningTime" {
                    in_signature = false;
                }

                writer.write_event(Event::End(e.to_owned()))?;
            }

            Ok(Event::Text(e)) => {
                if in_signature {
                    let new_time = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                    writer.write_event(Event::Text(BytesText::new(&new_time)))?;
                } else {
                    writer.write_event(Event::Text(e.to_owned()))?;
                }
            }

            Ok(Event::Empty(e)) => {
                // Empty tags pass through
                writer.write_event(Event::Empty(e.to_owned()))?;
            }

            Ok(Event::Eof) => break,

            Ok(ev) => {
                writer.write_event(ev.to_owned())?;
            }

            Err(e) => return Err(anyhow!(format!("XML parse error : {}", e))),
        }

        buf.clear();
    }

    Ok(writer.into_inner())
}

#[derive(Debug, PartialEq)]
enum ActiveReference {
    Invoice,
    SignedProperties,
    Other,
}
pub fn edit_signed_info(
    xml: &[u8],
    invoice_hash: &[u8],
    signed_props_hash: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));

    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    let mut active_ref = ActiveReference::Other;
    let mut in_digest_value = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                // <ds:Reference>
                if e.local_name().as_ref() == b"Reference" {
                    active_ref = ActiveReference::Other;

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Id" if attr.value.as_ref() == b"invoiceSignedData" => {
                                active_ref = ActiveReference::Invoice;
                            }
                            b"Type"
                                if attr.value.as_ref()
                                    == b"http://www.w3.org/2000/09/xmldsig#SignatureProperties" =>
                            {
                                active_ref = ActiveReference::SignedProperties;
                            }
                            _ => {}
                        }
                    }
                }

                // <ds:DigestValue>
                if e.local_name().as_ref() == b"DigestValue" {
                    in_digest_value = true;
                }

                writer.write_event(Event::Start(e.to_owned()))?;
            }

            Event::End(e) => {
                if e.local_name().as_ref() == b"Reference" {
                    active_ref = ActiveReference::Other;
                }

                if e.local_name().as_ref() == b"DigestValue" {
                    in_digest_value = false;
                }

                writer.write_event(Event::End(e.to_owned()))?;
            }

            Event::Text(e) => {
                if in_digest_value {
                    match active_ref {
                        ActiveReference::Invoice => {
                            let str_invoice_hash = String::from_utf8(invoice_hash.to_vec())?;
                            writer.write_event(Event::Text(BytesText::new(&str_invoice_hash)))?;
                        }
                        ActiveReference::SignedProperties => {
                            let str_signed_props_hash =
                                String::from_utf8(signed_props_hash.to_vec())?;
                            writer
                                .write_event(Event::Text(BytesText::new(&str_signed_props_hash)))?;
                        }
                        ActiveReference::Other => {
                            writer.write_event(Event::Text(e.to_owned()))?;
                        }
                    }
                } else {
                    writer.write_event(Event::Text(e.to_owned()))?;
                }
            }

            Event::Empty(e) => {
                writer.write_event(Event::Empty(e.to_owned()))?;
            }

            Event::Eof => break,

            ev => {
                writer.write_event(ev)?;
            }
        }

        buf.clear();
    }

    Ok(writer.into_inner())
}

pub fn edit_signature(xml: &[u8], signature: String) -> anyhow::Result<Vec<u8>> {
    let mut reader = Reader::from_reader(Cursor::new(xml));

    // Example: trim whitespace around text if you want
    {
        let cfg = reader.config_mut();
        cfg.trim_text_start = false;
        cfg.trim_text_end = false;
    }

    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    // State — whether we are inside a <SigningTime> tag
    let mut in_signature = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == b"SignatureValue" {
                    in_signature = true;
                }

                writer.write_event(Event::Start(e.to_owned()))?;
            }

            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"SignatureValue" {
                    in_signature = false;
                }

                writer.write_event(Event::End(e.to_owned()))?;
            }

            Ok(Event::Text(e)) => {
                if in_signature {
                    writer.write_event(Event::Text(BytesText::new(&signature)))?;
                } else {
                    writer.write_event(Event::Text(e.to_owned()))?;
                }
            }

            Ok(Event::Empty(e)) => {
                // Empty tags pass through
                writer.write_event(Event::Empty(e.to_owned()))?;
            }

            Ok(Event::Eof) => break,

            Ok(ev) => {
                writer.write_event(ev.to_owned())?;
            }

            Err(e) => return Err(anyhow!(format!("XML parse error: {}", e))),
        }

        buf.clear();
    }

    Ok(writer.into_inner())
}

// Helper to gracefully ignore XML namespaces (e.g., handles "cac:AdditionalDocumentReference" or "AdditionalDocumentReference")

pub fn edit_qr(xml: &[u8], hash: &[u8], signature: &[u8]) -> anyhow::Result<Vec<u8>> {
    // IMPORTANT: Do not configure reader.trim_text(true).
    // We must preserve exact whitespaces for signature validity.
    let mut reader = Reader::from_reader(Cursor::new(xml));
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    // State trackers
    let mut in_additional_doc_ref = false;
    let mut in_id_tag = false;
    let mut is_qr_block = false;
    let mut in_binary_object = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                if e.local_name().as_ref() == b"AdditionalDocumentReference" {
                    in_additional_doc_ref = true;
                    is_qr_block = false; // Reset block state
                }

                if in_additional_doc_ref && e.local_name().as_ref() == b"ID" {
                    in_id_tag = true;
                }

                if is_qr_block && e.local_name().as_ref() == b"EmbeddedDocumentBinaryObject" {
                    in_binary_object = true;
                }

                writer.write_event(Event::Start(e.to_owned()))?;
            }

            Event::End(e) => {
                if e.local_name().as_ref() == b"AdditionalDocumentReference" {
                    in_additional_doc_ref = false;
                    is_qr_block = false;
                }

                if e.local_name().as_ref() == b"ID" {
                    in_id_tag = false;
                }

                if e.local_name().as_ref() == b"EmbeddedDocumentBinaryObject" {
                    in_binary_object = false;
                }

                writer.write_event(Event::End(e.to_owned()))?;
            }

            Event::Text(e) => {
                if in_id_tag {
                    // Check if this AdditionalDocumentReference is specifically the "QR" one
                    if e.as_ref() == b"QR" {
                        is_qr_block = true;
                    }
                    writer.write_event(Event::Text(e.to_owned()))?;
                } else if in_binary_object {
                    // WE FOUND IT: Swap the old base64 text with the edited base64 text
                    let e = edit_tlv(&e, hash, signature)?;
                    writer.write_event(Event::Text(BytesText::new(&e)))?;
                } else {
                    // Pass all other text content through completely untouched
                    writer.write_event(Event::Text(e.to_owned()))?;
                }
            }

            Event::Empty(e) => {
                // Safely pass through self-closing tags like <cbc:ID/>
                writer.write_event(Event::Empty(e.to_owned()))?;
            }

            Event::Eof => break,

            ev => {
                // Pass through Comments, CData, Decl, PIs untouched
                writer.write_event(ev.to_owned())?;
            }
        }

        buf.clear();
    }

    Ok(writer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::de::from_str;
    use std::fs;

    // Test constants
    const TEST_INVOICE_HASH: &str = "new_test_invoice_hash_value";
    const TEST_SIGNED_PROPS_HASH: &str = "new_test_signed_props_hash_value";
    const TEST_SIGNATURE: &str = "new_test_signature_value_for_testing";

    // Helper functions
    fn load_test_invoice() -> Vec<u8> {
        fs::read("test.xml").expect("Failed to read test.xml from project root")
    }

    fn validate_xml_well_formed(xml: &[u8]) -> bool {
        let xml_str = std::str::from_utf8(xml);
        match xml_str {
            Ok(s) => from_str::<serde_json::Value>(s).is_ok(),
            Err(_) => false,
        }
    }

    fn extract_current_signing_time(xml: &[u8]) -> Option<String> {
        let xml_str = std::str::from_utf8(xml).ok()?;
        let start = xml_str.find("<xades:SigningTime>")?;
        let end = xml_str.find("</xades:SigningTime>")?;
        Some(xml_str[start + 20..end].to_string())
    }

    // Original simple tests
    #[test]
    fn test_clear_invoice() {
        let xml = r#"<Root>
    <Data>Some data</Data>
    <SigningTime>2023-10-01T12:00:00Z</SigningTime>
    <MoreData>Other data</MoreData>
    </Root>"#;
        let cleared = String::from_utf8(edit_signing_time(xml.as_bytes()).unwrap()).unwrap();
        assert!(cleared.contains("<Data>Some data</Data>"));
        assert!(cleared.contains("<MoreData>Other data</MoreData>"));
        assert!(!cleared.contains("2023-10-01T12:00:00Z")); // Old time should be gone
        assert!(cleared.contains("<SigningTime>")); // Tag should still be there
    }

    #[test]
    fn test_edit_qr() {
        let xml = br#"<cac:AdditionalDocumentReference>
        <cbc:ID>PIH</cbc:ID>
        <cac:Attachment>
            <cbc:EmbeddedDocumentBinaryObject mimeCode="text/plain">NWZlY2ViNjZmZmM4NmYzOGQ5NTI3ODZjNmQ2OTZjNzljMmRiYzIzOWRkNGU5MWI0NjcyOWQ3M2EyN2ZiNTdlOQ==</cbc:EmbeddedDocumentBinaryObject>
        </cac:Attachment>
    </cac:AdditionalDocumentReference>
    <cac:AdditionalDocumentReference>
        <cbc:ID>QR</cbc:ID>
        <cac:Attachment>
            <cbc:EmbeddedDocumentBinaryObject mimeCode="text/plain">AW/YtNix2YPYqSDYqtmI2LHZitivINin2YTYqtmD2YbZiNmE2YjYrNmK2Kcg2KjYo9mC2LXZiSDYs9ix2LnYqSDYp9mE2YXYrdiv2YjYr9ipIHwgTWF4aW11bSBTcGVlZCBUZWNoIFN1cHBseSBMVEQCDzMxMDA5NDAxMDMwMDAwMwMTMjAyMi0wOS0wNVQxNjo1MDo0MAQENC42MAUDMC42BixrRS84Q2dEMHFYZUZnRHNPUVZzTzlWZXN2dUxhVjE5aFlUeGRteUpvYTg4PQdgTUVRQ0lCZ2ZtSXRhN2FPeU5ndVRaUXU1T3ZCUFh2M0E5blJsb3dzZjJSTnE5b3l6QWlCYno4a1ZMOWlIdEYwNTFiVmJUT3hrZzdIM3UybGcreDRSZDM3Q1VtNDV5QT09CFgwVjAQBgcqhkjOPQIBBgUrgQQACgNCAARqD+MJJBltQnk7fzrUgl4/N2CPcAeTPTAD7RatCimMjrul4UMQuWH4FrduVOCclWrFlDk8aJV2tAmRgUxbiNA5</cbc:EmbeddedDocumentBinaryObject>
        </cac:Attachment>
    </cac:AdditionalDocumentReference>"#;
        let hash = [0u8; 9];
        let signature = [0u8; 17];
        let result = edit_qr(xml, &hash, &signature).unwrap();
        //    dbg!("left : {}",String::from_utf8_lossy(&result));
        assert_ne!(result, xml);
    }

    // Comprehensive edit_signing_time test using real invoice XML
    #[test]
    fn test_edit_signing_time_with_real_invoice() {
        let xml = load_test_invoice();
        let original_time = extract_current_signing_time(&xml).unwrap();
        let result = edit_signing_time(&xml).unwrap();

        let result_str = String::from_utf8(result.clone()).unwrap();

        // Verify old time is gone
        assert!(!result_str.contains(&original_time));

        // Verify new timestamp is present and follows ISO format
        assert!(result_str.contains("<xades:SigningTime>"));
        assert!(result_str.contains("</xades:SigningTime>"));

        // Extract new time and validate format
        let start = result_str.find("<xades:SigningTime>").unwrap();
        let end = result_str.find("</xades:SigningTime>").unwrap();
        let new_time = &result_str[start + 20..end];

        // Should match ISO format pattern
        assert!(new_time.len() >= 19); // At least YYYY-MM-DDTHH:MM:SS
        assert!(new_time.contains('T'));
        assert!(new_time.ends_with('Z'));

        // Verify XML is still well-formed
        assert!(validate_xml_well_formed(&result));
    }

    // edit_signed_info test with real invoice XML
    #[test]
    fn test_edit_signed_info_invoice_hash_with_real_invoice() {
        let xml = load_test_invoice();
        let result = edit_signed_info(
            &xml,
            TEST_INVOICE_HASH.as_bytes(),
            TEST_SIGNED_PROPS_HASH.as_bytes(),
        )
        .unwrap();
        let result_str = String::from_utf8(result.clone()).unwrap();

        // Verify old invoice hash is gone
        assert!(!result_str.contains("V4U5qlZ3yXQ/Si1AC/R8SLc3F+iNy27wdVe8IWRqFAQ="));

        // Verify new invoice hash is present
        assert!(result_str.contains(TEST_INVOICE_HASH));

        // Verify invoiceSignedData Reference structure is preserved
        assert!(result_str.contains("Id=\"invoiceSignedData\""));
        assert!(result_str.contains("URI=\"\""));

        // Verify XML is still well-formed
        assert!(validate_xml_well_formed(&result));
    }

    // edit_signature test with real invoice XML
    #[test]
    fn test_edit_signature_with_real_invoice() {
        let xml = load_test_invoice();
        let result = edit_signature(&xml, TEST_SIGNATURE.to_string()).unwrap();
        let result_str = String::from_utf8(result.clone()).unwrap();

        // Verify old signature is gone
        assert!(!result_str.contains("MEUCIBxyR8rc4K8728wdSF4XSDqPs+rIL+3TFh9m+aNxQPtSAiEA6cHapItvp13yMSu66NbOg2CpomHwUSnYJ9h6uGQ65aY="));

        // Verify new signature is present
        assert!(result_str.contains(TEST_SIGNATURE));

        // Verify ds namespace is preserved
        assert!(result_str.contains("<ds:SignatureValue>"));
        assert!(result_str.contains("</ds:SignatureValue>"));

        // Verify XML is still well-formed
        assert!(validate_xml_well_formed(&result));
    }

    // Integration test - complete signing workflow simulation
    #[test]
    fn test_complete_signing_workflow_with_real_invoice() {
        let xml = load_test_invoice();

        // Step 1: Edit signing time
        let step1 = edit_signing_time(&xml).unwrap();
        assert!(validate_xml_well_formed(&step1));

        // Step 2: Edit signed info
        let step2 = edit_signed_info(
            &step1,
            TEST_INVOICE_HASH.as_bytes(),
            TEST_SIGNED_PROPS_HASH.as_bytes(),
        )
        .unwrap();
        assert!(validate_xml_well_formed(&step2));

        // Step 3: Edit signature
        let final_result = edit_signature(&step2, TEST_SIGNATURE.to_string()).unwrap();
        assert!(validate_xml_well_formed(&final_result));

        let result_str = String::from_utf8(final_result.clone()).unwrap();

        // Verify all modifications are present
        assert!(result_str.contains(TEST_INVOICE_HASH));
        assert!(result_str.contains(TEST_SIGNED_PROPS_HASH));
        assert!(result_str.contains(TEST_SIGNATURE));

        // Verify old values are gone
        assert!(!result_str.contains("V4U5qlZ3yXQ/Si1AC/R8SLc3F+iNy27wdVe8IWRqFAQ="));
        assert!(!result_str.contains("ODQwNTg1NTBhMjMzM2YxY2ZkZjVkYzdlNTZiZjY0ODJjMjNkYWI4MTUzNjdmNDVjMjAwZTBjODc2YTNhMWQ1Ng=="));
        assert!(!result_str.contains("MEUCIBxyR8rc4K8728wdSF4XSDqPs+rIL+3TFh9m+aNxQPtSAiEA6cHapItvp13yMSu66NbOg2CpomHwUSnYJ9h6uGQ65aY="));

        // Verify old signing time is gone (contains current time)
        assert!(!result_str.contains("2024-01-14T10:21:40"));

        // Verify XML structure is preserved
        assert!(result_str.contains("<cbc:ID>SME00015</cbc:ID>"));
        assert!(result_str.contains("<cbc:IssueDate>2022-09-05</cbc:IssueDate>"));
        assert!(result_str.contains("<xades:SignedProperties"));
        assert!(result_str.contains("<ds:SignedInfo>"));
    }

    // Legacy simple tests (preserved for backward compatibility)
    #[test]
    fn test_edit_signed_info_invoice_hash_replacement() {
        let xml = r#"<Root><Reference Id="invoiceSignedData"><DigestValue>oldInvoiceHash</DigestValue></Reference></Root>"#;
        let result = String::from_utf8(
            edit_signed_info(xml.as_bytes(), b"newInvoiceHash", b"propHash").unwrap(),
        )
        .unwrap();
        assert!(!result.contains("oldInvoiceHash"));
        assert!(result.contains("newInvoiceHash"));
    }

    #[test]
    fn test_edit_signed_info_signed_properties_hash_replacement() {
        let xml = r#"<Root><Reference Type="http://www.w3.org/2000/09/xmldsig#SignatureProperties"><DigestValue>oldPropHash</DigestValue></Reference></Root>"#;
        let result = String::from_utf8(
            edit_signed_info(xml.as_bytes(), b"invHash", b"newPropHash").unwrap(),
        )
        .unwrap();
        assert!(!result.contains("oldPropHash"));
        assert!(result.contains("newPropHash"));
    }

    #[test]
    fn test_edit_signed_info_preserves_other_digests() {
        let xml = r#"<Root><Reference><DigestValue>otherHash</DigestValue></Reference></Root>"#;
        let result =
            String::from_utf8(edit_signed_info(xml.as_bytes(), b"invHash", b"propHash").unwrap())
                .unwrap();
        assert!(result.contains("otherHash"));
    }

    #[test]
    fn test_edit_signed_info_preserves_structure() {
        let xml = r#"<Root><Reference Id="invoiceSignedData"><DigestMethod Algorithm="test"/><DigestValue>hash</DigestValue></Reference></Root>"#;
        let result =
            String::from_utf8(edit_signed_info(xml.as_bytes(), b"newHash", b"propHash").unwrap())
                .unwrap();
        assert!(result.contains("<DigestMethod"));
        assert!(result.contains("Algorithm="));
    }
}
#[test]
fn test_edit_signing_time_updates_timestamp() {
    let xml = r#"<Root>
    <Data>Some data</Data>
    <SigningTime>2023-10-01T12:00:00Z</SigningTime>
    <MoreData>Other data</MoreData>
</Root>"#;
    let result = String::from_utf8(edit_signing_time(xml.as_bytes()).unwrap()).unwrap();
    assert!(result.contains("<Data>Some data</Data>"));
    assert!(result.contains("<MoreData>Other data</MoreData>"));
    assert!(!result.contains("2023-10-01T12:00:00Z"));
    assert!(result.contains("<SigningTime>"));
    assert!(result.contains("T") && result.contains("Z")); // Contains ISO format timestamp
}

#[test]
fn test_edit_signed_info_invoice_hash_replacement() {
    let xml = r#"<Root><Reference Id="invoiceSignedData"><DigestValue>oldInvoiceHash</DigestValue></Reference></Root>"#;
    let result = String::from_utf8(
        edit_signed_info(xml.as_bytes(), b"newInvoiceHash", b"propHash").unwrap(),
    )
    .unwrap();
    assert!(!result.contains("oldInvoiceHash"));
    assert!(result.contains("newInvoiceHash"));
}

#[test]
fn test_edit_signed_info_signed_properties_hash_replacement() {
    let xml = r#"<Root><Reference Type="http://www.w3.org/2000/09/xmldsig#SignatureProperties"><DigestValue>oldPropHash</DigestValue></Reference></Root>"#;
    let result =
        String::from_utf8(edit_signed_info(xml.as_bytes(), b"invHash", b"newPropHash").unwrap())
            .unwrap();
    assert!(!result.contains("oldPropHash"));
    assert!(result.contains("newPropHash"));
}

#[test]
fn test_edit_signed_info_preserves_other_digests() {
    let xml = r#"<Root><Reference><DigestValue>otherHash</DigestValue></Reference></Root>"#;
    let result =
        String::from_utf8(edit_signed_info(xml.as_bytes(), b"invHash", b"propHash").unwrap())
            .unwrap();
    assert!(result.contains("otherHash"));
}

#[test]
fn test_edit_signed_info_preserves_structure() {
    let xml = r#"<Root><Reference Id="invoiceSignedData"><DigestMethod Algorithm="test"/><DigestValue>hash</DigestValue></Reference></Root>"#;
    let result =
        String::from_utf8(edit_signed_info(xml.as_bytes(), b"newHash", b"propHash").unwrap())
            .unwrap();
    assert!(result.contains("<DigestMethod"));
    assert!(result.contains("Algorithm="));
}
