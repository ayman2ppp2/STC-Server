STC-Server: Sudanese Taxation Chamber Integration Prototype
STC-Server is a high-performance, secure backend prototype designed for the Sudanese Taxation Chamber (STC). It serves as the central clearinghouse and reporting authority for the national electronic invoicing system, ensuring that all B2B, B2C, and B2G transactions comply with national tax regulations and international data standards.
🌟 Core System Features
1. Electronic Invoicing Standard (UBL 2.1)
 * Strict XSD Validation: All incoming XML invoices are validated against official Universal Business Language (UBL) 2.1 schemas before processing.
 * Semantic Verification: Ensures that required tax fields, VAT rates, seller/buyer identifiers, and totals are structurally and mathematically correct.
2. Cryptographic Security & PKI (Public Key Infrastructure)
 * XAdES Digital Signatures: Implements XML Advanced Electronic Signatures (XAdES) to guarantee non-repudiation, document integrity, and authenticity of the issuer.
 * Taxpayer Onboarding via CSR: Taxpayers generate a private key locally and submit a Certificate Signing Request (CSR) to the STC-Server to receive an official X.509 cryptographic certificate for signing their invoices.
 * OpenSSL Integration: Native integration for high-speed cryptographic operations and certificate validation.
3. Dual-Mode Invoice Processing
The system supports the two primary operational models for tax authorities:
 * Clearance Model (B2B/B2G): Real-time invoice submission. The seller submits the invoice to the STC-Server, which validates and mathematically signs the clearance status before the invoice can be legally issued to the buyer.
 * Reporting Model (B2C): Deferred submission. Point-of-Sale (POS) systems generate simplified tax invoices locally and report them to the STC-Server in batches within a specified legal timeframe.
🏗 Technical Architecture & Stack
The system is built for extreme memory safety, high concurrency, and reliable database interactions.
 * Core Language: Rust 🦀
 * Web Framework: Actix-web (for high-throughput API routing and middleware)
 * Database: PostgreSQL
 * ORM / Query Builder: SQLx (featuring compile-time checked SQL queries)
 * Cryptography: OpenSSL bindings
 * Containerization: Docker & Docker Compose
📁 Repository Structure
STC-Server/
├── .github/workflows/   # CI/CD pipelines
├── .sqlx/               # Prepared SQLx query metadata for offline compilation
├── .vscode/             # Editor configurations
├── migrations/          # PostgreSQL database migration scripts
├── schemas/
│   └── UBL-2.1/xsd/     # Official XML Schema Definitions for UBL
├── scripts/             # Utility scripts for database and cert management
├── src/                 # Rust source code (Controllers, Models, Crypto, XML parsing)
├── invoice.xml          # Sample UBL 2.1 invoice for testing
├── test-csr.sh          # Script to test the CSR generation and onboarding flow
├── test_onboard.sh      # E2E taxpayer registration simulation
└── test.sh              # General API payload testing

🚀 Getting Started
Prerequisites
 * Rust Toolchain (stable)
 * PostgreSQL 15+
 * OpenSSL development headers (libssl-dev on Debian/Ubuntu)
 * Docker & Docker Compose (optional, for containerized runs)
Note: This repository is fully configured for cloud development environments like GitHub Codespaces. You can launch a Codespace to immediately get a working Rust environment with PostgreSQL running.
Local Installation & Setup
 * Clone the repository:
   git clone https://github.com/ayman2ppp2/STC-Server.git
cd STC-Server

 * Environment Configuration:
   Create a .env file in the root directory:
   DATABASE_URL=postgres://username:password@localhost:5432/stc_database
SERVER_HOST=127.0.0.1
SERVER_PORT=8080

 * Database Initialization:
   Ensure your PostgreSQL instance is running, then create the database and run migrations:
   cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx database create
sqlx migrate run

 * Run the Server:
   cargo run --release

📡 API Workflows (Testing the Prototype)
The repository includes shell scripts to simulate the complete taxpayer journey.
Step 1: Taxpayer Onboarding
Taxpayers must first register their ERP/POS system to obtain a cryptographic identity.
# Simulates submitting a CSR to the /onboard endpoint
./test_onboard.sh
# Validates the CSR processing specifically
./test-csr.sh

Step 2: Invoice Clearance
Submit a drafted XML invoice to the authority for validation.
# Simulates submitting invoice.xml to the /clearance endpoint
./test.sh

If successful, the server responds with a stamped XML containing the STC's digital signature, which can then be sent to the buyer.
🛣 Roadmap & Ongoing Development
Check the todo.md file for the active task list. Upcoming architectural phases include:
 * Completing the reporting endpoints for offline POS syncs.
 * Implementing rate-limiting and API gateway routing.
 * Enhancing the XAdES validation strictness against the Sudanese specific localized business rules.
 * Building out the administrative dashboard endpoints for tax inspectors.
🛡 License & Disclaimer
This is an architectural prototype developed for demonstration and structural planning. It is not yet intended for production deployment without a comprehensive security audit of the cryptographic implementations.
