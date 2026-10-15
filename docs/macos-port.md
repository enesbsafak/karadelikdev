# macOS desteği (1.3.0) — devir belgesi

Hedef: KaradelikDev'in Windows'taki özelliklerinin tamamı macOS'ta da çalışsın; 1.3.0 iki platform için birlikte yayınlansın.
Bu belge Windows'ta başlatılan işi Mac'te devam ettirecek oturum içindir. Durum ve kararlar burada; kod `macos` dalında.

## Mac'te hazırlık

```bash
xcode-select --install
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install node
git clone https://github.com/enesbsafak/karadelikdev.git && cd karadelikdev && git checkout macos
npm install
cd src-tauri && cargo check        # ilk hedef: macOS'ta derlemek
```

## Yapılanlar (Windows'ta)

- `src-tauri/src/platform/` eklendi. `mod.rs` ortak arayüzü listeler; `windows.rs` mevcut davranışı içerir (util.rs, cli.rs, paths.rs, hosts.rs'ten taşındı, davranış değişmedi, Windows testleri geçiyor).
- `platform/macos.rs` yazıldı ama **hiç derlenmedi/test edilmedi**: `lsof` ile port sahipleri, `pkill`/`kill`, `osascript` ile yönetici izni ve Terminal, ayarlar `~/Library/Application Support/KaradelikDev/config.json`, kurulum tespiti `.app/Contents/MacOS`, varsayılan veri klasörü `~/KaradelikDev`.
- `util.rs` süreç yardımcılarını `platform`'dan yeniden dışa aktarır; çağıran kod değişmedi.

## Kararlar

| Konu | Karar | Neden |
|---|---|---|
| Paket kaynağı | **Homebrew** (`brew install php@8.3 httpd nginx mysql@8.4 mariadb postgresql@17 redis memcached mongodb/brew/mongodb-community mailpit node@22 ...`) | PHP, Apache, Nginx, MariaDB, Redis'in resmi macOS ikilileri yok. Servisler `brew services` ile değil, KaradelikDev'in kendi ürettiği ayarlarla çalıştırılır (Windows'taki gibi). |
| Sürüm düzeni | `bin/<kategori>/<sürüm>` klasörleri Homebrew'a **sembolik bağlantı** (ör. `bin/php/php-8.3 -> /opt/homebrew/opt/php@8.3`) | `versions.rs` ve menüler değişmeden çalışır. |
| Veri klasörü | `~/KaradelikDev` (sihirbazdan değiştirilebilir) | Boşluksuz, kullanıcıya ait. |
| Dağıtım | Tauri `.app` + `.dmg`; güncelleme `.app.tar.gz` indirip SHA-256 doğrulayarak `/Applications/KaradelikDev.app`'i değiştirir | Windows'taki setup akışının karşılığı. |
| İmza | Kullanıcı kararı bekliyor: Apple Developer hesabı (yıllık 99 $) ile Developer ID imzası + notarization; yoksa ad-hoc imza ve ilk açılışta "doğrulanamadı" uyarısı | |
| Port 80 | macOS 10.14+ root olmadan 80'i dinlemeye izin verir | Doğrulanmalı. |

## Yapılacaklar (Windows -> macOS karşılıkları)

Önce `cargo check` ile derlet; Windows'a özgü kodu `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` ile ayır.
`Cargo.toml`: `winreg` ve `windows-sys` `[target.'cfg(windows)'.dependencies]` altına alınmalı.

1. **Çalıştırılabilir adları**: `".exe"` geçen her yer `crate::platform::exe("httpd")` kullanmalı (`services/mod.rs` exe_name, `versions::exe/tool` çağrıları, `php.rs`, `projects.rs`, `services/mysql.rs`, `services/others.rs`, `services/phpcgi.rs`, `terminal.rs`, `tunnel.rs`, `actions.rs`, `menu.rs`, `commands.rs`). `grep -n '\.exe' src-tauri/src` ile listelenir.
2. **packages.rs / packages.conf**: macOS için ayrı katalog (`resources/defaults/packages.mac.conf`), kurulum `brew install <formül>` + `bin/<kat>/<sürüm>` bağlantısı. Homebrew yoksa sihirbaz kurulum komutunu Terminal'de açar.
3. **Apache**: Homebrew httpd modülleri `lib/httpd/modules` altında, MPM paylaşımlı modül (`mpm_prefork` mod_php için). `httpd.conf.tpl`'in macOS sürümü gerekir (`AcceptFilter`, `mpm_winnt` satırları yok). PHP: Homebrew `php-cgi` ile mevcut php-cgi havuzu (`services/phpcgi.rs`) çalışır; mod_php için `lib/httpd/modules/libphp.so`.
4. **php.rs**: eklentiler `.so` (`php_x.dll` değil); Homebrew PHP'de çoğu eklenti derlemenin içinde, menüdeki aç/kapa yalnız paylaşımlı olanlar için.
5. **MariaDB**: `mariadb-install-db --auth-root-authentication-method=normal` (yoksa root yalnız unix_socket ile girer, TCP parolasız giriş çalışmaz).
6. **hosts.rs**: `/etc/hosts` yazımı `platform::run_elevated` (osascript parola penceresi) + `dscacheutil -flushcache; killall -HUP mDNSResponder`.
7. **ssl.rs**: CA güveni `security add-trusted-cert -r trustRoot -k ~/Library/Keychains/login.keychain-db`, kaldırma `security remove-trusted-cert`.
8. **terminal.rs**: Terminal.app (`platform::open_console`) / iTerm; ortam dosyası `usr/kdenv.sh` (`.cmd` yerine), `user.sh`.
9. **winenv.rs** (PATH, sağ tık menüsü, otomatik başlatma): PATH için `~/.zprofile`'da işaretli blok; sağ tık menüsü için Finder Hızlı Eylemi (`~/Library/Services/*.workflow`); otomatik başlatma Tauri autostart eklentisi (LaunchAgent).
10. **services/winservice.rs** ("servis olarak çalıştır"): `~/Library/LaunchAgents/dev.karadelik.<servis>.plist` + `launchctl bootstrap gui/$UID`.
11. **acl.rs**: macOS'ta veri klasörü `chmod 700` denetimi.
12. **setup.rs**: VC++ denetimi yok; onun yerine Homebrew ve Xcode CLT denetimi. **importer.rs** (Laragon) macOS'ta gizlenir.
13. **updater.rs**: PowerShell trampolini yerine `/bin/sh` betiği: süreç bitince `.app` değiştir, `open -a`. `latest.json`'a platform alanları eklenmeli (ör. `windows`, `macos-aarch64`, `macos-x86_64` için ayrı url/sha256) ve eski istemciler bozulmamalı (`url`/`sha256` Windows setup'ı göstermeye devam eder).
14. **tauri.conf.json**: `bundle.targets` platforma göre (`nsis` Windows, `dmg`/`app` macOS); macOS simge (`icons/icon.icns`) ve `minimumSystemVersion`.
15. **Menü/kısayollar**: `Ctrl+Alt+T` macOS'ta `Cmd+Alt+T`; HeidiSQL/Notepad++ yerine Sequel Ace/VS Code; netstat yerine `lsof`.
16. **Testler**: `tests/functest.sh` macOS'ta çalışacak şekilde (`netstat -ano` yerine `lsof`, `.exe` yok); `scripts/release.ps1`'in macOS karşılığı `scripts/release.sh` (dmg + app.tar.gz + SHA256SUMS + latest.json güncellemesi).
17. Son adım Windows'ta: tam regresyon testi (Windows Sandbox önerilir), sonra 1.3.0 iki platform birlikte yayın.

## Bu depoda öğrenilmiş tuzaklar

- Apache + php-cgi (Windows): `SetHandler "proxy:balancer://ad/"` + `ProxyFCGISetEnvIf` ile `SCRIPT_FILENAME` düzeltmesi gerekiyor; macOS yollarında (`/Users/...`) bu düzeltme gereksiz olabilir, regex yalnız sürücü harfli yolları değiştirir.
- Yayın sırası: önce `main` push, sonra `gh release create` (yoksa etiket eski commit'e bağlanır).
- Commit'lerde yapay zekâ imzası/ortak yazar satırı kullanılmaz; yazar `enesbsafak` (GitHub noreply e-postası).
- `docs/laragon-analizi.md` yalnız yerelde kalır, depoya girmez.
- Kullanıcı oyun oynarken pencere/parola istemi açılmamalı; parola/UAC gerektiren testlerden önce sorulmalı.
