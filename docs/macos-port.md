# macOS desteği (1.3.0) — devir belgesi

Hedef: KaradelikDev'in Windows'taki özelliklerinin tamamı macOS'ta da çalışsın; 1.3.0 iki platform için birlikte yayınlansın.
Kod `macos` dalında. Bu belge durumu, kararları ve sıradaki adımları tutar.

## Durum (15 Ekim 2026): 1.3.0 iki platform için yayımlandı

GitHub Actions (`.github/workflows/platform-tests.yml`, `macos` dalındaki her push'ta) — çalışma 38786660110, hepsi yeşil:

| Koşucu | Birim | Uçtan uca (`functest.sh`) | Ek servisler | Paket |
|---|---|---|---|---|
| Windows (windows-latest) | geçti | 78/78 | 12/12 (PostgreSQL 18, MongoDB 8, MariaDB 11.8) | — |
| macOS Intel (macos-15-intel, Sequoia) | geçti | 79/79 | 8/8 (MariaDB: Homebrew'da Sequoia/Intel hazır paketi yok, atlandı) | x86_64 `.app` + `.dmg` |
| macOS Apple Silicon (macos-latest, Tahoe) | geçti | 79/79 | 12/12 | arm64 `.app` + `.dmg` |

CI'ın Windows'ta bulduğu ve düzeltilen önceden var olan sorunlar: katalogdaki msys2 Redis derlemesi `/d/...` yolunu tanımıyordu
(yapılandırma yolu artık göreli), `initdb` önceden oluşturulmuş veri klasörünün izinlerini değiştiremeyebiliyordu (klasörü artık
initdb oluşturur), MongoDB'nin ilk açılışı 20 sn'yi aşabiliyordu (60 sn). macOS'ta bulunan: `localhost` ile bağlanan PHP'nin
Homebrew'un `/tmp/mysql.sock`'una gitmesi (artık `mysqli/pdo_mysql.default_socket` KaradelikDev MySQL'i), Homebrew'un formülü
kurup bağlayamadığında kurulumun başarısız sayılması (artık `opt/<formül>` varsa kullanılır).

### Mac'te yerel doğrulama

- `cargo check`, `cargo test --lib` (22 test) ve `npm run build` (ad-hoc imzalı `.app` 9,7 MB + `.dmg` 5,4 MB) macOS'ta
  (Apple Silicon, macOS 27) geçiyor. Windows hedefi Mac'ten `cargo check --target x86_64-pc-windows-gnu --tests` ile
  uyarısız derleniyor (mingw-w64; yalnız derleme denetimi, Windows'ta çalıştırılmadı).
- Uçtan uca testler gerçek Homebrew paketleriyle macOS'ta çalıştırıldı (uygulama `KARADELIK_ROOT` + `KARADELIK_HOSTS` ile):
  - `bash tests/functest.sh`: 80 kontrol (servisler, güvenlik varsayılanları, veritabanı yöneticisi, otomatik vhost + hosts,
    proje başına PHP ve Node proxy, Hızlı uygulama + WordPress, Apache/Nginx/php-cgi izleyici, Redis/Memcached/PHP sunucusu,
    PHP ayarları ve sürüm değişimi, Mailpit, SSL, profiller, paket kurulumu, PATH, Finder Hızlı Eylemleri, CLI).
  - `bash tests/functest-extra-services.sh`: 12 kontrol; PostgreSQL, MongoDB (mongodb/brew tap'i), MariaDB'ye geçiş ve geri dönüş.
  - Elle (betikle) doğrulananlar: Procfile Terminal/iTerm'de KaradelikDev ortamıyla çalışıp "Tümünü kapat" ile kapanıyor;
    KaradelikDev terminalinde `php`/`composer`/`mysql` seçili sürümden geliyor (Herd/Homebrew PHP'sinden önce), normal kabuk
    etkilenmiyor; Finder Hızlı Eylemi `automator` ile çalıştırıldı (seçilen klasörde terminal); launchd servisi
    kur → durdur → başlat → kaldır; uygulama içi güncelleme `.app` paketinde yerel sunucuyla (indirme, SHA-256, `.app` değişimi,
    `--updated` ile servislerin geri gelmesi; bozuk özet reddediliyor); pencereler ve menü çubuğu ekran görüntüsüyle.
  - Parola isteyenler (kullanıcı onayıyla, gerçek sistemde): yerel CA'nın anahtar zincirine eklenip `demo.test` sertifikasının
    sistemce geçerli sayılması ve kaldırılması (CA anahtar zincirinden de siliniyor); yeni proje klasörünün `/etc/hosts`'a
    parola penceresiyle yazılması (`root:wheel 644` korunuyor), `http://kdhosts.test`'in açılması, projeler silinince bloğun
    kaldırılıp dosyanın birebir eski haline dönmesi.
  - Oturum açılışında çalıştırma: LaunchAgent (`--autostart`) yazılıyor, launchd uygulamayı başlatıyor, `--uninstall` siliyor.
  - Paylaşım: cloudflared (Homebrew, uygulama içinden kuruldu) ile genel adres `demo` projesini döndürdü, durdurulunca kapandı.
  - Intel: `npm run build -- --target x86_64-apple-darwin` → x86_64 `.app` (imza geçerli) + `_x64.dmg`. Bu Mac'te Rosetta
    olmadığı için çalıştırılmadı.
- Windows kodu davranış değişmeden korunuyor; Windows'a özgü dosyalar `winenv/windows.rs`, `acl/windows.rs`,
  `services/winservice/windows.rs` olarak taşındı (git rename). **Windows'ta henüz çalıştırılıp test edilmedi** (aşağıda 1. adım).
- macOS paket kimliği `dev.karadelik.desktop` (`tauri.macos.conf.json`); Windows'taki `dev.karadelik.app` değişmedi
  (`.app` ile biten kimlik macOS'ta paket uzantısıyla çakışır, Tauri uyarır).

## Yapılanlar

Platform katmanı (`src-tauri/src/platform/`): `exe()`, `join_path()` (PATH ayırıcı), `LINE_END`, `TERMINAL_SHORTCUT`, `rel()`.
`macos.rs`: `lsof` ile port sahipleri, `kill_tree` (alt süreçler + SIGTERM/SIGKILL, zombi denetimi `ps` ile), `osascript`
parola penceresi (`run_elevated_shell`), Terminal/iTerm (`.command` dosyası `open -a` ile açılır, AppleScript izni gerekmez),
Homebrew öneki/Cellar tespiti, `.app` paketi tespiti, Finder'dan açılınca eksik PATH'in tamamlanması.

| Windows | macOS karşılığı | Dosya |
|---|---|---|
| `.exe` adları | `platform::exe()`; Homebrew düzeni `x-8.4/bin/php` | `services/*`, `php.rs`, `terminal.rs`, `tunnel.rs`… |
| `packages.conf` (zip URL'leri) | `packages.mac.conf`: `ad=brew:formül`; kurulum `brew install` + `bin/<kat>/<ad>-<x.y>` → `opt/<formül>` bağlantısı; kurulu formül indirmeden bağlanır; üçüncü taraf tap (mongodb/brew) önce eklenir; `{arch}` (ngrok) | `packages.rs` |
| Apache Lounge `httpd.conf.tpl` | `resources/tpl/mac/httpd.conf.tpl`: mpm_prefork + unixd, ServerRoot `lib/httpd`, mime.types Homebrew `etc/httpd`, `-DFOREGROUND`, `DefaultRuntimeDir` kökte | `services/apache.rs`, `templates.rs` |
| `php8apache2_4.dll` | Homebrew `lib/httpd/modules/libphp.so` (mod_php) | `php.rs` |
| `php_x.dll`, `extension_dir` | `extension_dir` yönetilmez (Homebrew varsayılanı = pecl); listede `.so`'lar; opcache conf.d ile yüklü, aç/kapa `opcache.enable` | `php.rs` |
| php.ini kaynağı sürüm klasöründe | `<önek>/etc/php/<x.y>/php.ini(-development/-production)` | `php.rs` |
| fcgi `SCRIPT_FILENAME` düzeltmesi (sürücü harfi) | `proxy:balancer://<ad>` öneki kaldırılır (`templates::fcgi_script_fix`) | `templates.rs`, `vhosts.rs` |
| MySQL `my.ini.tpl` | `mac/my.ini.tpl`: kökte Unix soketi (uzun yolda `/tmp/karadelikdev-<hash>.sock`), `loose-mysqlx=OFF` | `services/mysql.rs` |
| `mariadb-install-db.exe` | Unix betiği `--auth-root-authentication-method=normal --skip-test-db` (TCP'den parolasız root) | `services/mysql.rs` |
| Nginx `conf/` | Homebrew `etc/nginx` (mime.types, fastcgi.conf); `mac/nginx.conf.tpl` `daemon off;` | `services/nginx.rs` |
| PostgreSQL | `-k <tmp>` (uzun yolda `/tmp`): Homebrew'un /tmp soket kilidiyle çakışmaz | `services/others.rs` |
| hosts (UAC + `--apply-hosts`) | parola penceresi + `cat > /etc/hosts` (izinler korunur) + `dscacheutil -flushcache; killall -HUP mDNSResponder`; düzenleme Terminal'de `sudo nano` | `hosts.rs` |
| certutil (kullanıcı deposu) | `security add-trusted-cert -r trustRoot -k login.keychain-db`; denetim `security verify-cert`; kaldırma `remove-trusted-cert` + `delete-certificate` | `ssl.rs` |
| `kdenv.cmd`, `user.cmd`, wt/cmd/PowerShell/Cmder | `kdenv.sh`, `user.sh`; Terminal.app / iTerm2; kullanıcının kabuğu kendi rc'sini okur, sonra `kdenv.sh` (zsh: geçici `ZDOTDIR`, bash: `--rcfile`) | `terminal.rs`, `platform/macos.rs` |
| Procfile konsolları | her girdi Terminal'de; "Tümünü kapat" `tmp/procfile/<ad>.pid` ile | `procfile.rs` |
| Kullanıcı PATH'i (HKCU) | `~/.zprofile` (bash: `~/.bash_profile`) işaretli blok; kaldırınca dosya eski haline döner | `winenv/macos.rs` |
| Explorer sağ tık | Finder Hızlı Eylemleri `~/Library/Services/KaradelikDev Terminal.workflow` (+ Editör); `pbs -flush` | `winenv/macos.rs` |
| Windows servisi (sc) | `~/Library/LaunchAgents/dev.karadelik.<servis>.plist` + `launchctl bootstrap/kickstart/bootout gui/<uid>` | `services/winservice/macos.rs` |
| icacls | grup/diğerleri yazma izni kaldırılır | `acl/macos.rs` |
| VC++ denetimi | Homebrew + Xcode CLT denetimi; yoksa resmi kurulum betiği Terminal'de açılır, sihirbazda "Yeniden denetle" | `setup.rs`, `ui/js/setup.js` |
| Laragon içe aktarma | gizli (`find_laragon` → `None`) | `importer.rs` |
| PowerShell trampolini + setup.exe | `latest.json` → `platforms."macos-<mimari>"` `.app.tar.gz`; SHA-256; `/bin/sh` betiği `.app`'i değiştirip `open -n` ile açar (`--updated`); geri dönüş SHA256SUMS ile | `updater.rs` |
| windows.php.net PHP güncellemesi | `brew outdated`/`brew upgrade` (klasör adı x.y olduğu için profil değişmez) | `scheduler.rs` |
| HeidiSQL / Notepad++ | Sequel Ace / TablePlus (`mysql://` adresiyle) / DBeaver; VS Code veya `open -t` | `actions.rs`, `menu.rs`, `terminal.rs` |
| netstat, certmgr.msc | `lsof -nP -iTCP -sTCP:LISTEN`, Anahtar Zinciri Erişimi | `actions.rs` |
| Tepsi simgesi, Ctrl+Alt+T | Menü çubuğu şablon simgesi (`tray-mac-on/off.png`, tıklayınca menü + "KaradelikDev'i göster"), `⌥⌘T`; uygulama menüsü (Hakkında, Tercihler ⌘,, Gizle, Çık ⌘Q = servisleri durdurur), Düzen (⌘C/⌘V giriş alanlarında), Pencere; Dock simgesine tıklayınca pencere | `lib.rs`, `menu.rs` |
| Çerçevesiz pencere + kendi düğmeleri | `tauri.macos.conf.json`: `titleBarStyle: Overlay`, trafik ışıkları özel başlık çubuğunda; diğer pencereler koyu başlıkla (`windows.rs` `themed`); SF yazı tipi (`html.mac`) | `ui/css/app.css`, `ui/js/common.js` |
| NSIS, `icon.ico` | `bundle.targets: app, dmg`, `icon.icns`, `minimumSystemVersion 11.0`, ad-hoc imza (`signingIdentity: "-"`), `Info.plist` | `tauri.macos.conf.json` |
| Arayüz metinleri | `<anahtar>.mac` varsa macOS'ta o kullanılır (aynı `tr.json`/`en.json`) | `i18n.rs` |
| `tests/*.sh`, `release.ps1` | testler iki platformda (lsof, portlar `karadelik.ini`'den, `KARADELIK_HOSTS`); `scripts/release.sh` | `tests/`, `scripts/` |

## Kararlar

| Konu | Karar | Neden |
|---|---|---|
| Paket kaynağı | **Homebrew**; servisler `brew services` ile değil, KaradelikDev'in ürettiği ayarlarla | PHP, Apache, Nginx, MariaDB, Redis'in resmi macOS ikilileri yok. |
| Sürüm düzeni | `bin/<kategori>/<formül>-<x.y>` → `/opt/homebrew/opt/<formül>` bağlantısı; sürüm eki kurulumda Cellar'dan okunur | `versions.rs` ve menüler değişmeden çalışır. |
| Port 80/443 | **Tüm arabirimlerde** dinle, "yerel ağdan erişim" kapalıyken sunucuda kısıtla (Apache `<Location "/"> AuthMerging And; Require local`, Nginx `allow 127.0.0.1; allow ::1; deny all`) | Doğrulandı: macOS (10.14+) root olmayan süreçlerin 80'i yalnız `*`'ta dinlemesine izin verir, `127.0.0.1:80` → `EACCES`. Root gerektiren pf yönlendirmesi veya sudo'lu nginx (Valet/Herd yaklaşımı) Windows'taki "yönetici izni gerekmez" modeline uymaz. 1024 üstü portlar (Nginx 8080) loopback'te kalır. |
| Veri klasörü | `~/KaradelikDev` (sihirbazdan değiştirilebilir) | Boşluksuz, kullanıcıya ait. |
| Dağıtım | `.dmg` (kullanıcı) + `.app.tar.gz` (güncelleyici), mimari başına ayrı | Windows setup akışının karşılığı. |
| İmza | **Kullanıcı kararı bekliyor**: Developer ID + notarization (yıllık 99 $) yoksa ad-hoc imza ve ilk açılışta "sağ tık → Aç" | Ad-hoc imzalı uygulama uygulama içi güncellemede sorun çıkarmaz (indirmeye karantina özniteliği konmaz). |
| Süreç grupları | macOS'ta her arka plan süreci kendi süreç grubunda (`platform::hidden` → `process_group(0)`) | Apache prefork kapanırken `killpg(getpgrp())` yapar; aynı grupta KaradelikDev'i ve diğer servisleri de öldürüyordu. |

## Bu depoda öğrenilmiş tuzaklar

- Apache + php-cgi (Windows): `SetHandler "proxy:balancer://ad/"` + `ProxyFCGISetEnvIf` ile `SCRIPT_FILENAME` düzeltmesi gerekiyor;
  macOS'ta da gerekli (yol `proxy:balancer://kdphpNNNN/Users/...` olarak gelir), regex platforma göre `templates::fcgi_script_fix`'te.
- macOS: Apache prefork kendi süreç grubunu öldürür (yukarıda). `kill_tree` 0 ve kendi PID'ini asla hedef almaz.
- macOS: Unix soket yolu en fazla 103 karakter; uzun kök yollarında MySQL/PostgreSQL soketi `/tmp`'ye düşer.
- macOS: Homebrew `mysql` ile `mariadb` (sürümsüz) birbirini dışlar; katalog sürümlü (keg-only) formülleri tercih eder.
- macOS: Homebrew 6+ güvenilmeyen tap'lerden formül yüklemez ("Refusing to load formula ... from untrusted tap"); yalnız istenen
  formüle değil bağımlılıklarına da (mongodb-database-tools) güven gerekir. Kurulum `brew tap` sonrası `brew trust --tap <tap>` çalıştırır.
- macOS: `/etc/hosts`'ta yalnız `127.0.0.1` girdisi varsa AAAA sorgusu dış DNS'e gider ve `.test` adresleri saniyelerce
  bekler; blok her ad için `::1` girdisi de yazar.
- macOS: `mysqladmin shutdown` MySQL 8.4 günlüğüne "via user signal" yazar; bu normal kapanıştır.
- macOS: aynı binary'yi `brew services` de çalıştırabileceği için "bizim süreç" denetimi komut satırında/çalışma dizininde kök yolunu da arar
  (`util::pid_mentions_root`); servisler kökteki `tmp/` çalışma diziniyle başlar.
- Güncelleyici, kurulumsuz (taşınabilir) Windows sürümünden geçişte bulunduğu kökü `DataRoot` olarak kaydeder; 1.3.0'da bu,
  `KARADELIK_ROOT` ile çalışan bir kopyadan güncellemede de oluyordu (macOS'ta `config.json`'a geçici test klasörü yazıldı).
  Artık yalnız Windows'ta ve ortam değişkeni yokken kaydedilir (`updater::adopt_portable_root`).
- Test sırasında `.dmg` oluşturan Tauri betiği Finder penceresi açar; derleme çıktısındaki `.app`'ler Spotlight'ta ayrı uygulama
  olarak görünür. Testten sonra `src-tauri/target/**/bundle/macos/*.app` silinip `lsregister -u` ile kayıttan çıkarılmalı.
- Yayın sırası: önce `main` push, sonra `gh release create` (yoksa etiket eski commit'e bağlanır).
- Commit'lerde yapay zekâ imzası/ortak yazar satırı kullanılmaz; yazar `enesbsafak` (GitHub noreply e-postası).
- `docs/laragon-analizi.md` yalnız yerelde kalır, depoya girmez.
- Kullanıcı oyun oynarken pencere/parola istemi açılmamalı; parola/UAC gerektiren testlerden önce sorulmalı.
  macOS testleri `KARADELIK_HOSTS` ile parola istemeden çalışır; CA güveni (`security add-trusted-cert`) ve gerçek `/etc/hosts`
  yazımı parola ister, otomatik testlere alınmadı.

## Mac'te hazırlık

```bash
xcode-select --install
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install node
git clone https://github.com/enesbsafak/karadelikdev.git && cd karadelikdev && git checkout macos
npm install
cd src-tauri && cargo test --lib
```

Uçtan uca test (geçici kökle, gerçek hosts dosyasına dokunmadan):

```bash
export KARADELIK_ROOT=/tmp/kd KARADELIK_HOSTS=/tmp/kd-hosts
printf '127.0.0.1 localhost\n' > $KARADELIK_HOSTS
./src-tauri/target/debug/karadelikdev --setup "php-8.4,php-8.3,apache,nginx,mysql-8.4,redis,memcached,mailpit,composer,phpmyadmin,adminer"
./src-tauri/target/debug/karadelikdev &        # GUI
bash tests/functest.sh && bash tests/functest-extra-services.sh
```

## Yapılacaklar

1. Windows'ta CI'ın ölçemediği kurulum akışı: NSIS kurulum programıyla 1.2.1'den 1.3.0'a uygulama içi güncelleme (UAC).
2. İmza kararı (Developer ID + notarization veya ad-hoc). Karar verilirse `tauri.macos.conf.json` `signingIdentity` ve
   notarization ortam değişkenleri (`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`).
3. Gerçek bir Intel Mac'te (veya Rosetta ile) çalıştırma ve ilk gerçek GitHub sürümünden (1.3.0 → sonraki) güncelleme.
4. Sonraki sürümler: sürüm numarası + `docs/releases/<sürüm>.md`; Actions → **Windows sürüm paketi** (veya Windows'ta
   `scripts\release.ps1`) → `windows-dist` Mac'te `dist/`'e → `bash scripts/release.sh --universal` → önce `main` push, sonra `gh release create`.
