#!/usr/bin/env bash
# İndirme adreslerinin hâlâ açıldığını denetler (haftalık CI: .github/workflows/url-check.yml).
# Varsayılan packages.conf (Windows), packages.mac.conf ve sites.conf'taki http(s) adresleri ile
# koddaki sabit adresler denenir. PHP (downloads.php.net) ve Apache Lounge adresleri burada atlanır:
# onların eski dosyaları kaldırılır, kurulumda güncel derleme aranır; bunu Rust testi denetler
# (cargo test --lib -- --ignored rolling_sources_resolve).
# Kullanım: scripts/check-urls.sh   (kök dizinden)
set -uo pipefail

UA="KaradelikDev/1.0 (+https://github.com/enesbsafak/karadelikdev)"
D=src-tauri/resources/defaults

urls=()
for f in "$D/packages.conf" "$D/packages.mac.conf" "$D/sites.conf"; do
  while IFS= read -r line; do
    line=${line%$'\r'}
    [[ $line =~ ^[[:space:]]*[#\;] ]] && continue
    u=${line#*=}
    [[ $line == *=* && $u == http* ]] || continue
    case $u in
      *windows.php.net/downloads/releases/*|*downloads.php.net/~windows/releases/*|https://www.apachelounge.com/download/*) continue ;;
    esac
    if [[ $u == *"{arch}"* ]]; then
      urls+=("${u//\{arch\}/arm64}" "${u//\{arch\}/amd64}")
    else
      urls+=("$u")
    fi
  done < "$f"
done
# Koddaki sabit adresler (scheduler.rs, setup.rs, phpext.rs)
urls+=(
  "https://curl.se/ca/cacert.pem"
  "https://aka.ms/vc14/vc_redist.x64.exe"
  "https://downloads.php.net/~windows/pecl/releases/xdebug/"
)

check() {
  local code
  code=$(curl -sS -o /dev/null -L -I -A "$UA" --retry 2 --retry-delay 5 --max-time 60 -w '%{http_code}' "$1" 2>/dev/null)
  # HEAD'i reddeden sunucular için ilk baytı GET ile iste
  if [[ $code != 200 ]]; then
    code=$(curl -sS -o /dev/null -L -r 0-0 -A "$UA" --retry 2 --retry-delay 5 --max-time 60 -w '%{http_code}' "$1" 2>/dev/null)
  fi
  echo "$code"
}

fail=0
for u in $(printf '%s\n' "${urls[@]}" | sort -u); do
  code=$(check "$u")
  if [[ $code == 200 || $code == 206 ]]; then
    echo "ok   $code $u"
  else
    echo "FAIL $code $u"
    [[ -n ${GITHUB_STEP_SUMMARY:-} ]] && echo "- \`$code\` $u" >> "$GITHUB_STEP_SUMMARY"
    fail=1
  fi
done
exit $fail
