#!/usr/bin/env bash
# KaradelikDev macOS sürüm paketi: .app ve .dmg'yi derler; dist/ altında
#   KaradelikDev-<sürüm>-macos-<mimari>.dmg         (kullanıcıların indirdiği kurulum görüntüsü)
#   KaradelikDev-<sürüm>-macos-<mimari>.app.tar.gz  (uygulama içi güncelleyicinin indirdiği arşiv)
# üretir, SHA256SUMS.txt'ye ekler ve latest.json'a "platforms" girdilerini yazar.
#
#   bash scripts/release.sh                  # bu Mac'in mimarisi (Apple Silicon: aarch64)
#   bash scripts/release.sh --universal      # aarch64 + x86_64 ayrı ayrı (rustup target add x86_64-apple-darwin)
#   bash scripts/release.sh --no-build       # mevcut derlemeyi paketle
#
# İki platform birlikte yayınlanırken sıra: Windows'ta scripts\release.ps1 -> dist\ içindeki setup, SHA256SUMS.txt ve
# latest.json bu Mac'teki dist/ klasörüne kopyalanır -> bu betik onları silmeden macOS girdilerini ekler.
# latest.json'daki eski alanlar (url/sha256 = Windows kurulum programı) korunur; eski Windows istemcileri bozulmaz.
set -euo pipefail
cd "$(dirname "$0")/.."

BUILD=1; ARCHS=()
for a in "$@"; do
  case "$a" in
    --no-build) BUILD= ;;
    --universal) ARCHS=(aarch64 x86_64) ;;
    *) echo "bilinmeyen seçenek: $a" >&2; exit 2 ;;
  esac
done
[ ${#ARCHS[@]} -eq 0 ] && ARCHS=("$(uname -m | sed 's/arm64/aarch64/')")

version=$(sed -nE 's/^version *= *"([^"]+)".*/\1/p' src-tauri/Cargo.toml | head -1)
pkg=$(python3 -c 'import json;print(json.load(open("package.json"))["version"])')
conf=$(python3 -c 'import json;print(json.load(open("src-tauri/tauri.conf.json"))["version"])')
if [ "$version" != "$pkg" ] || [ "$version" != "$conf" ]; then
  echo "Sürüm uyuşmuyor: Cargo.toml=$version package.json=$pkg tauri.conf.json=$conf" >&2; exit 1
fi
notes="docs/releases/$version.md"
[ -f "$notes" ] || { echo "Sürüm notu yok: $notes" >&2; exit 1; }

mkdir -p dist
base="https://github.com/enesbsafak/karadelikdev/releases/download/v$version"
entries=()
for arch in "${ARCHS[@]}"; do
  target="$arch-apple-darwin"
  if [ -n "$BUILD" ]; then
    rustup target list --installed | grep -qx "$target" || rustup target add "$target"
    npm run build -- --target "$target"
  fi
  bundle="src-tauri/target/$target/release/bundle"
  [ -d "$bundle/macos/KaradelikDev.app" ] || bundle="src-tauri/target/release/bundle"
  app="$bundle/macos/KaradelikDev.app"
  dmg=$(ls "$bundle"/dmg/KaradelikDev_"$version"_*.dmg 2>/dev/null | head -1)
  [ -d "$app" ] || { echo ".app yok: $app" >&2; exit 1; }
  [ -n "$dmg" ] || { echo ".dmg yok: $bundle/dmg" >&2; exit 1; }
  # Ad-hoc imza doğrulaması (Apple Developer ID imzası yoksa ilk açılışta Gatekeeper uyarısı çıkar)
  codesign --verify --deep "$app"
  key="macos-$arch"
  tgz="KaradelikDev-$version-$key.app.tar.gz"
  dmg_name="KaradelikDev-$version-$key.dmg"
  tar -C "$(dirname "$app")" -czf "dist/$tgz" KaradelikDev.app
  cp "$dmg" "dist/$dmg_name"
  sha=$(shasum -a 256 "dist/$tgz" | cut -d' ' -f1)
  for f in "$tgz" "$dmg_name"; do
    h=$(shasum -a 256 "dist/$f" | cut -d' ' -f1)
    touch dist/SHA256SUMS.txt
    grep -v " $f\$" dist/SHA256SUMS.txt > dist/SHA256SUMS.tmp || true
    echo "$h $f" >> dist/SHA256SUMS.tmp
    mv dist/SHA256SUMS.tmp dist/SHA256SUMS.txt
  done
  entries+=("$key=$base/$tgz=$sha")
  echo "Hazır: dist/$dmg_name, dist/$tgz"
done

# latest.json: Windows'un ürettiği dosya varsa ona eklenir, yoksa yalnız macOS girdileriyle oluşturulur
python3 - "$version" "$notes" "${entries[@]}" <<'EOF'
import json, os, re, sys
version, notes_file, *entries = sys.argv[1:]
path = "dist/latest.json"
data = json.load(open(path, encoding="utf-8-sig")) if os.path.exists(path) else {}
if data.get("version") not in (None, version):
    sys.exit(f"dist/latest.json başka bir sürüme ait: {data.get('version')}")
data["version"] = version
if "notes" not in data:
    lines = [l.strip() for l in open(notes_file, encoding="utf-8") if re.match(r"^\s*- ", l)]
    data["notes"] = "\n".join(re.sub(r"\*\*|`", "", l) for l in lines)
plat = data.setdefault("platforms", {})
if data.get("url") and data.get("sha256"):
    plat.setdefault("windows-x86_64", {"url": data["url"], "sha256": data["sha256"]})
for e in entries:
    key, url, sha = e.split("=", 2)
    plat[key] = {"url": url, "sha256": sha}
open(path, "w", encoding="utf-8").write(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
print("dist/latest.json:", ", ".join(sorted(plat)))
EOF
cp "$notes" dist/RELEASE_NOTES.md
[ -f "dist/KaradelikDev-$version-setup.exe" ] || echo "Not: Windows kurulum programı dist/ içinde yok; latest.json'daki url/sha256 alanları Windows derlemesinden gelmeli."
assets=$(cd dist && ls | grep -v RELEASE_NOTES.md | sed 's#^#dist/#' | tr '\n' ' ')
echo "Önce main'i push edin, sonra: gh release create v$version $assets--title \"KaradelikDev $version\" --notes-file $notes"
