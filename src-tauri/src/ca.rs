//! 本机自签根 CA：首次生成并落盘，供 MITM 现签各域名叶证书。
use hudsucker::certificate_authority::RcgenAuthority;
use hudsucker::rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use hudsucker::rustls::crypto::aws_lc_rs;

pub const CA_COMMON_NAME: &str = "Codex 保安 Local Root CA";

/// 若本机尚无 CA 则生成并落盘（cert + key PEM）。幂等。
pub fn ensure_ca() -> Result<(), String> {
    let cert_path = crate::paths::ca_cert_path();
    let key_path = crate::paths::ca_key_path();
    if cert_path.exists() && key_path.exists() {
        return Ok(());
    }
    let key = KeyPair::generate().map_err(|e| e.to_string())?;
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, CA_COMMON_NAME);
    dn.push(DnType::OrganizationName, "codex-baoan (local only)");
    params.distinguished_name = dn;
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let cert = params.self_signed(&key).map_err(|e| e.to_string())?;
    std::fs::write(&cert_path, cert.pem()).map_err(|e| e.to_string())?;
    std::fs::write(&key_path, key.serialize_pem()).map_err(|e| e.to_string())?;
    Ok(())
}

/// 从落盘 PEM 构建 hudsucker 的 CA（用于现签叶证书）。
pub fn build_authority() -> Result<RcgenAuthority, String> {
    ensure_ca()?;
    let cert_pem = std::fs::read_to_string(crate::paths::ca_cert_path()).map_err(|e| e.to_string())?;
    let key_pem = std::fs::read_to_string(crate::paths::ca_key_path()).map_err(|e| e.to_string())?;
    let key = KeyPair::from_pem(&key_pem).map_err(|e| e.to_string())?;
    let issuer = Issuer::from_ca_cert_pem(&cert_pem, key).map_err(|e| e.to_string())?;
    Ok(RcgenAuthority::new(issuer, 1_000, aws_lc_rs::default_provider()))
}
