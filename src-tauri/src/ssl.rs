//! SSL: Laragon tek kendinden imzalı sertifika üretip onu kök depoya ekler.
//! KaradelikDev bir yerel kök CA üretir, kullanıcı deposuna bir kez ekler (yönetici gerekmez)
//! ve her host için o CA ile imzalı sertifika üretir. OpenSSL binary'si gerekmez.

use crate::kcore::paths;
use crate::util::{hidden, run_capture};
use anyhow::{Context, Result};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType,
};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub const CA_NAME: &str = "KaradelikDev Local CA";

pub fn ca_crt() -> PathBuf {
    paths().ssl().join("karadelik-ca.crt")
}

fn ca_key() -> PathBuf {
    paths().ssl().join("karadelik-ca.key")
}

pub fn ensure_ca() -> Result<()> {
    if ca_crt().exists() && ca_key().exists() {
        return Ok(());
    }
    fs::create_dir_all(paths().ssl())?;
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, CA_NAME);
    dn.push(DnType::OrganizationName, "KaradelikDev");
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
    let now = time_now();
    params.not_before = now - time::Duration::days(1);
    params.not_after = now + time::Duration::days(3650);
    let key = KeyPair::generate()?;
    let cert = params.self_signed(&key)?;
    fs::write(ca_crt(), cert.pem())?;
    fs::write(ca_key(), key.serialize_pem())?;
    crate::kcore::log("Yerel kök CA oluşturuldu");
    Ok(())
}

fn time_now() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
}

fn issuer() -> Result<Issuer<'static, KeyPair>> {
    ensure_ca()?;
    let key = KeyPair::from_pem(&fs::read_to_string(ca_key())?)?;
    let pem = fs::read_to_string(ca_crt())?;
    Ok(Issuer::from_ca_cert_pem(&pem, key)?)
}

/// `etc\ssl\<host>.crt/.key` yoksa veya 300 günden eskiyse üretir. Üretildiyse true.
pub fn ensure_host_cert(host: &str) -> Result<bool> {
    let crt = paths().ssl().join(format!("{host}.crt"));
    let key = paths().ssl().join(format!("{host}.key"));
    let fresh = fs::metadata(&crt)
        .and_then(|m| m.modified())
        .map(|t| SystemTime::now().duration_since(t).unwrap_or_default() < Duration::from_secs(300 * 86400))
        .unwrap_or(false);
    if fresh && key.exists() {
        return Ok(false);
    }
    let iss = issuer()?;
    let mut sans = vec![host.to_string(), format!("*.{host}")];
    if host == "localhost" {
        sans = vec!["localhost".into(), "*.localhost".into()];
    }
    let mut params = CertificateParams::new(sans)?;
    if host == "localhost" {
        params.subject_alt_names.push(SanType::IpAddress("127.0.0.1".parse().unwrap()));
        params.subject_alt_names.push(SanType::IpAddress("::1".parse().unwrap()));
    }
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, host);
    dn.push(DnType::OrganizationName, "KaradelikDev");
    params.distinguished_name = dn;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.use_authority_key_identifier_extension = true;
    let now = time_now();
    params.not_before = now - time::Duration::days(1);
    // Tarayıcıların özel CA'lar için de uyguladığı üst sınırın altında kal
    params.not_after = now + time::Duration::days(397);
    let leaf_key = KeyPair::generate()?;
    let cert = params.signed_by(&leaf_key, &iss)?;
    fs::write(&crt, cert.pem()).with_context(|| crt.display().to_string())?;
    fs::write(&key, leaf_key.serialize_pem())?;
    Ok(true)
}

pub fn is_trusted() -> bool {
    let mut c = hidden("certutil");
    c.args(["-user", "-verifystore", "Root", CA_NAME]);
    run_capture(c).map(|(code, _)| code == 0).unwrap_or(false)
}

/// Kullanıcının "Güvenilen Kök Sertifika Yetkilileri" deposuna ekler. Windows bir onay penceresi gösterir.
pub fn trust() -> Result<()> {
    ensure_ca()?;
    let mut c = hidden("certutil");
    c.args(["-user", "-f", "-addstore", "Root"]).arg(ca_crt());
    let (code, out) = run_capture(c)?;
    if code != 0 {
        anyhow::bail!("certutil: {}", out.trim());
    }
    Ok(())
}

pub fn untrust() -> Result<()> {
    let mut c = hidden("certutil");
    c.args(["-user", "-delstore", "Root", CA_NAME]);
    let (code, out) = run_capture(c)?;
    if code != 0 {
        anyhow::bail!("certutil: {}", out.trim());
    }
    Ok(())
}

/// Tüm host sertifikalarını silip yeniden üretilmesini sağlar (CA korunur).
pub fn regenerate_all() -> Result<()> {
    for e in fs::read_dir(paths().ssl())?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if (n.ends_with(".crt") || n.ends_with(".key")) && !n.starts_with("karadelik-ca") {
            let _ = fs::remove_file(e.path());
        }
    }
    Ok(())
}
