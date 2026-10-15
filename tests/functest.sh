#!/usr/bin/env bash
# KaradelikDev işlev testi. Çalışan uygulamaya `karadelikdev do <eylem>` ile komut gönderir,
# etkisini dış dünyadan (port, HTTP, MySQL, dosya, kayıt defteri / kabuk profili) doğrular.
# Kullanım: KaradelikDev (debug, kök: runtime veya KARADELIK_ROOT) çalışırken  bash tests/functest.sh
# Windows: yeni test projesi hosts dosyasına eklenirken UAC sorulabilir.
# macOS: uygulamayı KARADELIK_HOSTS=<dosya> ile başlatın; testler /etc/hosts yerine o dosyayı kullanır (parola sorulmaz).
#        Portlar usr/karadelik.ini'den okunur (ör. Homebrew MySQL 3306'dayken MySQL 3307).
cd "$(dirname "$0")/.." || exit 1
case "$(uname -s)" in Darwin) MAC=1; X="" ;; *) MAC=; X=.exe ;; esac
K=./src-tauri/target/debug/karadelikdev$X
R=${KARADELIK_ROOT:-runtime}
P=0; F=0; FAILS=()
ok()   { echo "  ✔ $1"; P=$((P+1)); }
bad()  { echo "  ✘ $1  →  $2"; F=$((F+1)); FAILS+=("$1: $2"); }
if [ -n "$MAC" ]; then
  port() { lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1; }
  pid_of_port() { lsof -nP -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null | head -1; }
  kill_pid() { kill -9 "$1" 2>/dev/null; }
  mtime() { stat -f %m "$1" 2>/dev/null || echo 0; }
  winpath() { echo "$1"; }
  clip() { pbpaste; }
  HOSTS=${KARADELIK_HOSTS:-/etc/hosts}
else
  port() { netstat -ano | grep LISTEN | grep -qE "[:.]$1 " ; }
  pid_of_port() { netstat -ano | grep LISTEN | grep "127.0.0.1:$1 " | awk '{print $5}' | head -1; }
  kill_pid() { taskkill //F //PID "$1" >/dev/null 2>&1; }
  mtime() { stat -c %Y "$1" 2>/dev/null || echo 0; }
  winpath() { cygpath -w "$1"; }
  clip() { powershell -NoProfile -Command Get-Clipboard; }
  HOSTS=/c/Windows/System32/drivers/etc/hosts
fi
# usr/karadelik.ini: [service.<ad>] Port=
cport() { awk -v s="[service.$1]" '$0==s{f=1;next} /^\[/{f=0} f&&/^Port=/{sub("Port=","");gsub("\r","");print;exit}' $R/usr/karadelik.ini; }
cssl() { awk -v s="[service.$1]" '$0==s{f=1;next} /^\[/{f=0} f&&/^SSLPort=/{sub("SSLPort=","");gsub("\r","");print;exit}' $R/usr/karadelik.ini; }
AP=$(cport apache); AP=${AP:-80}; NP=$(cport nginx); NP=${NP:-8080}; MP=$(cport mysql); MP=${MP:-3306}; MAILP=$(cport mailpit); MAILP=${MAILP:-1025}
APS=$(cssl apache); APS=${APS:-443}
waitfor() { local n=$1; shift; for _ in $(seq 1 "$n"); do eval "$@" && return 0; sleep 1; done; return 1; }
do_() { "$K" do "$@"; }
mysqlbin() { ls -d $R/bin/mysql/$(grep -A1 '^\[mysql\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r')/bin; }
q() { MYSQL_PWD="${PW:-}" "$(mysqlbin)/mysql$X" --default-character-set=utf8mb4 -u "${QU:-root}" -h 127.0.0.1 -P "$MP" -N -e "$1" 2>&1; }
http() { curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$@"; }
phpver() { grep -A1 '^\[php\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r'; }
INI() { echo "$R/etc/php/$(phpver)/php.ini"; }
A="http://127.0.0.1:$AP"; [ "$AP" = 80 ] && A="http://127.0.0.1"
L="http://localhost:$AP"; [ "$AP" = 80 ] && L="http://localhost"
mkdir -p $R/www/demo; [ -f $R/www/demo/index.php ] || echo '<?php echo "demo ", PHP_VERSION;' > $R/www/demo/index.php

echo "== Servisler"
do_ startall
waitfor 40 'port $AP && port $MP && port $MAILP' && ok "Tümünü başlat (Apache, MySQL, Mailpit)" || bad "Tümünü başlat" "portlar açılmadı"
[ "$(http $L/)" = 200 ] && ok "localhost başlangıç sayfası" || bad "localhost" "HTTP $(http $L/)"
for a in phpmyadmin adminer redis; do [ -d $R/etc/apps/$a ] || continue; c=$(http $L/$a/); [ "$c" = 200 ] && ok "/$a/ alias" || bad "/$a/ alias" "HTTP $c"; done
[ "$(http http://127.0.0.1:8025/)" = 200 ] && ok "Mailpit web arayüzü" || bad "Mailpit UI" "kapalı"

echo "== Güvenlik varsayılanları"
if [ -n "$MAC" ] && [ "$AP" -lt 1024 ]; then
  # macOS: 1024 altı port yalnız tüm arabirimlerde dinlenebilir; erişim httpd.conf'ta yerel makineyle sınırlanır
  grep -q "Require local" $R/etc/apache2/httpd.conf && ok "Apache *:$AP, erişim yalnız bu Mac (Require local)" || bad "Apache yerel kısıt" "httpd.conf'ta Require local yok"
else
  lsof_or_netstat=$( [ -n "$MAC" ] && lsof -nP -iTCP:$AP -sTCP:LISTEN || netstat -ano | grep LISTEN )
  echo "$lsof_or_netstat" | grep -qE "0\.0\.0\.0:$AP |\[::\]:$AP |\*:$AP " && bad "Apache yalnız loopback" "tüm arabirimleri dinliyor" || ok "Apache yalnız 127.0.0.1/::1 dinliyor"
fi
echo "SECRET=1" > $R/www/demo/.env
[ "$(http -H 'Host: demo.test' $A/.env)" = 403 ] && ok ".env sunulmuyor (403)" || bad ".env" "HTTP $(http -H 'Host: demo.test' $A/.env)"
rm -f $R/www/demo/.env
if [ -d $R/etc/apps/phpmyadmin ]; then
  [ "$(http -H 'Host: demo.test' $A/phpmyadmin/)" = 403 ] && ok "phpMyAdmin başka host adıyla açılmıyor" || bad "alias Host kontrolü" "HTTP $(http -H 'Host: demo.test' $A/phpmyadmin/)"
fi
[ "$(http -H 'Host: evil.example' "$A/?q=info")" = 403 ] && ok "phpinfo yabancı Host ile kapalı" || bad "phpinfo Host" "açık"
[ "$(q "SELECT @@secure_file_priv" | tr -d '\r')" != "" ] && ok "MySQL secure_file_priv kısıtlı" || bad "secure_file_priv" "boş"

echo "== MySQL parolasız varsayılan"
do_ dbcall:change_root ""
waitfor 15 '[ "$(q "SELECT 1")" = 1 ]' && ok "root parolası kaldırıldı, parolasız giriş" || bad "parolasız root" "$(q 'SELECT 1' | head -1)"
KEY=$(phpver >/dev/null; grep -A1 '^\[mysql\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r' | grep -oE '^[a-z]+-[0-9]+\.[0-9]+')
grep -qE "^RootPassword\.${KEY//./\\.}=\s*$" $R/usr/karadelik.ini && ok "parola kaydı ($KEY) boş" || bad "parola kaydı" "$(grep RootPassword $R/usr/karadelik.ini | tr '\n' ' ')"

echo "== Veritabanı yöneticisi işlemleri"
do_ dbcall:create_db kdtest
waitfor 10 'q "SHOW DATABASES" | grep -qx kdtest' && ok "veritabanı oluştur" || bad "create_db" "yok"
q "CREATE TABLE kdtest.t(x INT); INSERT INTO kdtest.t VALUES (7);" >/dev/null
do_ dbcall:create_user kduser Gizli123 kdtest
waitfor 10 '[ "$(QU=kduser PW=Gizli123 q "SELECT x FROM kdtest.t")" = 7 ]' && ok "kullanıcı oluştur + yetki (kduser → kdtest)" || bad "create_user" "$(QU=kduser PW=Gizli123 q 'SELECT x FROM kdtest.t' | head -1)"
QU=kduser PW=Gizli123 q "SHOW DATABASES" | grep -qx wpsite && bad "kullanıcı yetki sınırı" "kduser başka veritabanını görüyor" || ok "kullanıcı yalnız kendi veritabanını görüyor"
before=$(ls $R/backup/mysql/db-kdtest-*.sql 2>/dev/null | wc -l)
do_ dbcall:backup_db kdtest
waitfor 20 '[ $(ls $R/backup/mysql/db-kdtest-*.sql 2>/dev/null | wc -l) -gt $before ]' && ok "tekil yedek" || bad "backup_db" "dosya yok"
BK=$(ls -t $R/backup/mysql/db-kdtest-*.sql 2>/dev/null | head -1)
do_ dbcall:drop_db kdtest
waitfor 10 '! q "SHOW DATABASES" | grep -qx kdtest' && ok "veritabanı sil" || bad "drop_db" "hâlâ var"
do_ dbcall:restore "$(winpath "$BK")" kdtest
waitfor 20 '[ "$(q "SELECT x FROM kdtest.t")" = 7 ]' && ok "yedekten geri yükle (veriyle)" || bad "restore" "$(q 'SELECT x FROM kdtest.t' | head -1)"
do_ dbcall:drop_user kduser localhost; do_ dbcall:drop_user kduser 127.0.0.1
waitfor 10 '[ -z "$(q "SELECT User FROM mysql.user WHERE User=\"kduser\"")" ]' && ok "kullanıcı sil" || bad "drop_user" "hâlâ var"
do_ dbcall:drop_db kdtest; rm -f "$BK"
touch /tmp/kd-mark; sleep 1; do_ mysql:backup
waitfor 30 '[ -n "$(find $R/backup/mysql -name "mysql-*.sql" -newer /tmp/kd-mark 2>/dev/null)" ]' && ok "tüm veritabanlarını yedekle" || bad "backup_all" "yeni dosya yok"
do_ mysql:createdb_do kdmenu
waitfor 10 'q "SHOW DATABASES" | grep -qx kdmenu' && ok "menüden veritabanı oluştur" || bad "mysql:createdb" "yok"
do_ dbcall:drop_db kdmenu
do_ dbcall:reset_root
sleep 4; waitfor 40 'port $MP && [ "$(q "SELECT 1")" = 1 ]' && ok "root parolasını sıfırla (parolasız, MySQL yeniden başladı)" || bad "reset_root" "$(q 'SELECT 1' | head -1)"

echo "== Otomatik sanal host"
mkdir -p $R/www/kdauto/public && echo '<?php echo "auto ", PHP_VERSION;' > $R/www/kdauto/public/index.php && echo ok > $R/www/kdauto/artisan
waitfor 25 '[ -f $R/etc/apache2/sites-enabled/auto.kdauto.test.conf ]' && ok "yeni klasör algılandı → vhost" || bad "auto vhost" "conf yok"
grep -q "kdauto/public" $R/etc/apache2/sites-enabled/auto.kdauto.test.conf && ok "Laravel public algılama" || bad "public algılama" "yanlış DocumentRoot"
waitfor 15 'curl -s -H "Host: kdauto.test" $A/ | grep -q auto' && ok "kdauto.test yanıt veriyor" || bad "kdauto.test" "yanıt yok"
waitfor 30 'grep -q kdauto.test $HOSTS' && ok "hosts dosyasına eklendi" || bad "hosts" "eklenmedi (izin reddedildi mi?)"
rm -rf $R/www/kdauto
waitfor 25 '[ ! -f $R/etc/apache2/sites-enabled/auto.kdauto.test.conf ]' && ok "klasör silinince vhost kaldırıldı" || bad "vhost temizliği" "conf duruyor"
do_ vhosts_delete_auto; sleep 2
[ -z "$(ls $R/etc/apache2/sites-enabled/ | grep '^auto\.')" ] && ok "Tüm [auto] dosyalarını sil" || bad "delete_auto" "dosyalar duruyor"
do_ vhosts_sync
waitfor 15 '[ -f $R/etc/apache2/sites-enabled/auto.demo.test.conf ]' && ok "vhost yeniden senkron" || bad "vhosts_sync" "demo conf yok"

echo "== Proje başına ayarlar"
# PHP_VERSION'ın klasör adından okunan kısmı (Windows: php-8.4.26-..., Homebrew: php-8.4)
pv() { echo "$1" | grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -1; }
OTHER=$(ls $R/bin/php | grep -v "^$(phpver)$" | while read v; do [ -f "$R/bin/php/$v/php-cgi$X" ] || [ -f "$R/bin/php/$v/bin/php-cgi" ] && echo "$v"; done | head -1)
if [ -n "$OTHER" ]; then
  printf '[demo]\r\nPhp=%s\r\n' "$OTHER" > $R/usr/projects.ini; do_ vhosts_sync
  WANT=$(pv "$OTHER")
  waitfor 30 'curl -s -H "Host: demo.test" $A/ | grep -qF "$WANT"' && ok "demo projesi PHP $WANT ile (profil: $(phpver))" || bad "proje başına PHP" "$(curl -s -H 'Host: demo.test' $A/ | head -c 120)"
fi
NODE=$(ls $R/bin/nodejs/*/node$X $R/bin/nodejs/*/bin/node 2>/dev/null | head -1)
[ -z "$NODE" ] && [ -n "$MAC" ] && NODE=$(command -v node)
if [ -n "$NODE" ]; then
  "$NODE" -e "require('http').createServer((q,s)=>s.end('node-proxy-ok')).listen(5199,'127.0.0.1')" & NODEP=$!
  printf '[demo]\r\nProxy=5199\r\n' > $R/usr/projects.ini; do_ vhosts_sync
  waitfor 30 'curl -s -H "Host: demo.test" $A/ | grep -q node-proxy-ok' && ok "demo.test → 127.0.0.1:5199 proxy" || bad "proxy" "$(curl -s -H 'Host: demo.test' $A/ | head -c 120)"
  kill $NODEP 2>/dev/null
fi
rm -f $R/usr/projects.ini; do_ vhosts_sync
waitfor 30 'curl -s -H "Host: demo.test" $A/ | grep -qF "$(pv "$(phpver)")"' && ok "ayar kaldırılınca profildeki PHP" || bad "proje ayarı geri alma" "$(curl -s -H 'Host: demo.test' $A/ | head -c 120)"

echo "== Hızlı uygulama"
do_ quick_create:Blank kdblank
waitfor 20 '[ -d $R/www/kdblank ] && q "SHOW DATABASES" | grep -qx kdblank' && ok "Blank proje + otomatik veritabanı" || bad "Blank" "klasör/db yok"
do_ quick_create:WordPress kdwp
waitfor 120 '[ -f $R/www/kdwp/wp-config.php ]' && ok "WordPress indir + wp-config" || bad "WordPress" "wp-config yok"
grep -q "'DB_NAME', 'kdwp'" $R/www/kdwp/wp-config.php 2>/dev/null && ok "wp-config veritabanı adı" || bad "wp-config" "DB_NAME yanlış"
waitfor 20 '[ "$(http -H "Host: kdwp.test" $A/wp-admin/install.php)" = 200 ]' && ok "WordPress kurulum sayfası" || bad "WordPress sayfa" "HTTP $(http -H 'Host: kdwp.test' $A/wp-admin/install.php)"
do_ cloneproj_do:kdwp kdwp2
waitfor 60 '[ -f $R/www/kdwp2/wp-config.php ] && q "SHOW DATABASES" | grep -qx kdwp2' && ok "projeyi klonla (dosya + veritabanı)" || bad "clone" "eksik"
grep -q "'kdwp2'" $R/www/kdwp2/wp-config.php 2>/dev/null && ok "klon wp-config güncellendi" || bad "klon wp-config" "eski ad"
for p in kdblank kdwp kdwp2; do do_ delproj_do:$p; done
waitfor 30 '[ ! -d $R/www/kdblank ] && [ ! -d $R/www/kdwp ] && [ ! -d $R/www/kdwp2 ]' && ok "projeyi sil (3 proje)" || bad "delproj" "klasör duruyor"
waitfor 10 '! q "SHOW DATABASES" | grep -qE "^(kdblank|kdwp|kdwp2)$"' && ok "silinen projelerin veritabanları kaldırıldı" || bad "delproj db" "db duruyor"

echo "== Apache / Nginx / php-cgi"
do_ svc:apache:reload; sleep 2
waitfor 20 'port $AP && [ "$(http $L/)" = 200 ]' && ok "Apache yeniden yükle" || bad "apache reload" "kapalı"
do_ svc:nginx:start
waitfor 20 'port $NP && port 9001' && ok "Nginx + php-cgi havuzu" || bad "nginx" "port yok"
waitfor 10 'curl -s -H "Host: demo.test" http://127.0.0.1:$NP/ | grep -q demo' && ok "Nginx üzerinden PHP" || bad "nginx php" "yanıt yok"
kill_pid "$(pid_of_port 9001)"
waitfor 15 'port 9001' && ok "ölen php-cgi yeniden başlatıldı (izleyici)" || bad "php-cgi monitor" "9001 geri gelmedi"
do_ svc:nginx:reload; sleep 2; port $NP && ok "Nginx yeniden yükle" || bad "nginx reload" "kapalı"
do_ svc:nginx:stop
waitfor 15 '! port $NP' && ok "Nginx durdur" || bad "nginx stop" "açık"
waitfor 10 '! port 9001' && ok "php-cgi havuzu kapandı (mod_php kullanılıyor)" || bad "php-cgi release" "hâlâ açık"

echo "== Diğer servisler"
for s in redis:6379 memcached:11211 phpserver:8000; do n=${s%%:*}; p=${s##*:}
  do_ svc:$n:start; waitfor 15 "port $p" && ok "$n başlat" || bad "$n start" "port $p kapalı"
done
RCLI=$(ls $R/bin/redis/*/redis-cli$X $R/bin/redis/*/bin/redis-cli 2>/dev/null | head -1)
[ "$("$RCLI" -p 6379 PING 2>&1)" = PONG ] && ok "Redis PING" || bad "redis ping" "yanıt yok"
curl -s http://127.0.0.1:8000/ | grep -q KaradelikDev && ok "PHP yerleşik sunucu" || bad "phpserver" "yanıt yok"
for s in redis:6379 memcached:11211 phpserver:8000; do n=${s%%:*}; p=${s##*:}
  do_ svc:$n:stop; waitfor 15 "! port $p" && ok "$n durdur" || bad "$n stop" "port $p açık"
done

echo "== PHP"
if [ -z "$MAC" ]; then
do_ php:ext:xsl; waitfor 10 'grep -q "^;extension=xsl" "$(INI)"' && ok "eklenti kapat (xsl)" || bad "ext off" "değişmedi"
sleep 3; do_ php:ext:xsl; waitfor 10 'grep -q "^extension=xsl" "$(INI)"' && ok "eklenti aç (xsl)" || bad "ext on" "değişmedi"
else
# Homebrew PHP: çekirdek eklentiler derlemede; opcache aç/kapa opcache.enable ile
sleep 1; do_ php:quick:opcache; waitfor 10 'grep -q "^opcache.enable = 0" "$(INI)"' && ok "opcache kapat (opcache.enable)" || bad "opcache off" "değişmedi"
sleep 3; do_ php:quick:opcache; waitfor 10 'grep -q "^opcache.enable = 1" "$(INI)"' && ok "opcache aç" || bad "opcache on" "değişmedi"
fi
sleep 3; do_ php:quick:memory_limit; waitfor 10 'grep -q "^memory_limit = 128M" "$(INI)"' && ok "hızlı ayar memory_limit" || bad "quick" "değişmedi"
sleep 3; do_ php:quick:memory_limit; waitfor 10 'grep -q "^memory_limit = 512M" "$(INI)"' && ok "hızlı ayar geri" || bad "quick geri" "değişmedi"
cur=$(phpver); other=$(ls $R/bin/php | grep -v "^$cur$" | grep -v nts | head -1)
if [ -n "$other" ]; then
  sleep 3; do_ ver:php:$other; waitfor 30 'curl -s $L/ | grep -q "PHP $(pv $other)"' && ok "PHP sürüm değiştir → $other" || bad "php switch" "$(curl -s $L/ | grep -oE 'PHP [0-9.]+')"
  sleep 2; do_ ver:php:$cur; waitfor 30 'curl -s $L/ | grep -q "PHP $(pv $cur)"' && ok "PHP sürüm geri → $cur" || bad "php switch back" "$(curl -s $L/ | grep -oE 'PHP [0-9.]+')"
fi
do_ php:sendmail; sleep 1; clip | grep -q "sendmail_path" && ok "sendmail_path panoya" || bad "sendmail" "pano boş"
echo '<?php var_dump(mail("a@b.c","kdtest-mail","x"));' > $R/www/demo/m.php; curl -s $L/demo/m.php >/dev/null
waitfor 10 'curl -s http://127.0.0.1:8025/api/v1/messages | grep -q kdtest-mail' && ok "mail() → Mailpit" || bad "mail" "Mailpit'e düşmedi"; rm -f $R/www/demo/m.php

echo "== SSL / sertifikalar"
t0=$(mtime $R/etc/ssl/demo.test.crt); sleep 1
do_ ssl_regen; waitfor 20 '[ $(mtime $R/etc/ssl/demo.test.crt) -gt $t0 ]' && ok "sertifikaları yeniden üret" || bad "ssl_regen" "yenilenmedi"
waitfor 15 '[ "$(curl -sk -o /dev/null -w "%{http_code}" https://127.0.0.1:$APS/)" = 200 ]' && ok "HTTPS yanıt" || bad "https" "yanıt yok"
t1=$(mtime $R/etc/ssl/cacert.pem); sleep 1; do_ cacert_update
waitfor 30 '[ $(mtime $R/etc/ssl/cacert.pem) -gt $t1 ]' && ok "cacert.pem güncelle" || bad "cacert" "değişmedi"

echo "== Profil"
do_ profile_create kdprof; waitfor 10 '[ -f $R/usr/profile/kdprof.ini ] && grep -q "Profile=kdprof" $R/usr/karadelik.ini' && ok "profil oluştur + geç" || bad "profile_create" "yok"
# Profil değişince çalışan servisler arka planda yeniden başlatılır; sonraki testler onları açık bekler
do_ profile:default; waitfor 30 'grep -q "Profile=default" $R/usr/karadelik.ini' && sleep 2 && waitfor 60 'port $AP && port $MP' && ok "profile geri dön (servisler yeniden başladı)" || bad "profile switch" "değişmedi"
rm -f $R/usr/profile/kdprof.ini

echo "== Paket kurulumu"
if [ -z "$MAC" ]; then
rm -rf $R/bin/cloudflared
do_ pkg:cloudflared; waitfor 120 '[ -f $R/bin/cloudflared/cloudflared.exe ]' && ok "Hızlı ekle (cloudflared)" || bad "pkg" "dosya yok"
"$R/bin/cloudflared/cloudflared.exe" --version >/dev/null 2>&1 && ok "indirilen araç çalışıyor" || bad "cloudflared" "çalışmıyor"
else
# macOS: indirilen zip'teki ikili çalıştırılabilir kalmalı (ngrok); Homebrew paketi bağlantı olarak eklenir
rm -rf $R/bin/ngrok
do_ pkg:ngrok; waitfor 120 '[ -x $R/bin/ngrok/ngrok ]' && ok "Hızlı ekle (ngrok, zip)" || bad "pkg" "dosya yok"
"$R/bin/ngrok/ngrok" version >/dev/null 2>&1 && ok "indirilen araç çalışıyor" || bad "ngrok" "çalışmıyor"
fi

if [ -z "$MAC" ]; then
echo "== Windows entegrasyonu (geri alınarak)"
# Kök klasörün Windows yolu (ör. D:\a\karadelikdev\karadelikdev\runtime)
RW=$(cygpath -w "$(cd "$R" && pwd)")
do_ path_add; waitfor 10 'reg query "HKCU\\Environment" -v Path | grep -qiF "$RW"' && ok "PATH'e ekle" || bad "path_add" "yok"
do_ path_remove; waitfor 10 '! reg query "HKCU\\Environment" -v Path | grep -qiF "$RW"' && ok "PATH'ten çıkar" || bad "path_remove" "duruyor"
do_ ctx_toggle; waitfor 10 'reg query "HKCU\\Software\\Classes\\Directory\\shell\\KaradelikTerminal" >/dev/null 2>&1' && ok "sağ tık menüsü ekle" || bad "ctx add" "yok"
do_ ctx_toggle; waitfor 10 '! reg query "HKCU\\Software\\Classes\\Directory\\shell\\KaradelikTerminal" >/dev/null 2>&1' && ok "sağ tık menüsü kaldır" || bad "ctx remove" "duruyor"
else
echo "== macOS entegrasyonu (geri alınarak)"
ZP=$HOME/.zprofile; ZB=$(mktemp); cp -p "$ZP" "$ZB" 2>/dev/null; had=$([ -f "$ZP" ] && echo 1)
do_ path_add; waitfor 10 'grep -q "KaradelikDev PATH" "$ZP" && grep -q "$R/usr/bin" "$ZP"' && ok "PATH'e ekle (~/.zprofile)" || bad "path_add" "blok yok"
do_ path_remove; waitfor 10 '! grep -q "KaradelikDev PATH" "$ZP"' && ok "PATH'ten çıkar" || bad "path_remove" "blok duruyor"
if [ -n "$had" ]; then cmp -s "$ZP" "$ZB" && ok "~/.zprofile eski haline döndü" || { bad "zprofile" "içerik değişti (yedek: $ZB)"; }; else [ -s "$ZP" ] || rm -f "$ZP"; fi
WF="$HOME/Library/Services/KaradelikDev Terminal.workflow"
do_ ctx_toggle; waitfor 10 '[ -f "$WF/Contents/document.wflow" ]' && ok "Finder Hızlı Eylemi ekle" || bad "ctx add" "yok"
plutil -lint "$WF/Contents/document.wflow" >/dev/null && plutil -lint "$WF/Contents/Info.plist" >/dev/null && ok "Hızlı Eylem plist'leri geçerli" || bad "workflow" "plist bozuk"
do_ ctx_toggle; waitfor 10 '[ ! -e "$WF" ]' && ok "Finder Hızlı Eylemi kaldır" || bad "ctx remove" "duruyor"
fi

echo "== CLI"
"$K" reload; sleep 3; waitfor 20 'port $AP' && ok "karadelikdev reload" || bad "reload" "apache kapalı"
"$K" stop mysql; waitfor 20 '! port $MP' && ok "karadelikdev stop mysql" || bad "cli stop" "açık"
"$K" start mysql; waitfor 30 'port $MP' && ok "karadelikdev start mysql" || bad "cli start" "kapalı"

echo "== Hepsini durdur"
do_ stopall; waitfor 40 '! port $AP && ! port $MP && ! port $MAILP' && ok "Tümünü durdur" || bad "stopall" "açık port var"
echo; echo "SONUÇ: $P başarılı, $F başarısız"
for f in "${FAILS[@]}"; do echo "  - $f"; done
[ "$F" -eq 0 ]
