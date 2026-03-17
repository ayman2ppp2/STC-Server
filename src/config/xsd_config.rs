use std::path::PathBuf;

use include_dir::{include_dir, Dir};
use tempfile::TempDir;

static XSD_DATA: Dir = include_dir!("$CARGO_MANIFEST_DIR/schemas/UBL-2.1/xsd");

pub struct SchemaValidator {
    pub xsd_entry_path: PathBuf,
    _temp_dir: TempDir,
}

impl SchemaValidator {
    pub fn new() -> Self {
        let tmp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let tmp_path = tmp_dir.path();

        // Extract the embedded XSDs to the temporary location once at startup
        XSD_DATA.extract(tmp_path).expect("Failed to extract XSDs");

        let xsd_entry_path = tmp_path.join("maindoc/UBL-Invoice-2.1.xsd");
        
        println!("XSDs extracted to: {:?}", xsd_entry_path);

        Self {
            xsd_entry_path,
            _temp_dir: tmp_dir,
        }
    }
}

