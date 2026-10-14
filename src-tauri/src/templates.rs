//! Şablonlar: usr\tpl\<ad> varsa o, yoksa exe'ye gömülü varsayılan kullanılır.
//! Değişken sözdizimi Laragon ile aynıdır: `<<HOSTNAME>>`.

use crate::kcore::paths;

fn embedded(name: &str) -> Option<&'static str> {
    Some(match name {
        "httpd.conf.tpl" => include_str!("../resources/tpl/httpd.conf.tpl"),
        "apache.php.modphp.tpl" => include_str!("../resources/tpl/apache.php.modphp.tpl"),
        "apache.php.fcgi.tpl" => include_str!("../resources/tpl/apache.php.fcgi.tpl"),
        "apache.ssl.tpl" => include_str!("../resources/tpl/apache.ssl.tpl"),
        "apache.default.tpl" => include_str!("../resources/tpl/apache.default.tpl"),
        "apache.default.ssl.tpl" => include_str!("../resources/tpl/apache.default.ssl.tpl"),
        "apache.vhost.tpl" => include_str!("../resources/tpl/apache.vhost.tpl"),
        "apache.vhost.ssl.tpl" => include_str!("../resources/tpl/apache.vhost.ssl.tpl"),
        "apache.alias.tpl" => include_str!("../resources/tpl/apache.alias.tpl"),
        "nginx.conf.tpl" => include_str!("../resources/tpl/nginx.conf.tpl"),
        "nginx.fastcgi-php.tpl" => include_str!("../resources/tpl/nginx.fastcgi-php.tpl"),
        "nginx.default.tpl" => include_str!("../resources/tpl/nginx.default.tpl"),
        "nginx.server.tpl" => include_str!("../resources/tpl/nginx.server.tpl"),
        "nginx.alias.tpl" => include_str!("../resources/tpl/nginx.alias.tpl"),
        "my.ini.tpl" => include_str!("../resources/tpl/my.ini.tpl"),
        "mongod.conf.tpl" => include_str!("../resources/tpl/mongod.conf.tpl"),
        _ => return None,
    })
}

pub const NAMES: &[&str] = &[
    "httpd.conf.tpl",
    "apache.vhost.tpl",
    "apache.vhost.ssl.tpl",
    "nginx.conf.tpl",
    "nginx.server.tpl",
    "my.ini.tpl",
    "mongod.conf.tpl",
];

pub fn get(name: &str) -> String {
    let user = paths().usr().join("tpl").join(name);
    if let Ok(s) = std::fs::read_to_string(&user) {
        return s;
    }
    embedded(name).unwrap_or_default().to_string()
}

/// Kullanıcının düzenleyebilmesi için usr\tpl altına kopyalar ve yolunu döndürür.
pub fn materialize(name: &str) -> std::path::PathBuf {
    let user = paths().usr().join("tpl").join(name);
    if !user.exists() {
        let _ = std::fs::write(&user, embedded(name).unwrap_or_default());
    }
    user
}

pub fn render(tpl: &str, vars: &[(&str, String)]) -> String {
    let mut s = tpl.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("<<{k}>>"), v);
    }
    s.replace("\r\n", "\n")
}

pub fn render_named(name: &str, vars: &[(&str, String)]) -> String {
    render(&get(name), vars)
}
