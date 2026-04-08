use anyhow::Context;
use fastxml::schema::{CompiledSchema, FileFetcher, parse_xsd_with_imports};
use include_dir::{Dir, include_dir};
static XSD_DATA: Dir = include_dir!("$CARGO_MANIFEST_DIR/schemas/UBL-2.1/xsd");

pub fn schema_validator_from_temp() -> anyhow::Result<CompiledSchema> {
    let tmp_dir = tempfile::tempdir().context("Failed to create temp dir")?;

    XSD_DATA
        .extract(tmp_dir.path())
        .context("Failed to extract XSDs")?;
    let xsd_path = tmp_dir.path().join("maindoc/UBL-Invoice-2.1.xsd");
    let xsd_content = std::fs::read(&xsd_path).context("Failed to read XSD file")?;
    let fetcher = FileFetcher::with_base_dir(tmp_dir.path());
    let schema = parse_xsd_with_imports(
        &xsd_content,
        &format!("file://{}", xsd_path.display()),
        &fetcher,
    )
    .context("Failed to parse XSD with imports")?;
    println!("XSD schema compiled successfully");
    Ok(schema)
}
