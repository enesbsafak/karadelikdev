#!/usr/bin/env bash
# PostgreSQL, MongoDB, MariaDB: indir, başlat, sorgula, durdur.
# Kullanım: KaradelikDev (debug) çalışırken  bash tests/functest-extra-services.sh
cd "$(dirname "$0")/.." || exit 1
K=./src-tauri/target/debug/karadelikdev.exe
R=runtime
P=0; F=0; FAILS=()
ok()   { echo "  ✔ $1"; P=$((P+1)); }
bad()  { echo "  ✘ $1  →  $2"; F=$((F+1)); FAILS+=("$1: $2"); }
port() { netstat -ano | grep LISTEN | grep -qE "[:.]$1 " ; }
waitfor() { local n=$1; shift; for _ in $(seq 1 "$n"); do eval "$@" && return 0; sleep 1; done; return 1; }
do_() { "$K" do "$@"; }

echo "== PostgreSQL"
do_ pkg:postgresql-18.2
waitfor 300 'ls $R/bin/postgresql/*/pgsql/bin/pg_ctl.exe >/dev/null 2>&1 || ls $R/bin/postgresql/*/bin/pg_ctl.exe >/dev/null 2>&1' && ok "PostgreSQL indir + aç" || bad "pg pkg" "pg_ctl yok"
sleep 2; do_ svc:postgresql:start
waitfor 60 'port 5432' && ok "PostgreSQL başlat (initdb dahil)" || bad "pg start" "5432 kapalı ($(tail -2 $R/logs/postgresql.log 2>/dev/null))"
PSQL=$(ls $R/bin/postgresql/*/pgsql/bin/psql.exe $R/bin/postgresql/*/bin/psql.exe 2>/dev/null | head -1)
[ -n "$PSQL" ] && "$PSQL" -h 127.0.0.1 -U postgres -tAc "SELECT 'pg ok ' || version()" 2>&1 | grep -q "pg ok" && ok "psql sorgusu (postgres kullanıcısı, parolasız)" || bad "psql" "sorgu başarısız"
do_ svc:postgresql:stop
waitfor 30 '! port 5432' && ok "PostgreSQL durdur" || bad "pg stop" "açık"

echo "== MongoDB"
do_ pkg:mongodb-8.0
waitfor 400 'ls $R/bin/mongodb/*/bin/mongod.exe >/dev/null 2>&1' && ok "MongoDB indir + aç" || bad "mongo pkg" "mongod yok"
sleep 2; do_ svc:mongodb:start
waitfor 60 'port 27017' && ok "MongoDB başlat" || bad "mongo start" "27017 kapalı ($(tail -c 300 $R/logs/mongod.log 2>/dev/null))"
do_ svc:mongodb:stop
waitfor 30 '! port 27017' && ok "MongoDB durdur" || bad "mongo stop" "açık"

echo "== MariaDB"
do_ pkg:mariadb-11.8
waitfor 300 'ls $R/bin/mysql/mariadb-11.8*/bin/mysqld.exe >/dev/null 2>&1 || ls $R/bin/mysql/mariadb-11.8*/bin/mariadbd.exe >/dev/null 2>&1' && ok "MariaDB indir + aç" || bad "mariadb pkg" "exe yok"
MDB=$(ls -d $R/bin/mysql/mariadb-11.8* 2>/dev/null | head -1 | xargs -n1 basename 2>/dev/null)
CUR=$(grep -A1 '^\[mysql\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r')
do_ startall; waitfor 40 'port 3306' || true
N=$(wc -l < $R/logs/karadelik.log); do_ "ver:mysql:$MDB"
waitfor 90 'tail -n +$((N+1)) $R/logs/karadelik.log | grep -q "MySQL: $MDB" && port 3306' && ok "MariaDB'ye geç (veri dizini oluşturma dahil)" || bad "mariadb switch" "$(tail -3 $R/logs/karadelik.log | tr '\n' ' ')"
"$R/bin/mysql/$MDB/bin/mysql.exe" --default-character-set=utf8mb4 -u root -h 127.0.0.1 -N -e "SELECT VERSION()" 2>&1 | grep -qi mariadb && ok "MariaDB sorgusu (parolasız root)" || bad "mariadb query" "$("$R/bin/mysql/$MDB/bin/mysql.exe" -u root -h 127.0.0.1 -N -e 'SELECT VERSION()' 2>&1 | head -1)"
N=$(wc -l < $R/logs/karadelik.log); do_ "ver:mysql:$CUR"
waitfor 60 'tail -n +$((N+1)) $R/logs/karadelik.log | grep -q "MySQL: $CUR" && port 3306' && ok "MySQL'e geri dön ($CUR)" || bad "switch back" "olmadı"
do_ stopall; waitfor 40 '! port 80 && ! port 3306' && ok "Tümünü durdur" || bad "stopall" "açık port"
echo; echo "SONUÇ: $P başarılı, $F başarısız"
for f in "${FAILS[@]}"; do echo "  - $f"; done
