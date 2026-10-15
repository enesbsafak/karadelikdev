// Projeler: proje başına PHP sürümü ve Node/Vite proxy portu, hızlı erişim düğmeleri, hızlı uygulama.
const $ = (id) => document.getElementById(id);
let data = null;

function say(text, isError = false) {
  const m = $('msg');
  m.textContent = text || '';
  m.style.color = isError ? 'var(--err)' : '';
}

const act = (id) => invoke('run_action', { id });

function phpSelect(p) {
  const s = el('select', { class: 'field' });
  s.append(el('option', { value: '' }, TF('projects.php_default', data.current_php || '—')));
  for (const v of data.php_versions) {
    if (v.current) continue;
    s.append(el('option', { value: v.name, disabled: !v.cgi, title: v.cgi ? '' : T('projects.no_cgi') }, v.name));
  }
  s.value = p.php;
  return s;
}

function row(p) {
  const php = phpSelect(p);
  const proxy = el('input', {
    class: 'field small', type: 'number', min: 0, max: 65535, value: p.proxy || '', placeholder: '—',
    title: T('projects.proxy_hint'),
  });
  const save = el('button', { class: 'btn primary hidden' }, T('btn.save'));
  const dirty = () => save.classList.toggle('hidden', php.value === p.php && (parseInt(proxy.value, 10) || 0) === p.proxy);
  php.addEventListener('change', dirty);
  proxy.addEventListener('input', dirty);
  // Proxy etkinken PHP sürümü kullanılmaz
  const syncPhp = () => { php.disabled = (parseInt(proxy.value, 10) || 0) > 0; };
  proxy.addEventListener('input', syncPhp);
  syncPhp();
  save.addEventListener('click', async () => {
    save.disabled = true;
    say(T('db.working'));
    try {
      say(await invoke('project_set', { name: p.name, php: php.value, proxy: parseInt(proxy.value, 10) || 0 }));
      await load();
    } catch (e) {
      say(String(e), true);
      save.disabled = false;
    }
  });
  return el('tr', {},
    el('td', {},
      el('a', { href: '#', onclick: (e) => { e.preventDefault(); act(`site:${p.name}`); }, title: p.url }, p.host),
      el('div', { class: 'muted ellipsis', title: p.web_root }, p.dir)),
    el('td', { class: 'muted' }, p.kind),
    el('td', {}, php),
    el('td', {}, proxy),
    el('td', { class: 'actions' },
      save,
      el('button', { class: 'btn', onclick: () => act(`projdir:${p.name}`) }, T('projects.folder')),
      el('button', { class: 'btn', onclick: () => act(`projterm:${p.name}`) }, T('btn.terminal')),
      el('button', { class: 'btn', onclick: () => act(`share:${p.name}`) }, T('menu.share')),
      el('button', { class: 'btn', onclick: () => act(`cloneproj:${p.name}`) }, T('projects.clone')),
      el('button', { class: 'btn danger', onclick: () => act(`delproj:${p.name}`) }, T('btn.delete'))),
  );
}

function render() {
  $('docroot').textContent = data.document_root;
  const rows = $('rows');
  rows.textContent = '';
  for (const p of data.projects) rows.append(row(p));
  $('none').classList.toggle('hidden', data.projects.length > 0);
  const inv = $('invalid');
  inv.classList.toggle('hidden', !data.invalid.length);
  inv.textContent = data.invalid.length ? TF('projects.invalid', data.invalid.join(', ')) : '';
  const tpl = $('tpl');
  const keep = tpl.value;
  tpl.textContent = '';
  for (const t of data.templates) tpl.append(el('option', { value: t }, t));
  if (keep) tpl.value = keep;
}

async function load() {
  data = await invoke('projects_overview');
  render();
}

window.onI18n = () => render();

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  document.title = T('projects.title');
  await load();
  $('refresh').addEventListener('click', () => { say(''); load(); });
  $('openRoot').addEventListener('click', () => act('root'));
  $('newForm').addEventListener('submit', async (e) => {
    e.preventDefault();
    const name = $('newName').value.trim();
    if (!name) return;
    await invoke('run_action_values', { id: `quick_create:${$('tpl').value}`, values: [name] });
    $('newName').value = '';
    say(TF('projects.creating', name));
  });
  // Klasör eklenince/silinince listeyi güncelle
  setInterval(async () => {
    if (document.activeElement && ['INPUT', 'SELECT'].includes(document.activeElement.tagName)) return;
    const fresh = await invoke('projects_overview');
    if (JSON.stringify(fresh.projects.map((p) => [p.name, p.php, p.proxy])) !== JSON.stringify(data.projects.map((p) => [p.name, p.php, p.proxy]))) {
      data = fresh;
      render();
    }
  }, 4000);
});
