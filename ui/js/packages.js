// Paketler: packages.conf kataloğundan kurulum ve kurulu sürümler arasında geçiş.
const $ = (id) => document.getElementById(id);
let data = null;
let busy = null;

function say(text, isError = false) {
  const m = $('msg');
  m.textContent = text || '';
  m.style.color = isError ? 'var(--err)' : '';
}

const act = (id) => invoke('run_action', { id });
const host = (url) => {
  if (url.startsWith('brew:')) return `Homebrew: ${url.slice(5)}`;
  try { return new URL(url).host; } catch { return url; }
};

function renderCatalog() {
  const q = $('filter').value.trim().toLowerCase();
  const rows = $('pkgRows');
  rows.textContent = '';
  let group = null;
  for (const p of data.packages) {
    if (q && !p.name.toLowerCase().includes(q) && !p.group.toLowerCase().includes(q)) continue;
    if (p.group !== group) {
      group = p.group;
      rows.append(el('tr', {}, el('td', { colspan: 3, class: 'group-row' }, group)));
    }
    let action;
    if (p.installed) {
      action = el('span', { class: 'tag ok' }, T('packages.is_installed'));
    } else {
      action = el('button', { class: 'btn primary', disabled: busy !== null }, busy === p.name ? T('packages.installing') : T('packages.install'));
      action.addEventListener('click', () => {
        busy = p.name;
        say(TF('packages.installing_name', p.name));
        renderCatalog();
        act(`pkg:${p.name}`);
      });
    }
    rows.append(el('tr', {},
      el('td', {}, p.name, p.recommended ? el('span', { class: 'tag', style: 'margin-left:6px' }, T('packages.recommended')) : null),
      el('td', { class: 'muted ellipsis', title: p.url }, host(p.url)),
      el('td', { class: 'actions' },
        p.installed ? el('button', { class: 'btn', onclick: () => act(`open:${p.dest}`) }, T('projects.folder')) : null,
        action)));
  }
}

function renderVersions() {
  const rows = $('verRows');
  rows.textContent = '';
  for (const c of data.versions) {
    c.installed.forEach((v, i) => {
      const isCur = v === c.current;
      rows.append(el('tr', {},
        el('td', { class: i ? 'muted' : '' }, i ? '' : c.title),
        el('td', {}, v, isCur ? el('span', { class: 'tag ok', style: 'margin-left:6px' }, T('packages.active')) : null),
        el('td', { class: 'actions' },
          el('button', { class: 'btn', onclick: () => act(`open:${c.dir}${SEP}${v}`) }, T('projects.folder')),
          isCur ? null : el('button', { class: 'btn primary', onclick: () => { say(TF('packages.switching', c.title, v)); act(`ver:${c.cat}:${v}`); setTimeout(load, 1500); } }, T('packages.use')))));
    });
  }
}

async function load() {
  data = await invoke('packages_overview');
  renderCatalog();
  renderVersions();
}

window.onI18n = () => { renderCatalog(); renderVersions(); };

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  document.title = T('packages.title');
  await load();
  document.querySelectorAll('.tab').forEach((t) => t.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach((x) => x.classList.toggle('active', x === t));
    document.querySelectorAll('.content').forEach((c) => c.classList.toggle('hidden', c.id !== `tab-${t.dataset.tab}`));
  }));
  $('filter').addEventListener('input', renderCatalog);
  $('editConf').addEventListener('click', () => act(`edit:${data.conf}`));
  $('refresh').addEventListener('click', () => { say(''); load(); });

  listen('progress', (p) => {
    const bar = $('progress');
    if (p.finished) {
      bar.classList.add('hidden');
      return;
    }
    bar.classList.remove('hidden');
    bar.classList.toggle('indeterminate', p.percent == null);
    bar.firstElementChild.style.width = p.percent == null ? '' : `${p.percent}%`;
    const mb = (n) => (n / 1048576).toFixed(1);
    say(p.total ? `${p.label}  ${mb(p.done)} / ${mb(p.total)} MB` : p.label);
  });
  listen('status', (m) => { if (busy) say(m); });
  listen('error', (m) => {
    if (busy) {
      busy = null;
      say(m, true);
      load();
    }
  });
  // Kurulum bitince (paket kurulu görünür) listeyi yenile
  setInterval(async () => {
    if (!busy) return;
    const fresh = await invoke('packages_overview');
    const done = fresh.packages.find((p) => p.name === busy)?.installed;
    if (done) {
      say(TF('packages.installed_name', busy));
      busy = null;
      data = fresh;
      renderCatalog();
      renderVersions();
    }
  }, 2000);
});
