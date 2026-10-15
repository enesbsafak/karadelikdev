// Ortak yardımcılar: Tauri köprüsü ve çeviri.
const TAURI = window.__TAURI__;
const invoke = (cmd, args) => TAURI.core.invoke(cmd, args);
const listen = (ev, fn) => TAURI.event.listen(ev, (e) => fn(e.payload));
const currentWindow = () => TAURI.window.getCurrentWindow();

// Platform: macOS'ta pencere düğmeleri yerine sistemin trafik ışıkları kullanılır (bkz. app.css html.mac)
const IS_MAC = /Mac/.test(navigator.platform || navigator.userAgent);
if (IS_MAC) document.documentElement.classList.add('mac');
const SEP = IS_MAC ? '/' : '\\';

let I18N = {};
const T = (key) => I18N[key] ?? key;
const TF = (key, ...args) => args.reduce((s, a, i) => s.replaceAll(`{${i}}`, a), T(key));

async function loadI18n() {
  I18N = await invoke('i18n_table');
  applyI18n();
}

function applyI18n(root = document) {
  root.querySelectorAll('[data-i18n]').forEach((el) => { el.textContent = T(el.dataset.i18n); });
  root.querySelectorAll('[data-i18n-title]').forEach((el) => { el.title = T(el.dataset.i18nTitle); });
  root.querySelectorAll('[data-i18n-ph]').forEach((el) => { el.placeholder = T(el.dataset.i18nPh); });
}

listen('i18n', (table) => { I18N = table; applyI18n(); if (window.onI18n) window.onI18n(); });

function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'class') e.className = v;
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
    else if (v !== false && v != null) e.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat()) if (c != null) e.append(c);
  return e;
}

// Esc ile diyalog/tercih pencerelerini kapat
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && document.body.dataset.escClose) currentWindow().close();
});
