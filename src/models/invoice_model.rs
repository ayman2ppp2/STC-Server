use serde::{Deserialize, Serialize};
use sqlx::{Decode, FromRow};
#[derive(Debug, Deserialize, Serialize, FromRow)]
#[serde(rename = "Invoice")]
pub struct Invoice {
    #[serde(rename = "ID")]
    pub id: String,

    #[serde(rename = "IssueDate")]
    pub issue_date: String,

    #[serde(rename = "InvoiceTypeCode")]
    pub invoice_type_code: Option<String>,

    #[serde(rename = "DocumentCurrencyCode")]
    pub document_currency_code: Option<String>,

    #[serde(rename = "AccountingSupplierParty")]
    pub accounting_supplier_party: Option<PartyWrapper>,

    #[serde(rename = "AccountingCustomerParty")]
    pub accounting_customer_party: Option<PartyWrapper>,

    #[serde(rename = "InvoiceLine")]
    pub invoice_lines: Option<Vec<InvoiceLine>>,

    #[serde(rename = "TaxTotal")]
    pub tax_total: Option<TaxTotal>,

    #[serde(rename = "LegalMonetaryTotal")]
    pub legal_monetary_total: Option<LegalMonetaryTotal>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct PartyWrapper {
    #[serde(rename = "Party")]
    pub party: Party,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Party {
    #[serde(rename = "Name")]
    pub name: Option<String>,

    #[serde(rename = "PostalAddress")]
    pub postal_address: Option<PostalAddress>,

    #[serde(rename = "PartyTaxScheme")]
    pub party_tax_scheme: Option<PartyTaxScheme>,

    #[serde(rename = "Contact")]
    pub contact: Option<Contact>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct PostalAddress {
    #[serde(rename = "StreetName")]
    pub street_name: Option<String>,

    #[serde(rename = "CityName")]
    pub city_name: Option<String>,

    #[serde(rename = "Country")]
    pub country: Option<Country>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Country {
    #[serde(rename = "IdentificationCode")]
    pub identification_code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct PartyTaxScheme {
    #[serde(rename = "CompanyID")]
    pub company_id: Option<String>,

    #[serde(rename = "TaxScheme")]
    pub tax_scheme: Option<TaxScheme>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct TaxScheme {
    #[serde(rename = "ID")]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Contact {
    #[serde(rename = "Telephone")]
    pub telephone: Option<String>,

    #[serde(rename = "ElectronicMail")]
    pub electronic_mail: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct InvoiceLine {
    #[serde(rename = "ID")]
    pub id: Option<String>,

    #[serde(rename = "InvoicedQuantity")]
    pub invoiced_quantity: Option<Quantity>,

    #[serde(rename = "LineExtensionAmount")]
    pub line_extension_amount: Option<Amount>,

    #[serde(rename = "Item")]
    pub item: Option<Item>,

    #[serde(rename = "Price")]
    pub price: Option<Price>,

    #[serde(rename = "TaxTotal")]
    pub tax_total: Option<TaxTotal>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Quantity {
    #[serde(rename = "@unitCode")]
    pub unit_code: Option<String>,

    #[serde(rename = "$text")]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Amount {
    #[serde(rename = "@currencyID")]
    pub currency_id: Option<String>,

    #[serde(rename = "$text")]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Item {
    #[serde(rename = "Name")]
    pub name: Option<String>,

    #[serde(rename = "Description")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Price {
    #[serde(rename = "PriceAmount")]
    pub price_amount: Option<Amount>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct TaxTotal {
    #[serde(rename = "TaxAmount")]
    pub tax_amount: Option<Amount>,

    #[serde(rename = "TaxSubtotal")]
    pub tax_subtotal: Option<TaxSubtotal>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct TaxSubtotal {
    #[serde(rename = "TaxableAmount")]
    pub taxable_amount: Option<Amount>,

    #[serde(rename = "TaxAmount")]
    pub tax_amount: Option<Amount>,

    #[serde(rename = "TaxCategory")]
    pub tax_category: Option<TaxCategory>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct TaxCategory {
    #[serde(rename = "Percent")]
    pub percent: Option<String>,

    #[serde(rename = "TaxScheme")]
    pub tax_scheme: Option<TaxScheme>,
}

/// ⚠ If using quick-xml, namespace prefixes like "cbc:" may break.
/// Consider removing "cbc:" in the rename fields.
#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct LegalMonetaryTotal {
    #[serde(rename = "cbc:LineExtensionAmount")]
    pub line_extension_amount: Option<Amount>,

    #[serde(rename = "cbc:TaxExclusiveAmount")]
    pub tax_exclusive_amount: Option<Amount>,

    #[serde(rename = "cbc:TaxInclusiveAmount")]
    pub tax_inclusive_amount: Option<Amount>,

    #[serde(rename = "cbc:PayableAmount")]
    pub payable_amount: Option<Amount>,
}
