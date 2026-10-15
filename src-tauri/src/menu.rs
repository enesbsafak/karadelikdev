//! Native popup menü. Ağaç her açılışta güncel durumdan yeniden kurulur;
//! tepsi menüsü servis durumu değiştiğinde yenilenir. Öğe kimlikleri `actions::dispatch`'e gider.

use crate::kcore::{app, paths, settings};
use crate::i18n::t;
use crate::services::{self, Svc, SvcStatus};
use crate::versions;
use tauri::menu::{CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, Submenu, SubmenuBuilder};
use tauri::{AppHandle, Wry};

pub enum Node {
    Item(String, String, bool),
    Check(String, String, bool),
    Sep,
    Sub(String, Vec<Node>),
}

fn item(id: impl Into<String>, text: impl Into<String>) -> Node {
    Node::Item(id.into(), text.into(), true)
}
fn disabled(text: impl Into<String>) -> Node {
    Node::Item(format!("noop:{}", rand_id()), text.into(), false)
}
fn check(id: impl Into<String>, text: impl Into<String>, on: bool) -> Node {
    Node::Check(id.into(), text.into(), on)
}
fn sub(text: impl Into<String>, items: Vec<Node>) -> Node {
    Node::Sub(text.into(), items)
}
fn rand_id() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}
fn open(path: &std::path::Path, text: impl Into<String>) -> Node {
    item(format!("open:{}", path.display()), text)
}

fn build_sub(app: &AppHandle, text: &str, nodes: &[Node]) -> tauri::Result<Submenu<Wry>> {
    let mut b = SubmenuBuilder::new(app, text);
    for n in nodes {
        b = match n {
            Node::Item(id, text, en) => b.item(&MenuItemBuilder::with_id(id.as_str(), text).enabled(*en).build(app)?),
            Node::Check(id, text, on) => b.item(&CheckMenuItemBuilder::with_id(id.as_str(), text).checked(*on).build(app)?),
            Node::Sep => b.separator(),
            Node::Sub(text, items) => b.item(&build_sub(app, text, items)?),
        };
    }
    b.build()
}

/// Tepsi / menü çubuğu simgesinin menüsü.
pub fn build(app: &AppHandle, st: &[SvcStatus]) -> tauri::Result<Menu<Wry>> {
    build_nodes(app, tree(st, true))
}

/// Penceredeki "Menü" düğmesi ve sağ tık menüsü.
pub fn build_popup(app: &AppHandle, st: &[SvcStatus]) -> tauri::Result<Menu<Wry>> {
    build_nodes(app, tree(st, false))
}

fn build_nodes(app: &AppHandle, nodes: Vec<Node>) -> tauri::Result<Menu<Wry>> {
    let mut b = MenuBuilder::new(app);
    for n in nodes {
        b = match n {
            Node::Item(id, text, en) => b.item(&MenuItemBuilder::with_id(id, text).enabled(en).build(app)?),
            Node::Check(id, text, on) => b.item(&CheckMenuItemBuilder::with_id(id, text).checked(on).build(app)?),
            Node::Sep => b.separator(),
            Node::Sub(text, items) => b.item(&build_sub(app, &text, &items)?),
        };
    }
    b.build()
}

/// Tepsi / menü çubuğu simgesi: Windows'ta renkli; macOS'ta menü çubuğuna uyan tek renkli şablon görüntü
/// (koyu/açık temaya göre sistem boyar). Çalışan servis varken dolu, yokken boş halka.
pub fn tray_icon(running: bool) -> &'static [u8] {
    match (cfg!(target_os = "macos"), running) {
        (true, true) => include_bytes!("../icons/tray-mac-on.png"),
        (true, false) => include_bytes!("../icons/tray-mac-off.png"),
        (false, true) => include_bytes!("../icons/tray-on.png"),
        (false, false) => include_bytes!("../icons/tray-off.png"),
    }
}

/// macOS menü çubuğu: uygulama menüsü, Düzen (giriş alanlarında ⌘C/⌘V için gerekli), KaradelikDev menü ağacı, Pencere.
#[cfg(target_os = "macos")]
pub fn app_menu(app: &AppHandle, st: &[SvcStatus]) -> tauri::Result<Menu<Wry>> {
    use tauri::menu::AboutMetadataBuilder;
    let about = AboutMetadataBuilder::new()
        .name(Some("KaradelikDev"))
        .version(Some(env!("CARGO_PKG_VERSION")))
        .copyright(Some("© 2026 enesbsafak · MIT"))
        .website(Some("https://github.com/enesbsafak/karadelikdev"))
        .build();
    let app_sub = SubmenuBuilder::new(app, "KaradelikDev")
        .about_with_text(t("mac.about"), Some(about))
        .separator()
        .item(&MenuItemBuilder::with_id("prefs", t("menu.preferences")).accelerator("CmdOrCtrl+,").build(app)?)
        .item(&MenuItemBuilder::with_id("app_update", t("menu.app_update_check")).build(app)?)
        .separator()
        .services_with_text(t("mac.services"))
        .separator()
        .hide_with_text(t("mac.hide"))
        .hide_others_with_text(t("mac.hide_others"))
        .show_all_with_text(t("mac.show_all"))
        .separator()
        // Çıkış servisleri durdurur (Menü > Çıkış ile aynı)
        .item(&MenuItemBuilder::with_id("exit", t("mac.quit")).accelerator("CmdOrCtrl+Q").build(app)?)
        .build()?;
    let edit = SubmenuBuilder::new(app, t("mac.edit"))
        .undo_with_text(t("mac.undo"))
        .redo_with_text(t("mac.redo"))
        .separator()
        .cut_with_text(t("mac.cut"))
        .copy_with_text(t("mac.copy"))
        .paste_with_text(t("mac.paste"))
        .select_all_with_text(t("mac.select_all"))
        .build()?;
    let main = build_sub(app, &t("btn.menu"), &tree(st, false))?;
    let window = SubmenuBuilder::new(app, t("mac.window"))
        .minimize_with_text(t("mac.minimize"))
        .close_window_with_text(t("mac.close_window"))
        .separator()
        .item(&MenuItemBuilder::with_id("show", "KaradelikDev").build(app)?)
        .item(&MenuItemBuilder::with_id("projects", t("projects.title")).build(app)?)
        .item(&MenuItemBuilder::with_id("db", t("db.title")).build(app)?)
        .item(&MenuItemBuilder::with_id("packages", t("packages.title")).build(app)?)
        .item(&MenuItemBuilder::with_id("logs", t("logs.title")).build(app)?)
        .build()?;
    MenuBuilder::new(app).item(&app_sub).item(&edit).item(&main).item(&window).build()
}

/// Tepsi menüsünü ve ikonunu yeniler (macOS'ta menü çubuğundaki uygulama menüsünü de).
pub fn refresh(st: &[SvcStatus]) {
    let Some(app) = app() else { return };
    let app2 = app.clone();
    let st = st.to_vec();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = app2.tray_by_id("main") {
            if let Ok(menu) = build(&app2, &st) {
                let _ = tray.set_menu(Some(menu));
            }
            let on = st.iter().any(|s| s.running);
            if let Ok(img) = tauri::image::Image::from_bytes(tray_icon(on)) {
                let _ = tray.set_icon(Some(img));
                let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
            }
        }
        #[cfg(target_os = "macos")]
        if let Ok(m) = app_menu(&app2, &st) {
            let _ = app2.set_menu(m);
        }
    });
}

fn version_menu(cat: &str) -> Node {
    let cur = versions::current(cat);
    let list = versions::installed(cat);
    let title = match &cur {
        Some(v) => format!("{} [{v}]", t("menu.version")),
        None => t("menu.version"),
    };
    let mut items: Vec<Node> = list.iter().map(|v| check(format!("ver:{cat}:{v}"), v.clone(), Some(v) == cur.as_ref())).collect();
    if items.is_empty() {
        items.push(disabled(t("menu.not_installed")));
    }
    items.push(Node::Sep);
    items.push(open(&paths().bin().join(cat), format!("dir: {}", crate::platform::rel(&["bin", cat]))));
    items.push(item(format!("quickadd:{cat}"), t("menu.quick_add_more")));
    sub(title, items)
}

fn svc_head(st: &[SvcStatus], svc: Svc) -> Vec<Node> {
    let s = st.iter().find(|x| x.id == svc.id());
    let running = s.map(|x| x.running).unwrap_or(false);
    let mut v = vec![];
    if running {
        v.push(item(format!("svc:{}:stop", svc.id()), format!("{} {}", t("btn.stop"), svc.title())));
        v.push(item(format!("svc:{}:reload", svc.id()), t("btn.reload")));
    } else {
        v.push(item(format!("svc:{}:start", svc.id()), format!("{} {}", t("btn.start"), svc.title())));
    }
    v.push(version_menu(svc.category()));
    if services::winservice::supported(svc) {
        v.push(check(format!("svc:{}:winsvc", svc.id()), t("menu.run_as_service"), services::winservice::installed(svc)));
    }
    v
}

fn tree(st: &[SvcStatus], tray: bool) -> Vec<Node> {
    let p = paths();
    let s = settings();
    let projects = crate::vhosts::projects();
    let mut root = vec![];

    // macOS: menü çubuğu simgesine tıklayınca menü açılır (Windows'taki sol tık = pencere yerine ilk öğe)
    if cfg!(target_os = "macos") && tray {
        root.push(item("show", t("menu.show_window")));
        root.push(Node::Sep);
    }

    // ---- KaradelikDev
    let mut procs: Vec<Node> = crate::procfile::list().into_iter().map(|x| item(format!("proc:{}", x.name), x.name)).collect();
    procs.push(Node::Sep);
    procs.push(item(format!("edit:{}", crate::procfile::path().display()), "Procfile"));
    procs.push(item("proc_closeall", t("menu.close_all")));
    root.push(sub(
        "KaradelikDev",
        vec![
            item("web", t("btn.web")),
            item("db", t("btn.database")),
            item("projects", t("projects.title")),
            item("packages", t("packages.title")),
            item("logs", t("logs.title")),
            item("terminal", format!("{}  ({})", t("btn.terminal"), crate::platform::TERMINAL_SHORTCUT)),
            item("root", t("btn.root")),
            Node::Sep,
            sub("Procfile", procs),
            Node::Sep,
            open(&p.usr(), "dir: usr"),
            item(format!("edit:{}", crate::terminal::user_script_path().display()), t("menu.startup_file")),
            item(format!("edit:{}", p.ini().display()), t("menu.settings_file")),
            item(format!("edit:{}", p.usr().join("crontab").display()), "crontab"),
            Node::Sep,
            item(format!("edit:{}", p.log_file().display()), "karadelik.log"),
            open(&p.logs(), "dir: logs"),
            Node::Sep,
            match crate::updater::available() {
                Some(l) => item("app_update", crate::i18n::tf("menu.app_update_to", &[&l.version])),
                None => item("app_update", t("menu.app_update_check")),
            },
        ]
        .into_iter()
        .chain(crate::updater::rollback_available().then(|| item("app_rollback", t("menu.app_rollback"))))
        .collect(),
    ));

    // ---- www
    let mut www: Vec<Node> = projects.iter().map(|pr| item(format!("site:{}", pr.name), pr.host.clone())).collect();
    if www.is_empty() {
        www.push(disabled(t("menu.no_project")));
    }
    www.push(Node::Sep);
    www.push(item("localhost", "localhost"));
    www.push(open(std::path::Path::new(&s.document_root), t("btn.root")));
    www.push(item("docroot_switch", t("menu.switch_docroot")));
    root.push(sub("www", www));

    // ---- Paylaş
    let mut share: Vec<Node> = projects.iter().map(|pr| item(format!("share:{}", pr.name), pr.host.clone())).collect();
    if let Some((host, url)) = crate::tunnel::current() {
        share.insert(0, item("share_show", format!("● {host} → {url}")));
        share.insert(1, item("sharestop", t("menu.share_stop")));
        share.insert(2, Node::Sep);
    }
    share.push(Node::Sep);
    share.push(check("tunnel:cloudflared", "Cloudflare Tunnel", s.tunnel != "ngrok"));
    share.push(check("tunnel:ngrok", "ngrok", s.tunnel == "ngrok"));
    share.push(item("ngrok_token", t("menu.ngrok_token")));
    root.push(sub(t("menu.share"), share));

    // ---- Hızlı uygulama
    let mut quick: Vec<Node> = crate::quickapp::manifest().templates.into_iter().map(|tp| item(format!("quick:{}", tp.name), tp.name)).collect();
    quick.push(Node::Sep);
    let del: Vec<Node> = projects.iter().map(|pr| item(format!("delproj:{}", pr.name), pr.name.clone())).collect();
    let clone: Vec<Node> = projects.iter().map(|pr| item(format!("cloneproj:{}", pr.name), pr.name.clone())).collect();
    quick.push(sub(t("menu.delete_project"), if del.is_empty() { vec![disabled(t("menu.no_project"))] } else { del }));
    quick.push(sub(t("menu.clone_project"), if clone.is_empty() { vec![disabled(t("menu.no_project"))] } else { clone }));
    quick.push(Node::Sep);
    quick.push(item(format!("edit:{}", crate::quickapp::manifest_path().display()), "sites.conf"));
    root.push(sub(t("menu.quick_app"), quick));

    // ---- Araçlar
    let mut qa: Vec<Node> = vec![];
    let mut last_group = String::new();
    for pkg in crate::packages::list() {
        if pkg.group != last_group && !qa.is_empty() {
            qa.push(Node::Sep);
        }
        last_group = pkg.group.clone();
        let text = if pkg.installed { format!("✓ {}", pkg.name) } else { pkg.name.clone() };
        qa.push(Node::Item(format!("pkg:{}", pkg.name), text, !pkg.installed));
    }
    qa.push(Node::Sep);
    qa.push(item(format!("edit:{}", crate::packages::conf_path().display()), "packages.conf"));
    let mut tools = vec![sub(t("menu.quick_add"), qa), item("setup", t("menu.setup_wizard"))];
    if let Some(lg) = crate::importer::find_laragon() {
        tools.push(item("import_laragon", format!("{} ({})", t("menu.import_laragon"), lg.display())));
    }
    tools.push(Node::Sep);
    for (id, cat, exe, title) in [
        ("tool:heidisql", "heidisql", "heidisql.exe", "HeidiSQL"),
        ("tool:dbeaver", "dbeaver", "dbeaver.exe", "DBeaver"),
        ("tool:notepad++", "notepad++", "notepad++.exe", "Notepad++"),
        ("tool:vscode", "vscode", "Code.exe", "VS Code"),
    ] {
        if cfg!(windows) && versions::tool(cat, exe).is_some() {
            tools.push(item(id, title));
        }
    }
    // macOS: /Applications altındaki araçlar
    for (id, app, title) in [
        ("tool:sequelace", "Sequel Ace.app", "Sequel Ace"),
        ("tool:tableplus", "TablePlus.app", "TablePlus"),
        ("tool:dbeaver", "DBeaver.app", "DBeaver"),
        ("tool:vscode", "Visual Studio Code.app", "VS Code"),
    ] {
        if cfg!(target_os = "macos") && std::path::Path::new("/Applications").join(app).exists() {
            tools.push(item(id, title));
        }
    }
    tools.push(item("netstat", if cfg!(windows) { "netstat" } else { "lsof (portlar)" }));
    tools.push(sub(
        "SSL",
        vec![
            check("ssl_trust", t("menu.ssl_trust"), crate::ssl::is_trusted()),
            item("ssl_regen", t("menu.ssl_regen")),
            item("certmgr", t("menu.cert_manager")),
            Node::Sep,
            open(&p.ssl(), "dir: ssl"),
            item("cacert_update", t("menu.cacert_update")),
        ],
    ));
    tools.push(sub(
        "Path",
        vec![
            item("path_add", t("menu.path_add")),
            item("path_remove", t("menu.path_remove")),
            Node::Sep,
            item("path_manage", t("menu.path_manage")),
        ],
    ));
    tools.push(sub(
        t("menu.context_menu"),
        vec![
            check("ctx_toggle", t("menu.context_terminal"), crate::winenv::context_menu_installed()),
        ],
    ));
    tools.push(item("hosts_edit", t("menu.edit_hosts")));
    let mut tpls: Vec<Node> = crate::templates::NAMES.iter().map(|n| item(format!("tpl:{n}"), n.to_string())).collect();
    tpls.push(Node::Sep);
    tpls.push(open(&p.usr().join("tpl"), format!("dir: {}", crate::platform::rel(&["usr", "tpl"]))));
    tools.push(sub(t("menu.templates"), tpls));
    root.push(sub(t("menu.tools"), tools));
    root.push(Node::Sep);

    // ---- Apache
    let a_cfg = s.svc("apache");
    let mut apache = svc_head(st, Svc::Apache);
    let sites: Vec<Node> = list_dir(&p.etc().join("apache2").join("sites-enabled")).into_iter().map(|f| item(format!("edit:{}", f.display()), file_name(&f))).collect();
    apache.extend([
        Node::Sep,
        item(format!("edit:{}", services::apache::conf_path().display()), "httpd.conf"),
        sub("sites-enabled", with_dir(sites, &p.etc().join("apache2").join("sites-enabled"))),
        sub("SSL", vec![check("ssl:apache", t("menu.enabled"), a_cfg.ssl), item("ssl_trust_only", t("menu.ssl_trust"))]),
        item(format!("edit:{}", services::apache::error_log().display()), "error.log"),
        item(format!("edit:{}", services::apache::access_log().display()), "access.log"),
        Node::Sep,
        open(&p.etc().join("apache2"), format!("dir: {}", crate::platform::rel(&["etc", "apache2"]))),
        open(&p.etc().join("apache2").join("alias"), "dir: alias"),
        item("vhosts_delete_auto", t("menu.delete_auto")),
    ]);
    root.push(sub("Apache", apache));

    // ---- Nginx
    let n_cfg = s.svc("nginx");
    let mut nginx = svc_head(st, Svc::Nginx);
    let nsites: Vec<Node> = list_dir(&p.etc().join("nginx").join("sites-enabled")).into_iter().map(|f| item(format!("edit:{}", f.display()), file_name(&f))).collect();
    nginx.extend([
        Node::Sep,
        item(format!("edit:{}", services::nginx::conf_path().display()), "nginx.conf"),
        sub("sites-enabled", with_dir(nsites, &p.etc().join("nginx").join("sites-enabled"))),
        sub("SSL", vec![check("ssl:nginx", t("menu.enabled"), n_cfg.ssl)]),
        item(format!("edit:{}", p.logs().join("nginx_error.log").display()), "error.log"),
        item(format!("edit:{}", p.logs().join("nginx_access.log").display()), "access.log"),
        Node::Sep,
        open(&p.etc().join("nginx"), format!("dir: {}", crate::platform::rel(&["etc", "nginx"]))),
    ]);
    root.push(sub("Nginx", nginx));

    // ---- MySQL
    let mut my = vec![];
    for (alias, _) in crate::vhosts::app_aliases() {
        if ["phpmyadmin", "adminer"].contains(&alias.as_str()) || alias.starts_with("phpmyadmin") {
            my.push(item(format!("url:{}/{alias}/", crate::vhosts::localhost_url()), alias));
        }
    }
    if versions::tool("heidisql", "heidisql.exe").is_some() {
        my.push(item("tool:heidisql", "HeidiSQL"));
    } else if let Some(name) = crate::actions::db_tool_name() {
        my.push(item("tool:dbtool", name));
    }
    if !my.is_empty() {
        my.push(Node::Sep);
    }
    my.extend(svc_head(st, Svc::Mysql));
    my.extend([
        Node::Sep,
        item("db_manager", t("db.title")),
        item("mysql:createdb", t("menu.create_db")),
        item("mysql:chpw", t("menu.change_root_pw")),
        item("mysql:getpw", t("menu.get_root_pw")),
        item("mysql:backup", t("menu.backup_all")),
        Node::Sep,
        item("mysql:reset", t("menu.reset_root_pw")),
        Node::Sep,
    ]);
    if let Some(ini) = services::mysql::ini_path() {
        my.push(item(format!("edit:{}", ini.display()), "my.ini"));
    }
    my.push(item(format!("edit:{}", services::mysql::log_path().display()), "mysqld.log"));
    if let Some(d) = services::mysql::data_dir() {
        my.push(open(&d, "dir: data"));
    }
    my.push(open(&crate::mysqlops::backup_dir(), "dir: backup"));
    root.push(sub("MySQL", my));

    // ---- PostgreSQL
    let mut pg = svc_head(st, Svc::Postgresql);
    pg.extend([Node::Sep, item("pg:chpw", t("menu.pg_password")), item("pg:getpw", t("menu.pg_get_password")), Node::Sep]);
    if let Some(d) = services::others::pg_data() {
        pg.push(item(format!("edit:{}", d.join("postgresql.conf").display()), "postgresql.conf"));
        pg.push(open(&d, "dir: data"));
    }
    pg.push(item(format!("edit:{}", services::others::pg_log().display()), "postgresql.log"));
    root.push(sub("PostgreSQL", pg));

    // ---- MongoDB
    let mut mg = svc_head(st, Svc::Mongodb);
    mg.extend([
        Node::Sep,
        item(format!("edit:{}", services::others::mongo_conf().display()), "mongod.conf"),
        item(format!("edit:{}", p.logs().join("mongod.log").display()), "mongod.log"),
    ]);
    root.push(sub("MongoDB", mg));
    root.push(Node::Sep);

    // ---- Mailpit
    let mut mp = svc_head(st, Svc::Mailpit);
    mp.extend([Node::Sep, item(format!("url:{}", services::others::mailpit_ui_url()), "Web UI")]);
    root.push(sub("Mailpit", mp));

    // ---- Redis
    let mut rd = svc_head(st, Svc::Redis);
    rd.push(Node::Sep);
    if crate::vhosts::app_aliases().iter().any(|(a, _)| a == "redis") {
        rd.push(item(format!("url:{}/redis/", crate::vhosts::localhost_url()), "Web admin"));
    }
    rd.push(item("redis_cli", "redis-cli"));
    rd.push(item(format!("edit:{}", services::others::redis_conf().display()), "redis.conf"));
    root.push(sub("Redis", rd));

    // ---- Memcached
    let mut mc = svc_head(st, Svc::Memcached);
    if crate::vhosts::app_aliases().iter().any(|(a, _)| a == "memcached") {
        mc.push(Node::Sep);
        mc.push(item(format!("url:{}/memcached/", crate::vhosts::localhost_url()), "Web admin"));
    }
    root.push(sub("Memcached", mc));
    root.push(Node::Sep);

    // ---- PHP
    let mut php = svc_head(st, Svc::PhpServer);
    let quick: Vec<Node> = crate::php::quick_settings().into_iter().map(|(k, label, on)| check(format!("php:quick:{k}"), label, on)).collect();
    let exts: Vec<Node> = crate::php::extensions().into_iter().map(|(n, on)| check(format!("php:ext:{n}"), n, on)).collect();
    php.extend([
        Node::Sep,
        sub(t("menu.quick_settings"), if quick.is_empty() { vec![disabled(t("menu.not_installed"))] } else { quick }),
        sub(t("menu.extensions"), if exts.is_empty() { vec![disabled(t("menu.not_installed"))] } else { exts }),
    ]);
    if let Some(ini) = crate::php::ini_path() {
        php.push(item(format!("edit:{}", ini.display()), "php.ini"));
    }
    if let Some(d) = crate::php::dir() {
        php.push(open(&d.join("ext"), "dir: ext"));
    }
    php.extend([
        item(format!("edit:{}", p.logs().join("php_errors.log").display()), "php_errors.log"),
        Node::Sep,
        item("php:sendmail", t("menu.get_sendmail")),
        item("php:checkupdate", t("menu.check_php_update")),
    ]);
    root.push(sub("PHP", php));

    // ---- Diğer çalışma ortamları
    for (cat, title) in [("nodejs", "Node.js"), ("python", "Python"), ("go", "Go"), ("ruby", "Ruby"), ("java", "Java")] {
        if !versions::installed(cat).is_empty() || cat == "nodejs" || cat == "python" {
            root.push(sub(title, vec![version_menu(cat)]));
        }
    }
    root.push(Node::Sep);

    // ---- Alt kısım
    let any_running = st.iter().any(|x| x.running);
    if any_running {
        root.push(item("stopall", t("btn.stop_all")));
    } else {
        root.push(item("startall", t("btn.start_all")));
    }
    root.push(Node::Sep);
    let mut prof: Vec<Node> = versions::profiles().into_iter().map(|n| check(format!("profile:{n}"), n.clone(), n == s.profile)).collect();
    prof.extend([Node::Sep, item("profile_new", t("menu.new_profile")), open(&p.profiles(), "dir: profile")]);
    root.push(sub(format!("{} [{}]", t("menu.profile"), s.profile), prof));
    root.push(item("prefs", t("menu.preferences")));
    root.push(item("exit", t("menu.exit")));
    root
}

fn list_dir(d: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(d).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect()).unwrap_or_default();
    v.sort();
    v
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

fn with_dir(mut v: Vec<Node>, dir: &std::path::Path) -> Vec<Node> {
    if v.is_empty() {
        v.push(disabled("—"));
    }
    v.push(Node::Sep);
    v.push(open(dir, "dir: sites-enabled"));
    v
}
