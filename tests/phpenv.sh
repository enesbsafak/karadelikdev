#!/usr/bin/env bash
# PHP eklenti kurulumu ve terminalde proje başına PHP sürümü (usr/bin/php yönlendiricisi). Uygulama kapalıyken,
# pencere açmadan (`--headless`) çalışır; functest.sh'tan önce çalıştırılırsa servisler kurulan eklentilerle açılır.
# Kullanım: bash tests/phpenv.sh            (kök: runtime veya KARADELIK_ROOT; en az iki PHP sürümü kurulu olmalı)
#           KD_EXTS="xdebug redis" bash tests/phpenv.sh
cd "$(dirname "$0")/.." || exit 1
case "$(uname -s)" in Darwin) MAC=1; X="" ;; *) MAC=; X=.exe ;; esac
K=./src-tauri/target/debug/karadelikdev$X
R=${KARADELIK_ROOT:-runtime}
P=0; F=0; FAILS=()
ok()   { echo "  ✔ $1"; P=$((P+1)); }
bad()  { echo "  ✘ $1  →  $2"; F=$((F+1)); FAILS+=("$1: $2"); }
phpver() { grep -A1 '^\[php\]' $R/usr/profile/default.ini | tail -1 | cut -d= -f2 | tr -d '\r'; }
mm() { echo "$1" | grep -oE '[0-9]+\.[0-9]+' | head -1; }
CUR=$(phpver)
PHPDIR=$R/bin/php/$CUR
PHPBIN=$PHPDIR/php$X; [ -f "$PHPBIN" ] || PHPBIN=$PHPDIR/bin/php
INIDIR=$R/etc/php/$CUR
# Komutu verilen dizinde, KaradelikDev terminalindeki gibi usr/bin PATH'in başındayken çalıştırır
RBIN="$(cd "$R" && pwd)/usr/bin"
in_dir() {
  local d=$1; shift
  if [ -n "$MAC" ]; then
    (cd "$d" && PATH="$RBIN:$PATH" "$@")
  else
    (cd "$d" && PATH="$RBIN:$PATH" cmd //c "$@")
  fi
}

echo "== PHP eklentileri ($CUR)"
for e in ${KD_EXTS:-xdebug redis imagick mongodb pcov apcu igbinary}; do
  out=$("$K" --headless ext-install "$e" 2>&1)
  if [ $? -eq 0 ]; then
    loaded=$(PHPRC=$INIDIR "$PHPBIN" -r "echo extension_loaded('$e') ? 'yes' : 'no';" 2>&1)
    [ "$loaded" = yes ] && ok "$e kuruldu ve yüklü ($(echo "$out" | tail -1))" || bad "$e yüklü" "$loaded"
  else
    bad "$e kurulumu" "$(echo "$out" | tail -3 | tr '\n' ' ')"
  fi
done
if PHPRC=$INIDIR "$PHPBIN" -r "exit(extension_loaded('xdebug') ? 0 : 1);" 2>/dev/null; then
  [ "$(PHPRC=$INIDIR "$PHPBIN" -r "echo ini_get('xdebug.mode');")" = debug ] && ok "xdebug.mode = debug" || bad "xdebug ayarları" "xdebug.mode yok"
fi

echo "== Terminalde proje başına PHP"
OTHER=$(ls $R/bin/php | grep -v "^$CUR\$" | grep -v nts | head -1)
if [ -z "$OTHER" ]; then
  bad "ikinci PHP sürümü" "kurulu değil"
else
  mkdir -p $R/www/kdphp/public $R/www/kdphp2
  [ -f $R/usr/projects.ini ] && cp $R/usr/projects.ini $R/usr/projects.ini.phpenv-bak
  printf '[kdphp]\r\nPhp=%s\r\n' "$OTHER" > $R/usr/projects.ini
  "$K" --headless env >/dev/null
  shim=$R/usr/bin/php; [ -z "$MAC" ] && shim=$shim.cmd
  [ -f "$shim" ] && ok "usr/bin yönlendiricisi oluştu" || bad "yönlendirici" "$shim yok"
  v=$(in_dir $R/www/kdphp php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;" | tr -d '\r')
  [ "$v" = "$(mm "$OTHER")" ] && ok "proje klasöründe php → $v" || bad "proje klasörü" "$v (beklenen $(mm "$OTHER"))"
  v=$(in_dir $R/www/kdphp/public php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;" | tr -d '\r')
  [ "$v" = "$(mm "$OTHER")" ] && ok "alt klasörde php → $v" || bad "alt klasör" "$v"
  ini=$(in_dir $R/www/kdphp php -r "echo php_ini_loaded_file();" | tr -d '\r')
  echo "$ini" | grep -q "$OTHER" && ok "proje sürümünün php.ini'si yüklü" || bad "proje php.ini" "${ini:-yok}"
  v=$(in_dir $R/www/kdphp2 php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;" | tr -d '\r')
  [ "$v" = "$(mm "$CUR")" ] && ok "başka projede profildeki sürüm ($v)" || bad "başka proje" "$v (beklenen $(mm "$CUR"))"
  code=$(in_dir $R/www/kdphp php -r "exit(7);"; echo $?)
  [ "$code" = 7 ] && ok "çıkış kodu korunuyor" || bad "çıkış kodu" "$code"
  arg=$(in_dir $R/www/kdphp php -r 'echo $argv[1];' 'a!b' | tr -d '\r')
  [ "$arg" = 'a!b' ] && ok "özel karakterli argüman" || bad "argüman" "$arg"
  rm -f $R/usr/projects.ini
  rm -rf $R/www/kdphp $R/www/kdphp2
  "$K" --headless env >/dev/null
  [ ! -f "$shim" ] && ok "farklı sürümlü proje kalmayınca yönlendirici silindi" || bad "yönlendirici silinmedi" "$shim"
  if [ -f $R/usr/projects.ini.phpenv-bak ]; then
    mv $R/usr/projects.ini.phpenv-bak $R/usr/projects.ini
    "$K" --headless env >/dev/null
  fi
fi

echo
echo "Sonuç: $P geçti, $F kaldı"
for f in "${FAILS[@]}"; do echo "  - $f"; done
[ $F -eq 0 ]
