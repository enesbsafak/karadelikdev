<div align="center">

<img src="ui/img/logo.png" width="96" alt="KaradelikDev logo">

**English** · [Türkçe](README.tr.md)

# KaradelikDev

**A fast, lightweight local development environment for Windows and macOS.**
Apache, Nginx, PHP, MySQL/MariaDB, PostgreSQL, MongoDB, Redis, Memcached and Mailpit from one small window.
Drop a folder into `www` and `folder.test` is ready, with HTTPS.

[![Release](https://img.shields.io/github/v/release/enesbsafak/karadelikdev?color=ff8a1f)](https://github.com/enesbsafak/karadelikdev/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011%20x64-0078d6)
![macOS](https://img.shields.io/badge/macOS-11%2B%20%C2%B7%20Apple%20Silicon%20%7C%20Intel-555555)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24c8db)
![Rust](https://img.shields.io/badge/Rust-stable-dea584)

<img src="docs/screenshots/main.png" width="640" alt="KaradelikDev main window">

</div>

---

## Contents

- [Features](#features)
- [Why KaradelikDev?](#why-karadelikdev)
- [Quick start (Windows)](#quick-start-windows)
- [macOS](#macos)
- [Usage](#usage)
- [Folder layout](#folder-layout)
- [Configuration](#configuration)
- [Command line](#command-line)
- [Default ports](#default-ports)
- [Security notes](#security-notes)
- [Known limitations](#known-limitations)
- [Building from source](#building-from-source)
- [License](#license)

## Features

**Services**
- Apache (mod_php or php-cgi), Nginx, PHP built-in server
- MySQL and MariaDB, PostgreSQL, MongoDB, Redis, Memcached, Mailpit
- Start / stop / reload per service, version switching, logs and config files
- A warning that names the owning process when a port is already taken
- "Run as Windows Service" (macOS: launchd) for Apache, MySQL and PostgreSQL

**Projects**
- **Automatic virtual hosts:** `www\blog` → `http://blog.test`, including subdomains (`api.blog.test`)
- Detects the Laravel/Symfony `public` folder
- **PHP version per project:** each project can run on its own PHP version (Apache and Nginx via php-cgi), and `php`, `composer` and `artisan` in the terminal use that version too
- **Node/Vite proxy:** route `http://project.test` to a dev server such as `npm run dev`, WebSocket/HMR included
- **Projects screen:** PHP version, proxy port, folder, terminal, share, clone and delete in one list
- hosts file management (asks for admin rights only when something changes)
- **Quick app:** blank project, WordPress, Laravel, Symfony, CodeIgniter; database, `.env` and `wp-config.php` set up automatically
- Clone and delete projects (database included)
- Share to the internet with Cloudflare Tunnel (no account needed) or ngrok, with a QR code

<p align="center"><img src="docs/screenshots/projects.png" width="640" alt="Projects screen: PHP version and proxy port per project"></p>

**Database manager**
- Connection details on one screen, change or remove passwords
- Database list (table count, size), create, drop, open in phpMyAdmin
- Create a MySQL user that can access a single database only
- Single and full backups, restore from `.sql`, automatic backups (every 8 hours by default, keeps the last 5)
- Each MySQL/MariaDB version keeps its own data folder and password

<p align="center"><img src="docs/screenshots/databases.png" width="560" alt="Database manager"></p>

**PHP**
- Multiple PHP versions, one-click switching
- Toggle extensions and quick settings (xdebug, opcache, memory_limit, upload size…) from the menu
- **Install extensions:** Xdebug, Redis (phpredis), Imagick, MongoDB, PCOV, APCu, igbinary in one click; official PECL DLLs on Windows, `pecl install` on macOS. The Xdebug quick setting installs Xdebug first if it is missing
- `mail()` → Mailpit, `cacert.pem` and php.ini paths set up automatically
- Patch updates: downloads new PHP patch releases verified with SHA-256

**SSL**
- Local certificate authority and a certificate per project; no OpenSSL needed
- The CA is trusted once in the user store (no admin rights), so `https://project.test` opens without warnings

**Tools**
- **Setup wizard:** Visual C++ runtime check, recommended packages, PATH, Explorer context menu
- **Quick add / Packages screen:** download and install from `packages.conf` (zip, tar.gz, tar.xz), switch between installed versions
- **Logs screen:** live tail, filter and clear all logs
- **In-app updates:** downloads the new version verified with SHA-256, restarts your services; one-click rollback
- **Import from Laragon:** uses an existing Laragon installation's binaries without copying them
- Windows Terminal / cmd / PowerShell / Cmder with every selected version on PATH (`Ctrl+Alt+T`)
- Procfile (background commands), built-in cron, profiles, PATH and Explorer context menu
- English and Turkish UI (picked from the system language on first run)

<p align="center">
<img src="docs/screenshots/packages.png" width="420" alt="Packages screen">
<img src="docs/screenshots/logs.png" width="420" alt="Logs screen">
</p>

## Why KaradelikDev?

KaradelikDev follows the much-loved Laragon workflow (a self-contained folder, `*.test` hosts, one-click services) and goes further in a few places:

| | KaradelikDev |
|---|---|
| Free and open source | MIT licensed, no site limits, no paid tier. |
| Small and native | ~4 MB installer (Tauri + Rust), not a bundled browser. |
| Binary folders stay clean | Nothing under `bin\` is ever written to; all configuration is generated under `etc\`. Updating a version never breaks your settings, and the same binaries can be shared with other tools. |
| Per-version database passwords | MySQL 8.4, 9.4 and MariaDB each keep their own password, so switching versions does not end in "Access denied". |
| Database manager | Users and grants, single-database backups, restore screen. |
| SSL | Local CA plus a certificate per host, trusted without admin rights. |
| Sharing | Cloudflare Tunnel without an account. |
| Installs | Downloading a new package never switches the active version; services shut down cleanly with their old config when you switch. |
| Automation | `karadelikdev do <action>` and windowless `--headless` / `--setup` modes. |

> KaradelikDev is an independent project and is not affiliated with Laragon. "Laragon" belongs to its owner and is mentioned here only for comparison and the import feature.

## Quick start (Windows)

1. Download `KaradelikDev-<version>-setup.exe` from the [latest release](https://github.com/enesbsafak/karadelikdev/releases/latest) and run it. It installs to `C:\Program Files\KaradelikDev` (asks for admin rights; answer **Yes** to UAC) and adds Start menu and desktop shortcuts.
2. On first launch KaradelikDev shows the **setup wizard**:
   - **Data folder:** packages, projects and databases live here (default `C:\karadelik`; pick a path without spaces). If you used portable KaradelikDev before, pick that folder.
   - If Laragon is installed, "Import from Laragon" is preselected (no downloads).
   - Otherwise the recommended packages are preselected: Apache, PHP 8.4, MySQL 8.4, phpMyAdmin, Composer, Mailpit.
3. **Install** → **Finish** → **Start All**.
4. Create `C:\karadelik\www\hello\index.php` and open `http://hello.test`.

**Uninstall:** Settings → Apps → KaradelikDev → Uninstall. Services are stopped; autostart, the context menu and PATH entries are removed. The data folder (projects, databases) is **not** deleted; remove it yourself if you want. `.test` entries in the hosts file and the local SSL root certificate stay (you can untrust it under Menu → Tools → SSL before uninstalling).

> **Requirements:** Windows 10/11 x64, Microsoft Edge WebView2 (built into Windows 11). The wizard offers to install the Visual C++ runtime if it is missing.
>
> The installer is not code-signed yet, so Windows SmartScreen may show "Unknown publisher": **More info → Run anyway**.

<details>
<summary><b>Setup wizard and Preferences</b></summary>
<p align="center">
<img src="docs/screenshots/setup.png" width="420" alt="Setup wizard">
<img src="docs/screenshots/preferences.png" width="420" alt="Preferences">
</p>
</details>

## macOS

The macOS version offers every Windows feature with the same UI; the binaries come from Homebrew.

1. Download `KaradelikDev-<version>-macos-aarch64.dmg` (Apple Silicon) or `-macos-x86_64.dmg` (Intel) from the [latest release](https://github.com/enesbsafak/karadelikdev/releases/latest) and drag KaradelikDev into **Applications**.
2. The app is not signed with an Apple Developer ID yet (ad-hoc signature). If macOS says it "cannot be verified" on first launch: in Finder, **right-click KaradelikDev → Open → Open** (or System Settings → Privacy & Security → **Open Anyway**).
3. The setup wizard opens. If **Homebrew** is missing, "Install Homebrew" opens the official install script in Terminal (asks for your password); then click **Check again**. Recommended packages: Apache, PHP 8.4, MySQL 8.4, phpMyAdmin, Composer, Mailpit. Packages already installed with Homebrew (e.g. `mariadb`, `nginx`) are linked without downloading.
4. **Install** → **Finish** → **Start All**; create `~/KaradelikDev/www/hello/index.php` and open `http://hello.test`.

**How it works**
- Packages are installed with `brew install`; `bin/<category>/<name>-<x.y>` is a symlink to Homebrew's `opt/<formula>` (e.g. `bin/php/php-8.4 -> /opt/homebrew/opt/php@8.4`). Services do **not** run through `brew services`; they use configuration that KaradelikDev generates under `etc/`, and Homebrew's own config files are left alone. If a MySQL/PostgreSQL started with `brew services` is running at the same time, change the port in Preferences (Unix sockets are separate, so they do not clash).
- Apache runs PHP through Homebrew PHP's `libphp.so` (mod_php); Nginx and per-project PHP versions use the php-cgi pool. PHP patch updates come through `brew upgrade` (Menu → PHP → Check PHP updates).
- macOS lets non-root apps listen on ports 80/443 only on **all interfaces** (`127.0.0.1:80` is refused). KaradelikDev listens on all interfaces and, while "Allow access from the local network" is off, restricts requests to this Mac inside the server (Apache `Require local`, Nginx `allow 127.0.0.1`). With the macOS firewall on, you may see a permission prompt for `httpd` on first start.
- Updating `/etc/hosts` for `.test` hosts asks for your admin password, then flushes the DNS cache. The local SSL root certificate is added to the login keychain (macOS asks for your password to trust it).

| Windows | macOS |
|---|---|
| Tray icon, left click opens the window | Menu bar icon (click for the menu); plus the app menu, Edit and Window menus, Dock |
| `Ctrl+Alt+T` terminal | `⌥⌘T`; Terminal.app or iTerm2 (Preferences → Advanced). Your shell loads its own settings, then `usr/kdenv.sh` |
| Explorer context menu | Finder Quick Actions: right-click a folder → Quick Actions → KaradelikDev Terminal / Editor |
| User PATH (registry) | A marked block in `~/.zprofile` (`~/.bash_profile` for bash) |
| Run as Windows Service | Run as a service at login: `~/Library/LaunchAgents/dev.karadelik.<service>.plist` (no admin rights) |
| Run when Windows starts | Run at login (LaunchAgent) |
| HeidiSQL | Sequel Ace / TablePlus / DBeaver (if installed; opened with the connection details), phpMyAdmin, Adminer |
| `usr\user.cmd` | `usr/user.sh` |
| Import from Laragon | — (Windows only) |

**Updates (macOS).** Menu → KaradelikDev → Update: the `.app.tar.gz` is downloaded and verified with SHA-256; when the app quits, `/Applications/KaradelikDev.app` is replaced and reopened, and running services are restarted.

**Uninstall (macOS).** Optionally run `/Applications/KaradelikDev.app/Contents/MacOS/karadelikdev --uninstall` first (stops services and removes the login item, Quick Actions, the PATH block and LaunchAgents), then drag the app to the Trash. `~/KaradelikDev` (projects, databases) and Homebrew packages are not deleted.

## Usage

| Where | What it does |
|---|---|
| **Menu** (top left), right-click the window, right-click the tray icon | The menu with every feature |
| **Start All / Stop All** | Starts / stops the services enabled in Preferences |
| **Web** · **Database** · **Terminal** · **Root** | Opens `localhost` · database manager · KaradelikDev terminal · the `www` folder |
| Projects row | Click a project to open its address in the browser |
| Closing the window | Minimizes to the tray; quit with Menu → Exit (stops services) |

**Automatic virtual hosts.** Every valid folder under `www` (letters, digits, `-`) becomes a host. The hostname format can be changed in Preferences (e.g. `{name}.localhost`, which needs no hosts file entries). To manage a vhost file yourself, remove the `auto.` prefix from its name and KaradelikDev will leave it alone.

**Quick app.** Menu → Quick app → template → project name. URL templates (WordPress) are downloaded and extracted; command templates (Laravel, Symfony) run in a terminal, and `.env` and the database are set up when they finish. Templates live in `usr\sites.conf`.

**Sharing.** Menu → Share → project. The default is a Cloudflare Quick Tunnel (no account needed); ngrok is also available. The link is copied to the clipboard and shown as a QR code.

**Procfile and cron.** Commands in `usr\Procfile` (`Vite: npm run dev pwd=www/project`) run from the menu, or at startup with `autorun`. `usr\crontab` uses the standard 5-field cron format; output goes to `logs\cron.log`.

**Profiles.** The selected versions (PHP, MySQL, Apache…) are saved as a profile; switch with Menu → Profile.

**PHP version per project.** In the main window, Projects → Manage (or Menu → KaradelikDev → Projects), pick a PHP version for a project that differs from the profile. That version's `php-cgi` runs separately on ports from 9101; other projects keep the profile version. Settings are stored in `usr\projects.ini`.
It applies in the terminal too: `usr\bin\php.cmd` (macOS `usr/bin/php`) looks at the current folder and runs the project's PHP with its php.ini when you are inside a project folder (or a subfolder), and the profile version otherwise. Because `usr\bin` is first on PATH, the KaradelikDev terminal, other terminals and IDE terminals (once KaradelikDev is on PATH), Procfile and cron commands (`php artisan schedule:run`) all use the same version. The file is only created when some project uses a different version. On Windows it is a `cmd` batch file, so stopping a long-running command with Ctrl+C may ask "Terminate batch job (Y/N)?".

**Installing PHP extensions.** Menu → PHP → Extensions → Install extension. On Windows the DLL matching the selected PHP build (version, TS/NTS, VS16/VS17) is downloaded from `downloads.php.net/~windows/pecl`; the extension goes into `ext\` and any helper DLLs (Imagick) into the PHP folder. On macOS, Homebrew PHP's `pecl install` builds it from source (autoconf, pkgconf and required libraries are installed with Homebrew first; this can take a few minutes). The extension is enabled in php.ini and KaradelikDev checks that PHP can load it; if it cannot, the change is reverted. From the command line: `karadelikdev --headless ext-install <name>`.

**Node/Vite proxy.** On the same screen, enter your dev server's port (e.g. `5173` for Vite) as the **Proxy port**. `http://project.test` is forwarded to that port, WebSocket (HMR) included. Start the dev server yourself (terminal or Procfile: `Vite: npm run dev pwd=www/project`).

**Updates.** KaradelikDev checks for a new version at startup and once a day (can be turned off under Preferences → Advanced). Menu → KaradelikDev → Update: the installer is downloaded and verified with SHA-256, the app closes, the installer runs (only the UAC prompt appears), and KaradelikDev reopens and restarts the services that were running. **Roll back to previous version** downloads the previous version's installer from GitHub (verified with that release's `SHA256SUMS.txt`) and installs it the same way.

## Folder layout

```
C:\Program Files\KaradelikDev\   program (karadelikdev.exe, uninstall.exe)
/Applications/KaradelikDev.app  macOS: program; data folder ~/KaradelikDev (bin/ entries link to Homebrew)

C:\karadelik\                  data folder (can be changed in the wizard)
├─ bin\<category>\<version>\  Apache, PHP, MySQL… (Quick add or Import from Laragon)
├─ etc\                      generated config: apache2, nginx, php\<version>\php.ini, mysql, ssl, apps
├─ data\                     database files (per version: mysql-8.4, mariadb-11.8, postgresql-18.2…)
├─ www\                      Document Root
├─ usr\                      karadelik.ini, packages.conf, sites.conf, Procfile, crontab, profile\, tpl\
├─ logs\  tmp\  backup\
```

## Configuration

| File | Contents |
|---|---|
| `usr\karadelik.ini` | Preferences, ports, enabled services (edited in the Preferences window) |
| `usr\projects.ini` | PHP version and proxy port per project (edited on the Projects screen) |
| `usr\profile\<name>.ini` | The profile's selected versions |
| `usr\packages.conf` | Quick add list: `name=URL`; entries starting with `*` are recommended on first setup |
| `usr\sites.conf` | Quick app templates: `Name=URL` or `Name=command %s` |
| `usr\tpl\*.tpl` | Override the Apache/Nginx/MySQL templates (Menu → Tools → Templates) |
| `usr\user.cmd` | Your own commands, run when a terminal opens |

## Command line

```bat
karadelikdev reload [apache|nginx]        :: regenerate vhosts, reload the web servers
karadelikdev start|stop [service]         :: e.g. karadelikdev start mysql
karadelikdev do <action> [value]          :: run a menu action, e.g. do startall · do dbcall:create_db blog
karadelikdev --terminal <folder>          :: terminal with the KaradelikDev environment

:: Without a window (scripts and CI)
karadelikdev --setup [--import-laragon] [--no-vc] [package,package…]
karadelikdev --headless start|stop
karadelikdev --headless pg-password <password>   :: postgres password while PostgreSQL runs (empty = none)
karadelikdev --headless ext-install <name>       :: install a PHP extension into the selected PHP
karadelikdev --headless env                      :: rewrite the terminal environment and usr\bin\php shim
```

The data folder is stored in `HKCU\Software\KaradelikDev\DataRoot` (macOS: `~/Library/Application Support/KaradelikDev/config.json`) and changed from the wizard; the `KARADELIK_ROOT` environment variable overrides both. On macOS the `karadelikdev` command is available in the KaradelikDev terminal, and in every terminal once it is on PATH (`usr/bin/karadelikdev`). Outside an installation, e.g. an exe built from source, the exe's own folder is used as the root.

## Default ports

| Service | Port | Listens on |
|---|---|---|
| Apache | 80, SSL 443 | 127.0.0.1 and ::1 (can be opened to the LAN in Preferences). macOS: all interfaces, access limited to this Mac |
| Nginx | 8080, SSL 8443 | 127.0.0.1 and ::1 (can be opened to the LAN in Preferences) |
| MySQL / MariaDB | 3306 | 127.0.0.1 |
| PostgreSQL | 5432 | localhost |
| MongoDB | 27017 | 127.0.0.1 |
| Redis | 6379 | 127.0.0.1 |
| Memcached | 11211 | 127.0.0.1 |
| Mailpit | SMTP 1025, UI 8025 | 127.0.0.1 |
| PHP Server | 8000 | 127.0.0.1 |
| php-cgi pool | 9001+ (per-project versions 9101+) | 127.0.0.1 |

## Security notes

KaradelikDev's defaults favor convenience on a **single-user development machine**:

- The MySQL/MariaDB `root` user has **no password** by default; PostgreSQL does not ask local connections for a password (`trust`). Both can be given a password from the menu (MySQL → Change root password, PostgreSQL → Change postgres password); once PostgreSQL has a password, local connections require it via `scram-sha-256`.
- Every service listens on this computer only. To open Apache/Nginx to your LAN for testing on a phone: Preferences → Services and Ports → **Allow access from the local network**.
- Dotfiles and dot-folders such as `.env` and `.git` are never served (except `/.well-known`).
- Admin panels such as phpMyAdmin and Adminer open only from this computer and only via `localhost`; they cannot be reached through a shared tunnel address or DNS rebinding.
- MySQL's file commands (`LOAD DATA INFILE`, `SELECT … INTO OUTFILE`) are limited to the `data\mysql-files` folder.
- If the KaradelikDev folder is writable by other local users (e.g. `C:\karadelik` inheriting the permissions of `C:\`), permissions are restricted to you, administrators and SYSTEM at startup.
- Share (tunnel) exposes the project to the internet; do not keep secrets in the web root.
- Do not use it as a production server.

## Known limitations

- The Windows installer is unsigned (SmartScreen warning). The macOS app has an ad-hoc signature (Gatekeeper warning on first launch).
- The root folder path must not contain spaces (Apache and MySQL compatibility).
- KaradelikDev and Laragon use the same ports; do not run both at once.
- With an Apache imported from Laragon, mod_ssl may log an OpenSSL version warning (Laragon replaces `libcrypto` in that folder); a clean Apache does not.
- Windows Terminal may have trouble opening folders whose path contains spaces.

## Building from source

Requirements: [Rust](https://rustup.rs) (MSVC), Node.js 20+, Visual Studio Build Tools (C++), WebView2.
macOS: Xcode command line tools (`xcode-select --install`), Rust, Node.js 20+; `npm run build` produces the `.app` and `.dmg` (`src-tauri/target/release/bundle/`). For tests, start the app with `KARADELIK_ROOT=<folder> KARADELIK_HOSTS=<file>` (development builds then use that file instead of `/etc/hosts`, no password prompts).

```bash
npm install
npm run dev       # development; root folder: .\runtime
npm run build     # src-tauri\target\release\bundle\nsis\KaradelikDev_<version>_x64-setup.exe
```

**Tests**

```bash
cd src-tauri && cargo test --lib          # unit tests
bash tests/phpenv.sh                      # PHP extensions and per-project PHP (app closed, windowless)
bash tests/functest.sh                    # end-to-end test (while the app is running)
bash tests/functest-extra-services.sh     # PostgreSQL, MongoDB, MariaDB
```

All of these run on Windows, macOS Intel and macOS Apple Silicon in GitHub Actions (`.github/workflows/platform-tests.yml`).

**Project layout**

```
src-tauri\src\
├─ services\      apache, nginx, mysql, phpcgi, others (pg, mongo, redis, memcached, mailpit, php server), winservice
├─ vhosts.rs      automatic virtual hosts and aliases
├─ php.rs         php.ini, extensions, mod_php / php-cgi      phpext.rs   extension installer (PECL)
├─ mysqlops.rs    database operations and manager
├─ setup.rs       first-run setup wizard
├─ menu.rs        native menu           actions.rs   menu actions
├─ quickapp.rs  packages.rs  ssl.rs  hosts.rs  terminal.rs  procfile.rs
├─ projects.rs    per-project PHP version and proxy      updater.rs   in-app updates
├─ acl.rs         root folder permissions
├─ tunnel.rs  winenv.rs  scheduler.rs  importer.rs  cli.rs
src-tauri\resources\tpl\        built-in templates
src-tauri\resources\defaults\   files written under usr\ on first run
ui\                             UI (HTML/CSS/JS, no build step)
tests\                          end-to-end test scripts
src-tauri\windows\hooks.nsh    installer/uninstaller hooks (NSIS)
src-tauri\src\platform\        OS layer (windows.rs, macos.rs); winenv\, acl\, services\winservice\ follow the same pattern
src-tauri\tauri.macos.conf.json macOS bundle and window settings (merged over tauri.conf.json)
scripts\release.ps1             Windows release package: setup, SHA256SUMS.txt, latest.json
scripts/release.sh              macOS release package: dmg, app.tar.gz; appended to SHA256SUMS.txt and latest.json
```

Source code comments are in Turkish.

**Releasing**

1. Bump the version in `src-tauri\Cargo.toml`, `package.json` and `src-tauri\tauri.conf.json`, and write `docs\releases\<version>.md`.
2. Push main, then run the **Sürüm** (release) workflow on main (`gh workflow run release.yml --ref main`). The Windows runner builds the setup, `SHA256SUMS.txt` and `latest.json` with `scripts\release.ps1`; the macOS runner adds the aarch64 and x86_64 dmg and app.tar.gz with `bash scripts/release.sh --universal` and writes the `platforms` entries into `latest.json` (older Windows clients keep reading the `url`/`sha256` fields). The result is a `dist` artifact and a **draft** `v<version>` release bound to the commit that was built.
3. Review the draft and publish it. The in-app updater reads `latest.json` from the latest published release; drafts and releases without that file are invisible to users.

Manual packaging also works: `scripts\release.ps1` on Windows, copy `dist\` into `dist/` on a Mac and run `bash scripts/release.sh --universal`; push main first, then run the `gh release create` command the script prints.

## License

[MIT](LICENSE) © 2026 enesbsafak

KaradelikDev does not redistribute third-party binaries; Apache, PHP, MySQL and the other components are downloaded from their official sources under their own licenses.
