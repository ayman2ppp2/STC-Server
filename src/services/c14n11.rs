use xml_c14n::{CanonicalizationMode, CanonicalizationOptions, canonicalize_xml};
pub fn canonicalize_c14n11(cleaned_xml: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let options = CanonicalizationOptions {
        mode: CanonicalizationMode::Canonical1_1,
        keep_comments: false,
        inclusive_ns_prefixes: vec![],
    };
    let canonical = canonicalize_xml(std::str::from_utf8(&cleaned_xml)?, options)?;

    Ok(canonical.into_bytes())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_valid_xml() {
        let xml = b"<root><child>text</child></root>";
        let result = canonicalize_c14n11(xml.to_vec());
        assert!(result.is_ok());
    }

    #[test]
    fn test_canonicalize_with_whitespace() {
        let xml = b"<root>  <child>  text  </child>  </root>";
        let result = canonicalize_c14n11(xml.to_vec());
        assert!(result.is_ok());
    }

    #[test]
    fn test_canonicalize_with_comments() {
        let xml = b"<root><!-- comment --><child>text</child></root>";
        let result = canonicalize_c14n11(xml.to_vec());
        assert!(result.is_ok());
    }

    #[test]
    fn test_canonicalize_invalid_utf8() {
        let invalid_utf8 = vec![0xFF, 0xFE];
        let result = canonicalize_c14n11(invalid_utf8);
        assert!(result.is_err());
    }

    #[test]
    fn test_canonicalize_malformed_xml() {
        let xml = b"<root><child>text</root>";
        let result = canonicalize_c14n11(xml.to_vec());
        assert!(result.is_err());
    }

    #[test]
    fn test_canonicalize_namespaces() {
        let xml = b"<root xmlns='http://example.com'><child>text</child></root>";
        let result = canonicalize_c14n11(xml.to_vec());
        assert!(result.is_ok());
    }
}
