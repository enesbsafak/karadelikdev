#!/usr/bin/env bash
# PostgreSQL, MongoDB, MariaDB: indir (macOS: Homebrew), başlat, sorgula, durdur.
# Kullanım: KaradelikDev (debug) çalışırken  bash tests/functest-extra-services.sh
# macOS: paket adları usr/packages.conf'tan (Homebrew) okunur; MySQL portu usr/karadelik.ini'den.
cd "$(dirname "$0")/.." || exit 1
case "$(uname -s)" in Darwin) MAC=1; X="" ;; *) MAC=; X=.exe ;; esac
K=./src-tauri/target/debug/karadelikdev$X
R=${KARADELIK_ROOT:-runtime}
P=0; F=0; FAILS=()
ok()   { echo "  ✔ $1"; P=$((P+1)); }
bad()  { echo "  ✘ $1  →  $2"; F=$((F+1)); FAILS+=("$1: $2"); }
if [ -n "$MAC" ]; then
  port() { lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1; }
  PG_PKG=postgresql-17; MONGO_PKG=mongodb-8.0; MDB_PKG=mariadb-11.8
else
  port() { netstat -ano | grep LISTEN | grep -qE "[:.]$1 " ; }
  PG_PKG=postgresql-18.2; MONGO_PKG=mongodb-8.0; MDB_PKG=mariadb-11.8
fi
cport() { awk -v s="[service.$1]" '$0==s{f=1;next} /^\[/{f=0} f&&/^Port=/{sub("Port=","");gsub("\r","");print;exit}' $R/usr/karadelik.ini; }
AP=$(cport apache); AP=${AP:-80}; MP=$(cport mysql); MP=${MP:-3306}
waitfor() { local n=$1; shift; for _ in $(seq 1 "$n"); do eval "$@" && return 0; sleep 1; done; return 1; }
do_() { "$K" do "$@"; }
first() { for f in "$@"; do [ -e "$f" ] && { echo "$f"; return; }; done; }

echo "== PostgreSQL"
do_ pkg:$PG_PKG
waitfor 600 '[ -n "$(first $R/bin/postgresql/*/pgsql/bin/pg_ctl$X $R/bin/postgresql/*/bin/pg_ctl$X)" ]' && ok "PostgreSQL indir + aç" || bad "pg pkg" "pg_ctl yok"
sleep 2; do_ svc:postgresql:start
waitfor 60 'port 5432' && ok "PostgreSQL başlat (initdb dahil)" || bad "pg start" "5432 kapalı ($(tail -2 $R/logs/postgresql.log 2>/dev/null))"
PSQL=$(first $R/bin/postgresql/*/pgsql/bin/psql$X $R/bin/postgresql/*/bin/psql$X)
[ -n "$PSQL" ] && "$PSQL" -h 127.0.0.1 -U postgres -tAc "SELECT 'pg ok ' || version()" 2>&1 | grep -q "pg ok" && ok "psql sorgusu (postgres kullanıcısı, parolasız)" || bad "psql" "sorgu başarısız"
do_ svc:postgresql:stop
waitfor 30 '! port 5432' && ok "PostgreSQL durdur" || bad "pg stop" "açık"

echo "== MongoDB"
do_ pkg:$MONGO_PKG
waitfor 900 '[ -n "$(first $R/bin/mongodb/*/bin/mongod$X)" ]' && ok "MongoDB indir + aç" || bad "mongo pkg" "mongod yok"
sleep 2; do_ svc:mongodb:start
waitfor 60 'port 27017' && ok "MongoDB başlat" || bad "mongo start" "27017 kapalı ($(tail -c 300 $R/logs/mongod.log 2>/dev/null))"
do_ svc:mongodb:stop
waitfor 30 '! port 27017' && ok "MongoDB durdur" || bad "mongo stop" "açık"

echo "== MariaDB"
do_ pkg:$MDB_PKG
waitfor 900 '[ -n "$(first $R/bin/mysql/mariadb-11.8*/bin/mysqld$X $R/bin/mysql/mariadb-11.8*/bin/mariadbd$X)" ]' && ok "MariaDB indir + aç" || bad "mariadb pkg" "ikili yok"
MDB=$(ls -d $R/bin/mysql/mariadb-11.8* 2>/dev/null | head -1 | xargs -n1 basename 2>/dev/null)
CUR=$(grep -A1 '^\[mysql\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r')
do_ startall; waitfor 40 'port $MP' || true
N=$(wc -l < $R/logs/karadelik.log); do_ "ver:mysql:$MDB"
waitfor 90 'tail -n +$((N+1)) $R/logs/karadelik.log | grep -q "MySQL: $MDB" && port $MP' && ok "MariaDB'ye geç (veri dizini oluşturma dahil)" || bad "mariadb switch" "$(tail -3 $R/logs/karadelik.log | tr '\n' ' ')"
"$R/bin/mysql/$MDB/bin/mysql$X" --default-character-set=utf8mb4 -u root -h 127.0.0.1 -P "$MP" -N -e "SELECT VERSION()" 2>&1 | grep -qi mariadb && ok "MariaDB sorgusu (parolasız root, TCP)" || bad "mariadb query" "$("$R/bin/mysql/$MDB/bin/mysql$X" -u root -h 127.0.0.1 -P "$MP" -N -e 'SELECT VERSION()' 2>&1 | head -1)"
N=$(wc -l < $R/logs/karadelik.log); do_ "ver:mysql:$CUR"
waitfor 60 'tail -n +$((N+1)) $R/logs/karadelik.log | grep -q "MySQL: $CUR" && port $MP' && ok "MySQL'e geri dön ($CUR)" || bad "switch back" "olmadı"
do_ stopall; waitfor 40 '! port $AP && ! port $MP' && ok "Tümünü durdur" || bad "stopall" "açık port"
echo; echo "SONUÇ: $P başarılı, $F başarısız"
for f in "${FAILS[@]}"; do echo "  - $f"; done
