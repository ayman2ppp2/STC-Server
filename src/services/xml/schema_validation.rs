use actix_web::web::Data;
use anyhow::Context;
use fastxml::{
    parse,
    schema::{CompiledSchema, XmlSchemaValidationContext},
};
use tracing::instrument;

#[instrument(skip(schema, body))]
pub fn validate_schema(schema: Data<CompiledSchema>, body: &str) -> anyhow::Result<()> {
    let validator = XmlSchemaValidationContext::from_arc(schema.into_inner());
    let xml_doc = parse(body)?;
    validator
        .validate(&xml_doc)
        .context("XSD validation failed")?;
    Ok(())
}
