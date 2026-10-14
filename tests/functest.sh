#!/usr/bin/env bash
# KaradelikDev işlev testi. Çalışan uygulamaya `karadelikdev do <eylem>` ile komut gönderir,
# etkisini dış dünyadan (port, HTTP, MySQL, dosya, kayıt defteri) doğrular.
# Kullanım: KaradelikDev (debug, kök: runtime) çalışırken  bash tests/functest.sh
# Not: yeni test projesi hosts dosyasına eklenirken UAC sorulabilir.
cd "$(dirname "$0")/.." || exit 1
K=./src-tauri/target/debug/karadelikdev.exe
R=runtime
P=0; F=0; FAILS=()
ok()   { echo "  ✔ $1"; P=$((P+1)); }
bad()  { echo "  ✘ $1  →  $2"; F=$((F+1)); FAILS+=("$1: $2"); }
port() { netstat -ano | grep LISTEN | grep -qE "[:.]$1 " ; }
waitfor() { local n=$1; shift; for _ in $(seq 1 "$n"); do eval "$@" && return 0; sleep 1; done; return 1; }
do_() { "$K" do "$@"; }
mysqlbin() { ls -d $R/bin/mysql/$(grep -A1 '^\[mysql\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r')/bin; }
q() { MYSQL_PWD="${PW:-}" "$(mysqlbin)/mysql.exe" --default-character-set=utf8mb4 -u "${QU:-root}" -h 127.0.0.1 -N -e "$1" 2>&1; }
http() { curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$@"; }
phpver() { grep -A1 '^\[php\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r'; }
INI() { echo "$R/etc/php/$(phpver)/php.ini"; }

echo "== Servisler"
do_ startall
waitfor 40 'port 80 && port 3306 && port 1025' && ok "Tümünü başlat (Apache, MySQL, Mailpit)" || bad "Tümünü başlat" "portlar açılmadı"
[ "$(http http://localhost/)" = 200 ] && ok "localhost başlangıç sayfası" || bad "localhost" "HTTP $(http http://localhost/)"
for a in phpmyadmin adminer redis; do c=$(http http://localhost/$a/); [ "$c" = 200 ] && ok "/$a/ alias" || bad "/$a/ alias" "HTTP $c"; done
[ "$(http http://127.0.0.1:8025/)" = 200 ] && ok "Mailpit web arayüzü" || bad "Mailpit UI" "kapalı"

echo "== MySQL parolasız varsayılan"
do_ dbcall:change_root ""
waitfor 15 '[ "$(q "SELECT 1")" = 1 ]' && ok "root parolası kaldırıldı, parolasız giriş" || bad "parolasız root" "$(q 'SELECT 1' | head -1)"
grep -qE "^RootPassword\.mysql-8\.4=\s*$" $R/usr/karadelik.ini && ok "parola kaydı (mysql-8.4) boş" || bad "parola kaydı" "$(grep RootPassword $R/usr/karadelik.ini | tr '\n' ' ')"

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
do_ dbcall:restore "$(cygpath -w "$BK")" kdtest
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
sleep 4; waitfor 40 'port 3306 && [ "$(q "SELECT 1")" = 1 ]' && ok "root parolasını sıfırla (parolasız, MySQL yeniden başladı)" || bad "reset_root" "$(q 'SELECT 1' | head -1)"

echo "== Otomatik sanal host"
mkdir -p $R/www/kdauto/public && echo '<?php echo "auto ", PHP_VERSION;' > $R/www/kdauto/public/index.php && echo ok > $R/www/kdauto/artisan
waitfor 25 '[ -f $R/etc/apache2/sites-enabled/auto.kdauto.test.conf ]' && ok "yeni klasör algılandı → vhost" || bad "auto vhost" "conf yok"
grep -q "kdauto/public" $R/etc/apache2/sites-enabled/auto.kdauto.test.conf && ok "Laravel public algılama" || bad "public algılama" "yanlış DocumentRoot"
waitfor 15 'curl -s -H "Host: kdauto.test" http://127.0.0.1/ | grep -q auto' && ok "kdauto.test yanıt veriyor" || bad "kdauto.test" "yanıt yok"
waitfor 30 'grep -q kdauto.test /c/Windows/System32/drivers/etc/hosts' && ok "hosts dosyasına eklendi" || bad "hosts" "eklenmedi (UAC reddedildi mi?)"
rm -rf $R/www/kdauto
waitfor 25 '[ ! -f $R/etc/apache2/sites-enabled/auto.kdauto.test.conf ]' && ok "klasör silinince vhost kaldırıldı" || bad "vhost temizliği" "conf duruyor"
do_ vhosts_delete_auto; sleep 2
[ -z "$(ls $R/etc/apache2/sites-enabled/ | grep '^auto\.')" ] && ok "Tüm [auto] dosyalarını sil" || bad "delete_auto" "dosyalar duruyor"
do_ vhosts_sync
waitfor 15 '[ -f $R/etc/apache2/sites-enabled/auto.demo.test.conf ]' && ok "vhost yeniden senkron" || bad "vhosts_sync" "demo conf yok"

echo "== Hızlı uygulama"
do_ quick_create:Blank kdblank
waitfor 20 '[ -d $R/www/kdblank ] && q "SHOW DATABASES" | grep -qx kdblank' && ok "Blank proje + otomatik veritabanı" || bad "Blank" "klasör/db yok"
do_ quick_create:WordPress kdwp
waitfor 120 '[ -f $R/www/kdwp/wp-config.php ]' && ok "WordPress indir + wp-config" || bad "WordPress" "wp-config yok"
grep -q "'DB_NAME', 'kdwp'" $R/www/kdwp/wp-config.php 2>/dev/null && ok "wp-config veritabanı adı" || bad "wp-config" "DB_NAME yanlış"
waitfor 20 '[ "$(http -H "Host: kdwp.test" http://127.0.0.1/wp-admin/install.php)" = 200 ]' && ok "WordPress kurulum sayfası" || bad "WordPress sayfa" "HTTP $(http -H 'Host: kdwp.test' http://127.0.0.1/wp-admin/install.php)"
do_ cloneproj_do:kdwp kdwp2
waitfor 60 '[ -f $R/www/kdwp2/wp-config.php ] && q "SHOW DATABASES" | grep -qx kdwp2' && ok "projeyi klonla (dosya + veritabanı)" || bad "clone" "eksik"
grep -q "'kdwp2'" $R/www/kdwp2/wp-config.php 2>/dev/null && ok "klon wp-config güncellendi" || bad "klon wp-config" "eski ad"
for p in kdblank kdwp kdwp2; do do_ delproj_do:$p; done
waitfor 30 '[ ! -d $R/www/kdblank ] && [ ! -d $R/www/kdwp ] && [ ! -d $R/www/kdwp2 ]' && ok "projeyi sil (3 proje)" || bad "delproj" "klasör duruyor"
waitfor 10 '! q "SHOW DATABASES" | grep -qE "^(kdblank|kdwp|kdwp2)$"' && ok "silinen projelerin veritabanları kaldırıldı" || bad "delproj db" "db duruyor"

echo "== Apache / Nginx / php-cgi"
do_ svc:apache:reload; sleep 2
waitfor 20 'port 80 && [ "$(http http://localhost/)" = 200 ]' && ok "Apache yeniden yükle" || bad "apache reload" "kapalı"
do_ svc:nginx:start
waitfor 20 'port 8080 && port 9001' && ok "Nginx + php-cgi havuzu" || bad "nginx" "port yok"
waitfor 10 'curl -s -H "Host: demo.test" http://127.0.0.1:8080/ | grep -q demo' && ok "Nginx üzerinden PHP" || bad "nginx php" "yanıt yok"
pid=$(netstat -ano | grep LISTEN | grep "127.0.0.1:9001 " | awk '{print $5}' | head -1); taskkill //F //PID $pid >/dev/null 2>&1
waitfor 15 'port 9001' && ok "ölen php-cgi yeniden başlatıldı (izleyici)" || bad "php-cgi monitor" "9001 geri gelmedi"
do_ svc:nginx:reload; sleep 2; port 8080 && ok "Nginx yeniden yükle" || bad "nginx reload" "kapalı"
do_ svc:nginx:stop
waitfor 15 '! port 8080' && ok "Nginx durdur" || bad "nginx stop" "açık"
waitfor 10 '! port 9001' && ok "php-cgi havuzu kapandı (mod_php kullanılıyor)" || bad "php-cgi release" "hâlâ açık"

echo "== Diğer servisler"
for s in redis:6379 memcached:11211 phpserver:8000; do n=${s%%:*}; p=${s##*:}
  do_ svc:$n:start; waitfor 15 "port $p" && ok "$n başlat" || bad "$n start" "port $p kapalı"
done
[ "$("$R/bin/redis/redis-x64-5.0.14.1/redis-cli.exe" -p 6379 PING 2>&1)" = PONG ] && ok "Redis PING" || bad "redis ping" "yanıt yok"
curl -s http://127.0.0.1:8000/ | grep -q KaradelikDev && ok "PHP yerleşik sunucu" || bad "phpserver" "yanıt yok"
for s in redis:6379 memcached:11211 phpserver:8000; do n=${s%%:*}; p=${s##*:}
  do_ svc:$n:stop; waitfor 15 "! port $p" && ok "$n durdur" || bad "$n stop" "port $p açık"
done

echo "== PHP"
cp "$(INI)" /tmp/php.ini.bak
do_ php:ext:xsl; waitfor 10 'grep -q "^;extension=xsl" "$(INI)"' && ok "eklenti kapat (xsl)" || bad "ext off" "değişmedi"
sleep 3; do_ php:ext:xsl; waitfor 10 'grep -q "^extension=xsl" "$(INI)"' && ok "eklenti aç (xsl)" || bad "ext on" "değişmedi"
sleep 3; do_ php:quick:memory_limit; waitfor 10 'grep -q "^memory_limit = 128M" "$(INI)"' && ok "hızlı ayar memory_limit" || bad "quick" "değişmedi"
sleep 3; do_ php:quick:memory_limit; waitfor 10 'grep -q "^memory_limit = 512M" "$(INI)"' && ok "hızlı ayar geri" || bad "quick geri" "değişmedi"
cur=$(phpver); other=$(ls $R/bin/php | grep -v "^$cur$" | grep -v nts | head -1)
if [ -n "$other" ]; then
  sleep 3; do_ ver:php:$other; waitfor 30 'curl -s http://localhost/ | grep -q "PHP ${other:4:6}"' && ok "PHP sürüm değiştir → $other" || bad "php switch" "$(curl -s http://localhost/ | grep -oE 'PHP [0-9.]+')"
  sleep 2; do_ ver:php:$cur; waitfor 30 'curl -s http://localhost/ | grep -q "PHP ${cur:4:6}"' && ok "PHP sürüm geri → $cur" || bad "php switch back" "$(curl -s http://localhost/ | grep -oE 'PHP [0-9.]+')"
fi
do_ php:sendmail; sleep 1; powershell -NoProfile -Command Get-Clipboard | grep -q "sendmail_path" && ok "sendmail_path panoya" || bad "sendmail" "pano boş"
echo '<?php var_dump(mail("a@b.c","kdtest-mail","x"));' > $R/www/demo/m.php; curl -s http://localhost/demo/m.php >/dev/null
waitfor 10 'curl -s http://127.0.0.1:8025/api/v1/messages | grep -q kdtest-mail' && ok "mail() → Mailpit" || bad "mail" "Mailpit'e düşmedi"; rm -f $R/www/demo/m.php

echo "== SSL / sertifikalar"
t0=$(stat -c %Y $R/etc/ssl/demo.test.crt 2>/dev/null || echo 0); sleep 1
do_ ssl_regen; waitfor 20 '[ $(stat -c %Y $R/etc/ssl/demo.test.crt 2>/dev/null || echo 0) -gt $t0 ]' && ok "sertifikaları yeniden üret" || bad "ssl_regen" "yenilenmedi"
waitfor 15 '[ "$(curl -sk -o /dev/null -w "%{http_code}" https://127.0.0.1/)" = 200 ]' && ok "HTTPS yanıt" || bad "https" "yanıt yok"
t1=$(stat -c %Y $R/etc/ssl/cacert.pem); sleep 1; do_ cacert_update
waitfor 30 '[ $(stat -c %Y $R/etc/ssl/cacert.pem) -gt $t1 ]' && ok "cacert.pem güncelle" || bad "cacert" "değişmedi"

echo "== Profil"
do_ profile_create kdprof; waitfor 10 '[ -f $R/usr/profile/kdprof.ini ] && grep -q "Profile=kdprof" $R/usr/karadelik.ini' && ok "profil oluştur + geç" || bad "profile_create" "yok"
do_ profile:default; waitfor 30 'grep -q "Profile=default" $R/usr/karadelik.ini' && ok "profile geri dön" || bad "profile switch" "değişmedi"
rm -f $R/usr/profile/kdprof.ini

echo "== Paket kurulumu"
rm -rf $R/bin/cloudflared
do_ pkg:cloudflared; waitfor 120 '[ -f $R/bin/cloudflared/cloudflared.exe ]' && ok "Hızlı ekle (cloudflared)" || bad "pkg" "dosya yok"
"$R/bin/cloudflared/cloudflared.exe" --version >/dev/null 2>&1 && ok "indirilen araç çalışıyor" || bad "cloudflared" "çalışmıyor"

echo "== Windows entegrasyonu (geri alınarak)"
do_ path_add; waitfor 10 'reg query "HKCU\\Environment" -v Path | grep -qi "lokaldev\\\\runtime"' && ok "PATH'e ekle" || bad "path_add" "yok"
do_ path_remove; waitfor 10 '! reg query "HKCU\\Environment" -v Path | grep -qi "lokaldev\\\\runtime"' && ok "PATH'ten çıkar" || bad "path_remove" "duruyor"
do_ ctx_toggle; waitfor 10 'reg query "HKCU\\Software\\Classes\\Directory\\shell\\KaradelikTerminal" >/dev/null 2>&1' && ok "sağ tık menüsü ekle" || bad "ctx add" "yok"
do_ ctx_toggle; waitfor 10 '! reg query "HKCU\\Software\\Classes\\Directory\\shell\\KaradelikTerminal" >/dev/null 2>&1' && ok "sağ tık menüsü kaldır" || bad "ctx remove" "duruyor"

echo "== CLI"
"$K" reload; sleep 3; port 80 && ok "karadelikdev reload" || bad "reload" "apache kapalı"
"$K" stop mysql; waitfor 20 '! port 3306' && ok "karadelikdev stop mysql" || bad "cli stop" "açık"
"$K" start mysql; waitfor 30 'port 3306' && ok "karadelikdev start mysql" || bad "cli start" "kapalı"

echo "== Hepsini durdur"
do_ stopall; waitfor 40 '! port 80 && ! port 3306 && ! port 1025' && ok "Tümünü durdur" || bad "stopall" "açık port var"
echo; echo "SONUÇ: $P başarılı, $F başarısız"
for f in "${FAILS[@]}"; do echo "  - $f"; done
