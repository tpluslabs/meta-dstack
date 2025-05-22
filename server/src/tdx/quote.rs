use dcap_qvl::verify::VerifiedReport;
use tdx_attestation::{Attestation, InnerAttestationHelper};

// NB: we probably don't need this here.
pub async fn _get_verified_report(quote: &str, _appdata: &[u8]) -> anyhow::Result<VerifiedReport> {
    let attestation = Attestation::new();
    let verification = attestation.verify_quote(quote.to_string()).await;

    verification
}

pub async fn get_quote(report_data: &[u8]) -> anyhow::Result<String> {
    let attestation = Attestation::new();
    attestation.get_quote(report_data.to_vec()).await
}
