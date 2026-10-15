// Ana pencere: servis listesi, alt düğmeler, durum satırı.
const $ = (id) => document.getElementById(id);
let state = { services: [], projects: [] };

function portText(s) {
  return s.ssl ? `:${s.port} · ${s.ssl_port}` : `:${s.port}`;
}

function renderServices(list) {
  const box = $('services');
  box.textContent = '';
  const visible = list.filter((s) => s.enabled || s.running || s.foreign);
  const anyInstalled = list.some((s) => s.installed);
  $('empty').classList.toggle('hidden', anyInstalled);
  for (const s of visible) {
    const cls = ['svc'];
    if (s.running) cls.push('running');
    if (s.foreign) cls.push('foreign');
    if (!s.installed && !s.running) cls.push('disabled');

    const ver = el('div', { class: 'svc-ver', title: s.foreign || s.version || '' });
    if (s.foreign) ver.append(el('span', { class: 'warn' }, `⚠ ${T('ui.foreign')}`));
    else ver.append(s.installed ? s.version : T('ui.not_installed'));

    const actions = el('div', { class: 'svc-actions' });
    if (s.running) {
      actions.append(
        el('button', { class: 'btn', onclick: () => act(`svc:${s.id}:reload`) }, T('btn.reload')),
        el('button', { class: 'btn danger', onclick: () => act(`svc:${s.id}:stop`) }, T('btn.stop')),
      );
    } else if (s.installed && !s.foreign) {
      actions.append(el('button', { class: 'btn', onclick: () => act(`svc:${s.id}:start`) }, T('btn.start')));
    } else if (!s.installed) {
      actions.append(el('button', { class: 'btn', onclick: (e) => openMenu(e.currentTarget) }, T('menu.quick_add')));
    }

    box.append(el('div', { class: cls.join(' ') },
      el('span', { class: 'dot', title: s.running ? T('ui.running') : T('ui.stopped') }),
      el('span', { class: 'svc-name' }, s.title),
      ver,
      el('span', { class: 'svc-port' }, portText(s)),
      actions,
    ));
  }
  const anyRunning = list.some((s) => s.running);
  const start = $('startBtn');
  start.classList.toggle('stop', anyRunning);
  $('startLabel').textContent = anyRunning ? T('btn.stop_all') : T('btn.start_all');
  start.querySelector('svg').innerHTML = anyRunning
    ? '<rect x="6" y="6" width="12" height="12" rx="1.5"/>'
    : '<path d="M7 4.5v15l13-7.5z"/>';
}

function renderProjects(list, docroot) {
  const box = $('projects');
  box.textContent = '';
  $('docroot').textContent = docroot || '';
  if (!list.length) {
    box.append(el('div', { class: 'empty-hint' }, T('ui.no_projects')));
    return;
  }
  for (const p of list) {
    box.append(el('span', { class: 'project', title: p.url, onclick: () => act(`site:${p.name}`) }, p.host));
  }
}

function render() {
  renderServices(state.services);
  renderProjects(state.projects, state.document_root);
  $('profileChip').textContent = `[${state.profile}]`;
  const lg = state.laragon && state.empty;
  $('laragonNotice').classList.toggle('hidden', !lg);
  if (lg) $('laragonText').textContent = TF('ui.import_laragon', state.laragon);
  renderUpdate();
}

function renderUpdate() {
  $('updateNotice').classList.toggle('hidden', !state.update);
  if (state.update) $('updateText').textContent = TF('ui.update_available', state.update);
}

function setStatus(msg, isError = false) {
  const s = $('status');
  s.textContent = msg || '';
  s.title = msg || '';
  s.classList.toggle('error', isError);
}

function act(id) {
  invoke('run_action', { id });
}

function openMenu(target) {
  const r = target.getBoundingClientRect();
  invoke('show_menu', { x: r.left, y: r.bottom + 2 });
}

async function refreshAll() {
  state = await invoke('get_state');
  render();
  if (state.message) setStatus(state.message);
  else setStatus(T('ui.quote'));
}

window.onI18n = () => render();

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  await refreshAll();

  $('menuBtn').addEventListener('click', (e) => openMenu(e.currentTarget));
  $('profileChip').addEventListener('click', (e) => openMenu(e.currentTarget));
  $('prefsBtn').addEventListener('click', () => act('prefs'));
  $('hostsBtn').addEventListener('click', () => act('hosts_edit'));
  $('helpBtn').addEventListener('click', () => act('help'));
  $('importBtn').addEventListener('click', () => act('import_laragon'));
  $('updateBtn').addEventListener('click', () => act('app_update'));
  $('manageProjects').addEventListener('click', (e) => { e.preventDefault(); act('projects'); });
  $('minBtn').addEventListener('click', () => currentWindow().minimize());
  $('closeBtn').addEventListener('click', () => currentWindow().close());
  $('startBtn').addEventListener('click', () => {
    act(state.services.some((s) => s.running) ? 'stopall' : 'startall');
  });
  document.querySelectorAll('[data-action]').forEach((b) => b.addEventListener('click', () => act(b.dataset.action)));

  // Sağ tık: tarayıcı menüsü yerine KaradelikDev menüsü (Laragon: sağ tık = Menü)
  document.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    invoke('show_menu', { x: e.clientX, y: e.clientY });
  });

  listen('services', (list) => {
    state.services = list;
    renderServices(list);
  });
  listen('status', (msg) => setStatus(msg));
  listen('error', (msg) => setStatus(msg, true));
  listen('progress', (p) => {
    const bar = $('progress');
    if (p.finished) { bar.classList.add('hidden'); return; }
    bar.classList.remove('hidden');
    bar.classList.toggle('indeterminate', p.percent == null);
    bar.firstElementChild.style.width = p.percent == null ? '' : `${p.percent}%`;
    const mb = (n) => (n / 1048576).toFixed(1);
    setStatus(p.total ? `${p.label}  ${mb(p.done)} / ${mb(p.total)} MB` : p.label);
  });
  // Proje listesi ve diğer durumlar için hafif periyodik yenileme
  setInterval(async () => {
    const s = await invoke('get_state');
    state.projects = s.projects;
    state.profile = s.profile;
    state.laragon = s.laragon;
    state.empty = s.empty;
    state.document_root = s.document_root;
    state.update = s.update;
    renderUpdate();
    renderProjects(s.projects, s.document_root);
    $('profileChip').textContent = `[${s.profile}]`;
    const lg = s.laragon && s.empty;
    $('laragonNotice').classList.toggle('hidden', !lg);
  }, 5000);
});
