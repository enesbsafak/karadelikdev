<div align="center">

<img src="ui/img/logo.png" width="96" alt="KaradelikDev logosu">

# KaradelikDev

**Windows için hızlı, taşınabilir ve hafif yerel geliştirme ortamı.**
Apache, Nginx, PHP, MySQL/MariaDB, PostgreSQL, MongoDB, Redis, Memcached ve Mailpit tek pencereden.
`www` içine bir klasör koyun, `klasor.test` adresi kendiliğinden hazır olsun.

[![Release](https://img.shields.io/github/v/release/enesbsafak/karadelikdev?color=ff8a1f&label=s%C3%BCr%C3%BCm)](https://github.com/enesbsafak/karadelikdev/releases/latest)
[![Lisans: MIT](https://img.shields.io/badge/lisans-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-0078d6)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24c8db)
![Rust](https://img.shields.io/badge/Rust-stable-dea584)

<sub>*A portable local development environment for Windows (Apache/Nginx, PHP, MySQL/MariaDB, PostgreSQL, MongoDB, Redis, Mailpit) with automatic `*.test` virtual hosts. UI in Turkish and English.*</sub>

<img src="docs/screenshots/main.png" width="640" alt="KaradelikDev ana pencere">

</div>

---

## İçindekiler

- [Özellikler](#özellikler)
- [Neden KaradelikDev?](#neden-karadelikdev)
- [Hızlı başlangıç](#hızlı-başlangıç)
- [Kullanım](#kullanım)
- [Klasör yapısı](#klasör-yapısı)
- [Yapılandırma](#yapılandırma)
- [Komut satırı](#komut-satırı)
- [Varsayılan portlar](#varsayılan-portlar)
- [Güvenlik notları](#güvenlik-notları)
- [Bilinen sınırlamalar](#bilinen-sınırlamalar)
- [Kaynaktan derleme](#kaynaktan-derleme)
- [Lisans](#lisans)

## Özellikler

**Servisler**
- Apache (mod_php veya php-cgi), Nginx, PHP yerleşik sunucusu
- MySQL ve MariaDB, PostgreSQL, MongoDB, Redis, Memcached, Mailpit
- Her servis için başlat / durdur / yeniden yükle, sürüm seçimi, log ve ayar dosyaları
- Port başka bir uygulamadaysa sahibini gösteren uyarı
- Apache, MySQL ve PostgreSQL için "Windows servisi olarak çalıştır"

**Projeler**
- **Otomatik sanal host:** `www\blog` → `http://blog.test`, alt alan adları dahil (`api.blog.test`)
- Laravel/Symfony `public` klasörünü kendiliğinden algılama
- **Proje başına PHP sürümü:** her proje farklı bir PHP sürümüyle çalışabilir (Apache ve Nginx, php-cgi)
- **Node/Vite proxy:** `http://proje.test` adresini `npm run dev` gibi bir geliştirme sunucusuna yönlendirme, WebSocket/HMR dahil
- **Projeler ekranı:** PHP sürümü, proxy portu, klasör, terminal, paylaşma, klonlama ve silme tek listede
- hosts dosyası yönetimi (değişiklik olduğunda yönetici izni sorar)
- **Hızlı uygulama:** Boş proje, WordPress, Laravel, Symfony, CodeIgniter; veritabanı, `.env` ve `wp-config.php` otomatik
- Projeyi klonlama ve silme (veritabanıyla birlikte)
- Cloudflare Tunnel veya ngrok ile internete paylaşma, QR kod

**Veritabanı yöneticisi**
- Bağlantı bilgileri tek ekranda, parola değiştirme ve kaldırma
- Veritabanı listesi (tablo sayısı, boyut), oluşturma, silme, phpMyAdmin'de açma
- Yalnız tek bir veritabanına yetkili MySQL kullanıcısı oluşturma
- Tekil ve toplu yedek, `.sql` dosyasından geri yükleme, otomatik yedek (varsayılan 8 saatte bir, son 5)
- Her MySQL/MariaDB sürümü kendi veri klasörünü ve parolasını kullanır

<p align="center"><img src="docs/screenshots/databases.png" width="560" alt="Veritabanı yöneticisi"></p>

**PHP**
- Birden çok PHP sürümü, tek tıkla geçiş
- Eklentileri ve hızlı ayarları (xdebug, opcache, memory_limit, upload boyutu…) menüden açıp kapatma
- `mail()` → Mailpit, `cacert.pem` ve php.ini yolları otomatik
- Otomatik güncelleme: yeni PHP yama sürümlerini SHA-256 doğrulamasıyla indirir

**SSL**
- Yerel kök sertifika yetkilisi (CA) ve her proje için sertifika; OpenSSL gerekmez
- CA bir kez kullanıcı deposuna eklenir (yönetici izni gerekmez), `https://proje.test` uyarısız açılır

**Araçlar**
- **İlk kurulum sihirbazı:** Visual C++ çalışma zamanı denetimi, önerilen paketler, PATH, sağ tık menüsü
- **Hızlı ekle / Paketler ekranı:** `packages.conf` listesinden indir ve kur (zip, tar.gz, tar.xz), kurulu sürümler arasında geçiş
- **Loglar ekranı:** tüm logları canlı izleme, süzme ve temizleme
- **Uygulama içi güncelleme:** yeni sürümü SHA-256 doğrulamasıyla indirir, servisleri yeniden başlatır; tek tıkla önceki sürüme dönüş
- **Laragon'dan içe aktar:** mevcut Laragon kurulumunun binary'lerini kopyalamadan kullanır
- Windows Terminal / cmd / PowerShell / Cmder; tüm sürümler PATH'te hazır (`Ctrl+Alt+T`)
- Procfile (arka plan komutları), yerleşik cron, profiller, PATH ve Explorer sağ tık menüsü
- Türkçe ve İngilizce arayüz

## Neden KaradelikDev?

KaradelikDev, Laragon'un sevilen çalışma modelini (taşınabilir klasör, `*.test` adresler, tek tıkla servisler) örnek alır ve bazı noktalarda daha ileri gider:

| | KaradelikDev |
|---|---|
| Binary klasörleri | `bin\` içindeki dosyalara hiç yazılmaz; tüm ayarlar `etc\` altında üretilir. Sürüm güncellemek ayarları bozmaz, aynı binary'ler başka araçlarla paylaşılabilir. |
| Sürüm başına veritabanı parolası | MySQL 8.4, 9.4 ve MariaDB'nin her biri kendi parolasını tutar; sürüm değişince "Access denied" olmaz. |
| Veritabanı yöneticisi | Kullanıcı ve yetki yönetimi, tekil yedek, geri yükleme ekranı. |
| SSL | Yerel CA + host başına sertifika, yönetici izni olmadan güvenilir. |
| Paylaşım | Hesap gerektirmeyen Cloudflare Tunnel desteği. |
| Kurulum | Yeni bir paket indirmek aktif sürümü değiştirmez; sürüm değişiminde servisler eski ayarla temiz kapatılır. |
| Otomasyon | `karadelikdev do <eylem>` ve pencere açmadan çalışan `--headless` / `--setup` modları. |

> KaradelikDev bağımsız bir projedir; Laragon ile bağlantısı yoktur. "Laragon" adı sahibine aittir ve burada yalnızca karşılaştırma ve içe aktarma özelliği için anılır.

## Hızlı başlangıç

1. [Son sürümü](https://github.com/enesbsafak/karadelikdev/releases/latest) indirin ve zip'i **boşluk içermeyen** bir klasöre açın, ör. `C:\karadelik\`.
2. `karadelikdev.exe`'yi çalıştırın. İlk açılışta **kurulum sihirbazı** gelir:
   - Bilgisayarda Laragon varsa "Laragon'dan içe aktar" seçilidir (indirme yapılmaz).
   - Yoksa önerilen paketler seçilidir: Apache, PHP 8.4, MySQL 8.4, phpMyAdmin, Composer, Mailpit.
3. **Kur** → **Bitir** → **Tümünü Başlat**.
4. `C:\karadelik\www\merhaba\index.php` oluşturun ve `http://merhaba.test` adresini açın.

> **Gereksinimler:** Windows 10/11 x64, Microsoft Edge WebView2 (Windows 11'de hazır). Sihirbaz eksikse Visual C++ çalışma zamanını kurmayı önerir.
>
> Exe şimdilik dijital olarak imzalı değil; Windows SmartScreen "Bilinmeyen yayımcı" uyarısı gösterebilir: **Ek bilgi → Yine de çalıştır**.

<details>
<summary><b>Kurulum sihirbazı ve Tercihler</b></summary>
<p align="center">
<img src="docs/screenshots/setup.png" width="420" alt="Kurulum sihirbazı">
<img src="docs/screenshots/preferences.png" width="420" alt="Tercihler">
</p>
</details>

## Kullanım

| Nerede | Ne yapar |
|---|---|
| **Menü** (sol üst), pencereye sağ tık, tepsi ikonuna sağ tık | Tüm özelliklerin bulunduğu menü |
| **Tümünü Başlat / Durdur** | Tercihler'de etkin servisleri başlatır / durdurur |
| **Web** · **Veritabanı** · **Terminal** · **Kök** | `localhost`'u açar · Veritabanı yöneticisi · KaradelikDev terminali · `www` klasörü |
| Projeler satırı | Projeye tıklayınca adresini tarayıcıda açar |
| Pencereyi kapatmak | Tepsiye küçültür; çıkış için Menü → Çıkış (servisleri durdurur) |

**Otomatik sanal host.** `www` altındaki her geçerli klasör (harf, rakam, `-`) bir adres olur. Hostname biçimi Tercihler'den değiştirilebilir (ör. `{name}.localhost`; bu biçim hosts dosyasına gerek duymaz). Bir vhost dosyasını kendiniz yönetmek isterseniz adındaki `auto.` önekini kaldırın; KaradelikDev o dosyaya dokunmaz.

**Hızlı uygulama.** Menü → Hızlı uygulama → şablon → proje adı. URL şablonları (WordPress) indirilip açılır; komut şablonları (Laravel, Symfony) terminalde çalışır ve bitince `.env` ile veritabanı ayarlanır. Şablonlar `usr\sites.conf`'ta.

**Paylaşım.** Menü → Paylaş → proje. Varsayılan Cloudflare Quick Tunnel'dır (hesap gerekmez); ngrok da seçilebilir. Bağlantı panoya kopyalanır ve QR kodla gösterilir.

**Procfile ve cron.** `usr\Procfile` içindeki komutlar (`Vite: npm run dev pwd=www/proje`) menüden ya da `autorun` ile açılışta çalışır. `usr\crontab` standart 5 alanlı cron biçimini kullanır; çıktı `logs\cron.log`'a yazılır.

**Profiller.** Seçili sürümler (PHP, MySQL, Apache…) profil olarak saklanır; Menü → Profil ile geçiş yapılır.

**Proje başına PHP sürümü.** Ana pencerede Projeler → Yönet (veya Menü → KaradelikDev → Projeler) ekranında bir projeye profildekinden farklı bir PHP sürümü seçin. O sürümün `php-cgi.exe`'si 9101'den başlayan portlarda ayrıca çalıştırılır; diğer projeler profildeki sürümle devam eder. Ayarlar `usr\projects.ini`'de tutulur.

**Node/Vite proxy.** Aynı ekranda **Proxy portu** alanına geliştirme sunucusunun portunu (ör. Vite için `5173`) yazın. `http://proje.test` o porta yönlendirilir; WebSocket (HMR) de geçer. Sunucuyu kendiniz başlatın (terminal veya Procfile: `Vite: npm run dev pwd=www/proje`).

**Güncelleme.** KaradelikDev açılışta ve günde bir yeni sürüm olup olmadığına bakar (Tercihler → Gelişmiş'ten kapatılabilir). Menü → KaradelikDev → Güncelle: paket indirilir, SHA-256 ile doğrulanır, exe değiştirilir ve çalışan servisler uygulama yeniden açıldıktan sonra tekrar başlatılır. Önceki exe `karadelikdev.exe.old` olarak saklanır; **Önceki sürüme dön** ile geri alınabilir.

## Klasör yapısı

```
C:\karadelik\
├─ karadelikdev.exe
├─ bin\<kategori>\<sürüm>\   Apache, PHP, MySQL… (Hızlı ekle veya Laragon'dan içe aktar)
├─ etc\                      üretilen ayarlar: apache2, nginx, php\<sürüm>\php.ini, mysql, ssl, apps
├─ data\                     veritabanı dosyaları (sürüm başına: mysql-8.4, mariadb-11.8, postgresql-18.2…)
├─ www\                      Document Root
├─ usr\                      karadelik.ini, packages.conf, sites.conf, Procfile, crontab, profile\, tpl\
├─ logs\  tmp\  backup\
```

## Yapılandırma

| Dosya | İçerik |
|---|---|
| `usr\karadelik.ini` | Tercihler, portlar, servis seçimleri (Tercihler penceresinden düzenlenir) |
| `usr\projects.ini` | Proje başına PHP sürümü ve proxy portu (Projeler ekranından düzenlenir) |
| `usr\profile\<ad>.ini` | Profilin seçili sürümleri |
| `usr\packages.conf` | Hızlı ekle listesi: `ad=URL`; `*` ile başlayanlar ilk kurulumda önerilir |
| `usr\sites.conf` | Hızlı uygulama şablonları: `Ad=URL` veya `Ad=komut %s` |
| `usr\tpl\*.tpl` | Apache/Nginx/MySQL şablonlarını geçersiz kılmak için (Menü → Araçlar → Şablonlar) |
| `usr\user.cmd` | Terminal açılırken çalışan kendi komutlarınız |

## Komut satırı

```bat
karadelikdev reload [apache|nginx]        :: vhost'ları yenile, web sunucularını yeniden yükle
karadelikdev start|stop [servis]          :: ör. karadelikdev start mysql
karadelikdev do <eylem> [değer]           :: menü eylemini çalıştır, ör. do startall · do dbcall:create_db blog
karadelikdev --terminal <klasör>          :: KaradelikDev ortamıyla terminal

:: Pencere açmadan (betikler ve CI için)
karadelikdev --setup [--import-laragon] [--no-vc] [paket,paket…]
karadelikdev --headless start|stop
karadelikdev --headless pg-password <parola>   :: PostgreSQL çalışırken postgres parolası (boş = parolasız)
```

Kök klasör varsayılan olarak exe'nin bulunduğu klasördür; `KARADELIK_ROOT` ortam değişkeniyle değiştirilebilir.

## Varsayılan portlar

| Servis | Port | Dinlediği adres |
|---|---|---|
| Apache | 80, SSL 443 | 127.0.0.1 ve ::1 (Tercihler'den yerel ağa açılabilir) |
| Nginx | 8080, SSL 8443 | 127.0.0.1 ve ::1 (Tercihler'den yerel ağa açılabilir) |
| MySQL / MariaDB | 3306 | 127.0.0.1 |
| PostgreSQL | 5432 | localhost |
| MongoDB | 27017 | 127.0.0.1 |
| Redis | 6379 | 127.0.0.1 |
| Memcached | 11211 | 127.0.0.1 |
| Mailpit | SMTP 1025, arayüz 8025 | 127.0.0.1 |
| PHP Server | 8000 | 127.0.0.1 |
| php-cgi havuzu | 9001+ (proje başına sürümler 9101+) | 127.0.0.1 |

## Güvenlik notları

KaradelikDev'in varsayılanları **tek kullanıcılı bir geliştirme bilgisayarı** için kolaylığa göre seçilmiştir:

- MySQL/MariaDB `root` kullanıcısı varsayılan olarak **parolasızdır**; PostgreSQL yerel bağlantılara parola sormaz (`trust`). İkisine de menüden parola konabilir (MySQL → root parolasını değiştir, PostgreSQL → postgres parolasını değiştir); PostgreSQL'de parola konunca yerel bağlantılar `scram-sha-256` ile parola ister.
- Bütün servisler yalnız bu bilgisayarı dinler. Apache/Nginx'i telefonla test için yerel ağa açmak isterseniz: Tercihler → Servisler ve Portlar → **Yerel ağdan erişime izin ver**.
- `.env`, `.git` gibi nokta ile başlayan dosya ve klasörler web sunucusundan sunulmaz (`/.well-known` hariç).
- phpMyAdmin, Adminer gibi yönetim panelleri yalnız bu bilgisayardan ve `localhost` adresiyle açılır; tünelle paylaşılan bir projenin adresi veya DNS rebinding ile erişilemez.
- MySQL'in dosya okuma/yazma komutları (`LOAD DATA INFILE`, `SELECT … INTO OUTFILE`) yalnız `data\mysql-files` klasörüyle sınırlıdır.
- KaradelikDev klasörü başka yerel kullanıcıların yazabileceği bir yerdeyse (ör. `C:\karadelik`, `C:\` izinlerini devralır) açılışta izinleri yalnız sizinle, yöneticilerle ve SYSTEM ile sınırlar.
- Paylaş (tünel) özelliği projeyi internete açar; gizli bilgileri web kökünde tutmayın.
- Üretim ortamı olarak kullanmayın.

## Bilinen sınırlamalar

- Exe imzasız (SmartScreen uyarısı).
- Kök klasör yolunda boşluk olmamalı (Apache ve MySQL ile uyumluluk).
- KaradelikDev ve Laragon aynı portları kullanır; ikisini aynı anda çalıştırmayın.
- Laragon'dan içe aktarılan Apache'de mod_ssl bir OpenSSL sürüm uyarısı yazabilir (Laragon o klasördeki `libcrypto`'yu değiştirdiği için); temiz bir Apache'de oluşmaz.
- Windows Terminal yolu boşluk içeren klasörlerde açılışta sorun çıkarabilir.

## Kaynaktan derleme

Gerekenler: [Rust](https://rustup.rs) (MSVC), Node.js 20+, Visual Studio Build Tools (C++), WebView2.

```bash
npm install
npm run dev       # geliştirme; kök klasör: .\runtime
npm run build     # src-tauri\target\release\karadelikdev.exe
```

**Testler**

```bash
cd src-tauri && cargo test --lib          # birim testleri
bash tests/functest.sh                    # uçtan uca işlev testi (uygulama çalışırken, 71 kontrol)
bash tests/functest-extra-services.sh     # PostgreSQL, MongoDB, MariaDB (12 kontrol)
```

**Proje yapısı**

```
src-tauri\src\
├─ services\      apache, nginx, mysql, phpcgi, others (pg, mongo, redis, memcached, mailpit, php server), winservice
├─ vhosts.rs      otomatik sanal host ve alias'lar
├─ php.rs         php.ini, eklentiler, mod_php / php-cgi
├─ mysqlops.rs    veritabanı işlemleri ve yöneticisi
├─ setup.rs       ilk kurulum sihirbazı
├─ menu.rs        native menü           actions.rs   menü eylemleri
├─ quickapp.rs  packages.rs  ssl.rs  hosts.rs  terminal.rs  procfile.rs
├─ projects.rs    proje başına PHP sürümü ve proxy      updater.rs   uygulama içi güncelleme
├─ acl.rs         kök klasör izinleri
├─ tunnel.rs  winenv.rs  scheduler.rs  importer.rs  cli.rs
src-tauri\resources\tpl\        gömülü şablonlar
src-tauri\resources\defaults\   ilk açılışta usr\ altına yazılan dosyalar
ui\                             arayüz (HTML/CSS/JS, derleme adımı yok)
tests\                          uçtan uca test betikleri
scripts\release.ps1             sürüm paketi: zip, SHA256SUMS.txt, latest.json
```

**Sürüm yayınlama**

1. `src-tauri\Cargo.toml`, `package.json` ve `src-tauri\tauri.conf.json` içindeki sürümü yükseltin, `docs\releases\<sürüm>.md` notunu yazın.
2. `powershell -ExecutionPolicy Bypass -File scripts\release.ps1` → `dist\` altında zip, `SHA256SUMS.txt` ve `latest.json`.
3. Üç dosyayı `v<sürüm>` etiketli GitHub sürümüne yükleyin (betik `gh release create` komutunu yazdırır). Uygulama içi güncelleyici son sürümdeki `latest.json`'u okur; bu dosya eksikse kullanıcılara güncelleme görünmez.

## Lisans

[MIT](LICENSE) © 2026 enesbsafak

KaradelikDev kendi binary'lerini dağıtmaz; Apache, PHP, MySQL ve diğer bileşenler kendi lisanslarıyla resmi kaynaklarından indirilir.
