use anyhow::Result;
use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use crate::config::xsd_config::SchemaValidator;

pub fn validate_schema(schema: &SchemaValidator, body: &str) -> Result<String> {
    let xsd_path_str = schema.xsd_entry_path.to_str().unwrap_or_default();

    let mut parser_context = SchemaParserContext::from_file(xsd_path_str);
    let mut validation_context = SchemaValidationContext::from_parser(&mut parser_context)
        .map_err(|errors| anyhow::anyhow!("XSD parse failed: {:?}", errors))?;

    let doc = Parser::default()
        .parse_string(body)
        .map_err(|e| anyhow::anyhow!("XML syntax error: {}", e))?;

    validation_context
        .validate_document(&doc)
        .map_err(|errors| anyhow::anyhow!("XSD validation failed: {:?}", errors))?;

    Ok("valid".to_string())
}