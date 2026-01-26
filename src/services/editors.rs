use anyhow::anyhow;
use chrono::Utc;
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

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
    let mut in_signing_time = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == b"SigningTime" {
                    in_signing_time = true;
                }

                writer
                    .write_event(Event::Start(e.to_owned()))
                    ?;
            }

            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"SigningTime" {
                    in_signing_time = false;
                }

                writer
                    .write_event(Event::End(e.to_owned()))
                    ?;
            }

            Ok(Event::Text(e)) => {
                if in_signing_time {
                    let new_time = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                    writer
                        .write_event(Event::Text(BytesText::new(&new_time)))
                        ?;
                } else {
                    writer
                        .write_event(Event::Text(e.to_owned()))
                        ?;
                }
            }

            Ok(Event::Empty(e)) => {
                // Empty tags pass through
                writer
                    .write_event(Event::Empty(e.to_owned()))
                    ?;
            }

            Ok(Event::Eof) => break,

            Ok(ev) => {
                writer
                    .write_event(ev.to_owned())
                    ?;
            }

            Err(e) => return Err(anyhow!("XML parse error")),
        }

        buf.clear();
    }

    Ok(writer.into_inner())
}

/// Strip namespace prefix.

#[cfg(test)]
mod tests {
    use super::*;
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
}
