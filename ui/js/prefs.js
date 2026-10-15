// Tercihler penceresi.
const $ = (id) => document.getElementById(id);
const SVC_TITLES = {
  apache: 'Apache', nginx: 'Nginx', mysql: 'MySQL', postgresql: 'PostgreSQL', mongodb: 'MongoDB',
  redis: 'Redis', memcached: 'Memcached', mailpit: 'Mailpit (SMTP)', phpserver: 'PHP Server',
};
const BOOL = ['run_at_startup', 'run_minimized', 'auto_start', 'auto_vhosts', 'manage_hosts', 'auto_backup', 'auto_update', 'app_update_check', 'lan_access'];
const TEXT = ['language', 'document_root', 'data_dir', 'hostname_format', 'vhost_ignore', 'terminal', 'editor', 'tunnel'];
const NUM = ['php_upstream', 'mailpit_ui_port', 'backup_interval', 'backup_keep'];
let settings;

function buildServices() {
  const g = $('svcGrid');
  g.textContent = '';
  g.append(el('span', { class: 'head' }), el('span', { class: 'head' }, 'Port'),
    el('span', { class: 'head' }, 'SSL'), el('span', { class: 'head' }, 'SSL port'));
  for (const id of Object.keys(SVC_TITLES)) {
    const c = settings.services[id];
    if (!c) continue;
    const sslCapable = id === 'apache' || id === 'nginx';
    g.append(
      el('label', { class: 'check', style: 'margin:0' },
        el('input', { type: 'checkbox', 'data-svc': id, 'data-k': 'enabled', checked: c.enabled }),
        el('span', {}, SVC_TITLES[id])),
      el('input', { class: 'field', type: 'number', min: 1, max: 65535, value: c.port, 'data-svc': id, 'data-k': 'port' }),
      sslCapable
        ? el('label', { class: 'check', style: 'margin:0' }, el('input', { type: 'checkbox', 'data-svc': id, 'data-k': 'ssl', checked: c.ssl }))
        : el('span'),
      sslCapable
        ? el('input', { class: 'field', type: 'number', min: 1, max: 65535, value: c.ssl_port, 'data-svc': id, 'data-k': 'ssl_port' })
        : el('span'),
    );
  }
}

function fill(meta) {
  // Terminal seçenekleri platforma göre (Windows: wt/cmd/PowerShell/Cmder, macOS: Terminal/iTerm2)
  const term = $('terminal');
  term.textContent = '';
  for (const [value, label] of meta.terminals) term.append(el('option', { value }, label));
  if (![...term.options].some((o) => o.value === settings.terminal)) settings.terminal = 'auto';
  const lang = $('language');
  lang.textContent = '';
  for (const [code, name] of meta.languages) lang.append(el('option', { value: code }, name));
  for (const k of BOOL) $(k).checked = !!settings[k];
  for (const k of TEXT) $(k).value = settings[k] ?? '';
  for (const k of NUM) $(k).value = settings[k];
  $('run_at_startup').checked = meta.autostart || settings.run_at_startup;
  buildServices();
}

function collect() {
  const s = structuredClone(settings);
  for (const k of BOOL) s[k] = $(k).checked;
  for (const k of TEXT) s[k] = $(k).value.trim();
  for (const k of NUM) s[k] = Math.max(1, parseInt($(k).value, 10) || settings[k]);
  document.querySelectorAll('[data-svc]').forEach((inp) => {
    const c = s.services[inp.dataset.svc];
    const k = inp.dataset.k;
    c[k] = inp.type === 'checkbox' ? inp.checked : (parseInt(inp.value, 10) || c[k]);
  });
  return s;
}

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  document.title = T('ui.prefs');
  settings = await invoke('get_settings');
  fill(await invoke('get_meta'));

  document.querySelectorAll('.tab').forEach((t) => t.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach((x) => x.classList.toggle('active', x === t));
    document.querySelectorAll('.content').forEach((c) => c.classList.toggle('hidden', c.id !== `tab-${t.dataset.tab}`));
  }));
  document.querySelectorAll('[data-pick]').forEach((b) => b.addEventListener('click', async () => {
    const id = b.dataset.pick;
    const p = await invoke('pick_folder', { current: $(id).value });
    if (p) $(id).value = p;
  }));
  $('pickEditor').addEventListener('click', async () => {
    const p = await invoke('pick_file');
    if (p) $('editor').value = p;
  });
  $('cancel').addEventListener('click', () => currentWindow().close());
  $('save').addEventListener('click', async () => {
    try {
      await invoke('save_settings', { new: collect() });
      currentWindow().close();
    } catch (e) {
      $('err').textContent = e;
      $('err').classList.remove('hidden');
    }
  });
});
