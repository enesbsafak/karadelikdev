// İlk kurulum sihirbazı: VC++ çalışma zamanı, paketler, PATH, sağ tık menüsü ve başlangıç seçenekleri.
const $ = (id) => document.getElementById(id);
const OPTS = ['auto_vhosts', 'run_at_startup', 'context_menu', 'add_path', 'import_laragon', 'install_vc'];
let plan;

function updateSummary() {
  const n = document.querySelectorAll('[data-pkg]:checked').length;
  $('summary').textContent = TF('setup.selected', String(n));
}

function render(data) {
  plan = data.plan;
  const d = data.defaults;
  $('root').textContent = plan.root;
  $('rootBox').classList.toggle('hidden', !plan.installed);
  $('dataRoot').value = plan.root;
  $('spaceWarn').classList.toggle('hidden', !plan.root_has_space);
  if (plan.busy_ports.length) {
    $('busyText').textContent = `${T('setup.busy_ports')} ${plan.busy_ports.join(' · ')}`;
    $('busyWarn').classList.remove('hidden');
  }
  for (const k of OPTS) $(k).checked = !!d[k];

  if (plan.laragon) {
    $('laragonRow').classList.remove('hidden');
    $('laragonText').textContent = TF('setup.import_laragon', plan.laragon);
  }
  $('vcText').textContent = plan.vc_ok
    ? TF('setup.vc_ok', plan.vc_version || '')
    : T('setup.vc_missing');
  $('install_vc').disabled = plan.vc_ok;
  if (plan.vc_ok) $('install_vc').checked = true;

  const box = $('packages');
  box.textContent = '';
  let group = null;
  for (const p of plan.packages) {
    if (p.group !== group) {
      group = p.group;
      box.append(el('div', { class: 'pkg-group' }, group));
    }
    const input = el('input', { type: 'checkbox', 'data-pkg': p.name, checked: d.packages.includes(p.name), disabled: p.installed });
    input.addEventListener('change', updateSummary);
    box.append(el('label', { class: 'check pkg' },
      input,
      el('span', {}, p.name),
      p.recommended ? el('span', { class: 'tag' }, T('setup.recommended')) : null,
      p.installed ? el('span', { class: 'tag ok' }, T('setup.installed')) : null));
  }
  // Laragon içe aktarılacaksa paketler gereksiz; seçim kullanıcının
  $('import_laragon').addEventListener('change', (e) => {
    if (e.target.checked) document.querySelectorAll('[data-pkg]').forEach((i) => { i.checked = false; });
    updateSummary();
  });
  updateSummary();
}

function collect() {
  const o = {};
  for (const k of OPTS) o[k] = $(k).checked;
  o.packages = [...document.querySelectorAll('[data-pkg]:checked')].map((i) => i.dataset.pkg);
  return o;
}

function log(line) {
  const l = $('log');
  l.textContent += `${line}\n`;
  l.scrollTop = l.scrollHeight;
  $('current').textContent = line;
}

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  document.title = T('setup.title');
  render(await invoke('setup_plan'));

  $('skip').addEventListener('click', () => invoke('setup_finish'));
  $('pickRoot').addEventListener('click', async () => {
    const p = await invoke('pick_folder', { current: $('dataRoot').value });
    if (!p || p === $('dataRoot').value) return;
    $('rootErr').classList.add('hidden');
    try {
      // Başarılıysa uygulama yeni klasörle yeniden açılır
      await invoke('setup_set_root', { path: p });
    } catch (e) {
      $('rootErr').textContent = String(e);
      $('rootErr').classList.remove('hidden');
    }
  });
  $('finish').addEventListener('click', () => invoke('setup_finish'));
  $('install').addEventListener('click', () => {
    $('form').classList.add('hidden');
    $('progressView').classList.remove('hidden');
    $('install').classList.add('hidden');
    $('skip').classList.add('hidden');
    invoke('setup_run', { options: collect() });
  });

  listen('setup-log', log);
  listen('progress', (p) => {
    const bar = $('bar');
    if (p.finished) { bar.classList.add('indeterminate'); bar.firstElementChild.style.width = ''; return; }
    bar.classList.toggle('indeterminate', p.percent == null);
    bar.firstElementChild.style.width = p.percent == null ? '' : `${p.percent}%`;
    const mb = (n) => (n / 1048576).toFixed(1);
    $('current').textContent = p.total ? `${p.label}  ${mb(p.done)} / ${mb(p.total)} MB` : p.label;
  });
  listen('setup-done', (errors) => {
    const bar = $('bar');
    bar.classList.remove('indeterminate');
    bar.firstElementChild.style.width = '100%';
    $('summary').textContent = errors.length ? T('setup.done_with_errors') : T('setup.done');
    $('finish').classList.remove('hidden');
  });
});
