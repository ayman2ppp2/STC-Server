use crate::config::xsd_config::SchemaValidator;
use anyhow::Result;
use libxml::parser::Parser;

pub fn validate_schema(schema: &SchemaValidator, body: &str) -> Result<String> {
    let mut validation_context = schema
        .get_context()
        .ok_or_else(|| anyhow::anyhow!("Validation context pool exhausted"))?;

    let parse_result = (|| {
        let doc = Parser::default()
            .parse_string(body)
            .map_err(|e| anyhow::anyhow!("XML syntax error: {}", e))?;

        validation_context
            .validate_document(&doc)
            .map_err(|errors| anyhow::anyhow!("XSD validation failed: {:?}", errors))?;

        Ok::<(), anyhow::Error>(())
    })();

    schema.return_context(validation_context);

    parse_result?;

    Ok("valid".to_string())
}
