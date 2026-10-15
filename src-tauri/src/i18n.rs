//! Arayüz metinleri. Aynı JSON dosyaları hem Rust menülerinde hem web arayüzünde kullanılır.
//! macOS'ta `<anahtar>.mac` varsa `<anahtar>` yerine o kullanılır (ör. "Windows servisi" -> "launchd").

use once_cell::sync::Lazy;
use std::collections::HashMap;

static TR: Lazy<HashMap<String, String>> =
    Lazy::new(|| serde_json::from_str(include_str!("../../ui/i18n/tr.json")).unwrap_or_default());
static EN: Lazy<HashMap<String, String>> =
    Lazy::new(|| serde_json::from_str(include_str!("../../ui/i18n/en.json")).unwrap_or_default());

pub const LANGUAGES: &[(&str, &str)] = &[("tr", "Türkçe"), ("en", "English")];

const PLATFORM_SUFFIX: Option<&str> = if cfg!(target_os = "macos") { Some(".mac") } else { None };

pub fn t(key: &str) -> String {
    let lang = crate::kcore::settings().language;
    let primary = if lang == "en" { &*EN } else { &*TR };
    if let Some(sfx) = PLATFORM_SUFFIX {
        let k = format!("{key}{sfx}");
        if let Some(v) = primary.get(&k).or_else(|| EN.get(&k)) {
            return v.clone();
        }
    }
    primary.get(key).or_else(|| EN.get(key)).cloned().unwrap_or_else(|| key.to_string())
}

/// `{0}`, `{1}` yer tutucularını doldurur.
pub fn tf(key: &str, args: &[&str]) -> String {
    let mut s = t(key);
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), a);
    }
    s
}

pub fn table(lang: &str) -> HashMap<String, String> {
    let mut m = EN.clone();
    if lang != "en" {
        m.extend(TR.clone());
    }
    if let Some(sfx) = PLATFORM_SUFFIX {
        let over: Vec<(String, String)> = m.iter().filter_map(|(k, v)| k.strip_suffix(sfx).map(|b| (b.to_string(), v.clone()))).collect();
        m.extend(over);
    }
    m
}

#[cfg(test)]
mod tests {
    #[test]
    fn platform_overlay() {
        let m = super::table("tr");
        let svc = m.get("menu.run_as_service").unwrap();
        assert_eq!(svc.contains("launchd"), cfg!(target_os = "macos"));
        // Her .mac anahtarının karşılığı iki dilde de var
        for (k, _) in super::EN.iter().filter(|(k, _)| k.ends_with(".mac")) {
            assert!(super::TR.contains_key(k), "{k}");
        }
    }
}
