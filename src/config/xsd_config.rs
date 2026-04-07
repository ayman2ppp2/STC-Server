use std::sync::RwLock;

use anyhow::Context;
use include_dir::{include_dir, Dir};
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use tempfile::TempDir;

static XSD_DATA: Dir = include_dir!("$CARGO_MANIFEST_DIR/schemas/UBL-2.1/xsd");

pub struct SchemaValidator {
    // xsd_path: String,
    validation_pool: RwLock<Vec<SchemaValidationContext>>,
    _temp_dir: TempDir,
}

impl SchemaValidator {
    pub fn new(pool_size: usize) -> anyhow::Result<Self> {
        let tmp_dir = tempfile::tempdir().context("Failed to create temp dir")?;
        let tmp_path = tmp_dir.path();

        XSD_DATA
            .extract(tmp_path)
            .context("Failed to extract XSDs")?;

        let xsd_entry_path = tmp_path.join("maindoc/UBL-Invoice-2.1.xsd");
        let xsd_path = xsd_entry_path.to_string_lossy().to_string();

        println!("XSDs extracted to: {:?}", xsd_path);

        let mut validation_pool = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            let mut parser_context = SchemaParserContext::from_file(&xsd_path);
            match SchemaValidationContext::from_parser(&mut parser_context) {
                Ok(ctx) => validation_pool.push(ctx),
                Err(e) => println!(
                    "Warning: Failed to create validation context {}: {:?}",
                    i, e
                ),
            }
        }

        if validation_pool.is_empty() {
            return Err(anyhow::anyhow!("Failed to create any validation context"));
        }

        println!(
            "Created validation context pool with {} contexts",
            validation_pool.len()
        );

        Ok(Self {
            // xsd_path,
            validation_pool: RwLock::new(validation_pool),
            _temp_dir: tmp_dir,
        })
    }

    pub fn get_context(&self) -> Option<SchemaValidationContext> {
        self.validation_pool.write().unwrap().pop()
    }

    pub fn return_context(&self, ctx: SchemaValidationContext) {
        self.validation_pool.write().unwrap().push(ctx);
    }

    pub fn pool_size(&self) -> usize {
        self.validation_pool.read().unwrap().len()
    }
}

unsafe impl Send for SchemaValidator {}
unsafe impl Sync for SchemaValidator {}
