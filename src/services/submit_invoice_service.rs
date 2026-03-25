// use anyhow::{Context, Result, bail};
// use openssl::memcmp;
// use sqlx::PgPool;

// use crate::{config::{crypto_config::Crypto, xsd_config::SchemaValidator}, models::submit_invoice_dto::IntermediateInvoiceDto, services::{check_uuid::check_uuid, clear_invoice::clear_invoice, pki_service::{compute_hash, verify_cert_with_ca, verify_signature_with_cert}, save_invoice::save_invoice, schema_validation::validate_schema, verify_pih::verify_pih}};

// pub async fn process_invoice(
//     intermediate: IntermediateInvoiceDto,
//     db_pool: &PgPool,
//     crypto: &Crypto,
//     sandbox : bool,
//     schema : &SchemaValidator
// ) -> Result<String> {
//     // check uuid 

//     if !sandbox {
//         check_uuid(&intermediate.uuid,db_pool).await?;
//     }

//     // validate schema
//     let xml_body = std::str::from_utf8(&intermediate.invoice_bytes)
//         .context("Invoice XML is not valid UTF-8")?;
//     validate_schema(schema, xml_body)?;

    
//     // verify hash
//     let received_hash = &intermediate.invoice_hash;
//     let hash = compute_hash(&intermediate.canonicalized_invoice_bytes)?;

//     if !memcmp::eq(received_hash, &hash) {
//         bail!("Invoice hash mismatch");
//     }

//     // verify PIH chain if not in sandbox mode 
//     if !sandbox {
//         verify_pih(
//         &intermediate.invoice_bytes,
//         db_pool,
//         &intermediate.company,
//     )
//     .await?;
//     } 

//     // verify certificate
//     verify_cert_with_ca(&crypto.certificate, &intermediate.certificate).await?;

//     // verify signature
//     verify_signature_with_cert(
//         &intermediate.invoice_hash,
//         &intermediate.invoice_signature,
//         &intermediate.certificate,
//     )?;

//     // clear invoice
//     let (hash, cleared_invoice) = clear_invoice(&intermediate, crypto)?;

//     // store invoice
//     save_invoice(
//         db_pool,
//         &cleared_invoice,
//         &intermediate.uuid,
//         hash,
//         intermediate.company,
//     )
//     .await?;

//     Ok(cleared_invoice)
// }

