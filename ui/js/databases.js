// Veritabanı yöneticisi: bağlantı bilgileri, veritabanları, kullanıcılar, yedekler.
const $ = (id) => document.getElementById(id);
let data = null;
let showPw = false;

const fmtSize = (n) => {
  if (!n) return '0 B';
  const u = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`;
};
const fmtDate = (s) => new Date(s * 1000).toLocaleString();

function say(text, isError = false) {
  const m = $('msg');
  m.textContent = text || '';
  m.style.color = isError ? 'var(--err)' : '';
}

/** Silme gibi geri alınamaz işlemlerde iki tıklamalı onay. */
function confirmButton(label, onConfirm) {
  const b = el('button', { class: 'btn danger' }, label);
  let armed = false;
  let timer;
  b.addEventListener('click', () => {
    if (!armed) {
      armed = true;
      b.textContent = T('db.sure');
      timer = setTimeout(() => { armed = false; b.textContent = label; }, 3000);
      return;
    }
    clearTimeout(timer);
    onConfirm();
  });
  return b;
}

async function call(op, ...args) {
  say(T('db.working'));
  document.body.classList.add('busy');
  try {
    const r = await invoke('db_call', { op, args });
    say(r);
    await load();
    return true;
  } catch (e) {
    say(String(e), true);
    return false;
  } finally {
    document.body.classList.remove('busy');
  }
}

function pwText() {
  if (!data.password) return T('db.empty_password');
  return showPw ? data.password : '•'.repeat(Math.min(12, data.password.length));
}

function randomPassword() {
  const ch = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789';
  const a = new Uint32Array(16);
  crypto.getRandomValues(a);
  return [...a].map((x) => ch[x % ch.length]).join('');
}

function render() {
  $('dbDot').parentElement.classList.toggle('running', data.running);
  $('dbVersion').textContent = data.version || 'MySQL';
  $('notRunning').classList.toggle('hidden', data.running || !data.installed);
  $('host').textContent = data.host;
  $('port').textContent = data.port;
  $('user').textContent = data.user;
  $('password').textContent = pwText();
  $('password').classList.toggle('muted', !data.password);
  $('btnShow').textContent = showPw ? T('db.hide') : T('db.show');
  $('btnShow').classList.toggle('hidden', !data.password);
  $('btnRemovePw').classList.toggle('hidden', !data.password);
  $('btnPma').classList.toggle('hidden', !data.phpmyadmin);
  $('btnAdminer').classList.toggle('hidden', !data.adminer);
  $('btnHeidi').classList.toggle('hidden', !data.heidisql);

  // Veritabanları
  const dbs = data.databases.Ok || [];
  const tb = $('dbRows');
  tb.textContent = '';
  if (data.databases.Err) tb.append(el('tr', {}, el('td', { colspan: 4, class: 'err' }, data.databases.Err)));
  if (!data.running) tb.append(el('tr', {}, el('td', { colspan: 4, class: 'muted' }, T('db.not_running'))));
  else if (!dbs.length) tb.append(el('tr', {}, el('td', { colspan: 4, class: 'muted' }, T('db.no_databases'))));
  for (const d of dbs) {
    const actions = el('td', { class: 'actions' });
    actions.append(el('button', { class: 'btn', onclick: () => call('backup_db', d.name) }, T('db.backup')));
    if (data.phpmyadmin) {
      actions.append(el('button', {
        class: 'btn',
        onclick: () => invoke('open_url', { url: `${data.phpmyadmin}index.php?route=/database/structure&db=${encodeURIComponent(d.name)}` }),
      }, T('btn.open')));
    }
    actions.append(confirmButton(T('btn.delete'), () => call('drop_db', d.name)));
    tb.append(el('tr', {}, el('td', {}, d.name), el('td', { class: 'num' }, String(d.tables)), el('td', { class: 'num' }, fmtSize(d.size)), actions));
  }

  // Kullanıcılar
  const sel = $('uDb');
  const cur = sel.value;
  sel.textContent = '';
  sel.append(el('option', { value: '*' }, T('db.all_databases')));
  for (const d of dbs) sel.append(el('option', { value: d.name }, d.name));
  if (cur) sel.value = cur;
  const ub = $('userRows');
  ub.textContent = '';
  if (data.users.Err) ub.append(el('tr', {}, el('td', { colspan: 4, class: 'err' }, data.users.Err)));
  for (const u of data.users.Ok || []) {
    const scope = u.user === 'root' ? T('db.all_databases') : (u.databases.join(', ') || T('db.global_or_none'));
    ub.append(el('tr', {},
      el('td', {}, `${u.user}@${u.host}`),
      el('td', { class: 'muted' }, u.plugin),
      el('td', {}, scope),
      el('td', { class: 'actions' }, u.user === 'root' ? null : confirmButton(T('btn.delete'), () => call('drop_user', u.user, u.host)))));
  }

  // Yedekler
  $('backupDir').textContent = data.backup_dir;
  const bb = $('backupRows');
  bb.textContent = '';
  if (!data.backups.length) bb.append(el('tr', {}, el('td', { colspan: 5, class: 'muted' }, T('db.no_backups'))));
  for (const b of data.backups) {
    const actions = el('td', { class: 'actions' },
      confirmButton(T('db.restore'), () => call('restore', b.path, b.database)),
      confirmButton(T('btn.delete'), () => call('delete_backup', b.path)));
    actions.firstChild.classList.remove('danger');
    bb.append(el('tr', {},
      el('td', { class: 'ellipsis', title: b.path }, b.file),
      el('td', {}, b.database || T('db.all_databases')),
      el('td', { class: 'num' }, fmtSize(b.size)),
      el('td', { class: 'muted' }, fmtDate(b.modified)),
      actions));
  }
}

async function load() {
  data = await invoke('db_overview');
  render();
}

window.onI18n = () => data && render();

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  document.title = T('db.title');
  await load();

  document.querySelectorAll('.tab').forEach((t) => t.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach((x) => x.classList.toggle('active', x === t));
    document.querySelectorAll('.content').forEach((c) => c.classList.toggle('hidden', c.id !== `tab-${t.dataset.tab}`));
  }));
  document.querySelectorAll('[data-copy]').forEach((b) => b.addEventListener('click', () => {
    invoke('copy_text', { text: String(data[b.dataset.copy] ?? '') });
    say(T('ui.copied'));
  }));

  $('btnShow').addEventListener('click', () => { showPw = !showPw; render(); });
  $('btnChange').addEventListener('click', () => { $('pwForm').classList.remove('hidden'); $('newPw').focus(); });
  $('cancelPw').addEventListener('click', () => $('pwForm').classList.add('hidden'));
  $('genPw').addEventListener('click', () => { $('newPw').value = randomPassword(); });
  $('pwForm').addEventListener('submit', async (e) => {
    e.preventDefault();
    if (await call('change_root', $('newPw').value)) {
      $('pwForm').classList.add('hidden');
      $('newPw').value = '';
    }
  });
  $('btnReset').replaceWith(confirmButton(T('db.reset'), () => call('reset_root')));
  $('btnRemovePw').addEventListener('click', () => call('change_root', ''));
  $('btnStart').addEventListener('click', () => call('start'));
  $('btnPma').addEventListener('click', () => invoke('open_url', { url: data.phpmyadmin }));
  $('btnAdminer').addEventListener('click', () => invoke('open_url', { url: data.adminer }));
  $('btnHeidi').addEventListener('click', () => invoke('run_action', { id: 'tool:heidisql' }));

  $('createDb').addEventListener('submit', async (e) => {
    e.preventDefault();
    if (await call('create_db', $('newDb').value.trim())) $('newDb').value = '';
  });
  $('backupAll').addEventListener('click', () => call('backup_all'));
  let restorePath = '';
  $('restoreFile').addEventListener('click', async () => {
    const p = await invoke('pick_sql_file');
    if (!p) return;
    restorePath = p;
    $('restorePath').textContent = p.split(/[\\/]/).pop();
    const m = p.split(/[\\/]/).pop().match(/^db-(.+)-\d{4}-\d{2}-\d{2}_\d{6}\.sql$/);
    $('restoreDb').value = m ? m[1] : '';
    $('restoreForm').classList.remove('hidden');
  });
  $('restoreCancel').addEventListener('click', () => $('restoreForm').classList.add('hidden'));
  $('restoreForm').addEventListener('submit', async (e) => {
    e.preventDefault();
    if (await call('restore', restorePath, $('restoreDb').value.trim())) $('restoreForm').classList.add('hidden');
  });

  $('uGen').addEventListener('click', () => { $('uPass').value = randomPassword(); });
  $('createUser').addEventListener('submit', async (e) => {
    e.preventDefault();
    if (await call('create_user', $('uName').value.trim(), $('uPass').value, $('uDb').value)) {
      say(`${T('db.user_created_hint')} ${$('uName').value.trim()} / ${$('uPass').value}`);
      $('uName').value = '';
      $('uPass').value = '';
    }
  });
  $('openBackupDir').addEventListener('click', () => invoke('run_action', { id: `open:${data.backup_dir}` }));
  $('refresh').addEventListener('click', () => load().then(() => say('')));
});
