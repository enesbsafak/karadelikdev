// Loglar: logs klasöründeki dosyalar, son kısmı canlı izleme, süzme, temizleme.
const $ = (id) => document.getElementById(id);
let files = [];
let current = null;
let lastText = '';
let root = '';

const fmtSize = (n) => {
  if (!n) return '0 B';
  const u = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`;
};

function renderList() {
  const box = $('list');
  box.textContent = '';
  for (const f of files) {
    box.append(el('div', {
      class: `log-item${f.name === current ? ' active' : ''}`,
      title: new Date(f.modified * 1000).toLocaleString(),
      onclick: () => select(f.name),
    }, el('span', { class: 'ellipsis' }, f.name), el('span', { class: 'muted' }, fmtSize(f.size))));
  }
  if (!files.length) box.append(el('div', { class: 'empty-hint', style: 'padding:12px' }, T('logs.none')));
}

function show(text) {
  const q = $('find').value.trim().toLowerCase();
  const pre = $('text');
  const atBottom = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 30;
  pre.textContent = q ? text.split('\n').filter((l) => l.toLowerCase().includes(q)).join('\n') : text;
  if ($('follow').checked && atBottom) pre.scrollTop = pre.scrollHeight;
}

async function refreshText(force = false) {
  if (!current) return;
  try {
    const text = await invoke('log_tail', { name: current });
    if (force || text !== lastText) {
      lastText = text;
      show(text);
    }
    $('info').textContent = `${root}\\${current}`;
  } catch (e) {
    $('info').textContent = String(e);
  }
}

async function select(name) {
  current = name;
  lastText = '';
  $('current').textContent = name;
  renderList();
  await refreshText(true);
  const pre = $('text');
  pre.scrollTop = pre.scrollHeight;
}

async function loadList() {
  files = await invoke('logs_list');
  if (!current && files.length) {
    const prefer = files.find((f) => f.name === 'karadelik.log') || files[0];
    await select(prefer.name);
  } else {
    renderList();
  }
}

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  document.title = T('logs.title');
  root = (await invoke('get_state')).root + '\\logs';
  await loadList();
  $('find').addEventListener('input', () => show(lastText));
  $('edit').addEventListener('click', () => current && invoke('run_action', { id: `edit:${root}\\${current}` }));
  let armed = false;
  $('clear').addEventListener('click', async () => {
    if (!current) return;
    if (!armed) {
      armed = true;
      $('clear').textContent = T('db.sure');
      setTimeout(() => { armed = false; $('clear').textContent = T('logs.clear'); }, 3000);
      return;
    }
    armed = false;
    $('clear').textContent = T('logs.clear');
    try {
      await invoke('log_clear', { name: current });
    } catch (e) {
      $('info').textContent = String(e);
    }
    await refreshText(true);
    await loadList();
  });
  setInterval(() => { if ($('follow').checked) refreshText(); }, 1500);
  setInterval(loadList, 5000);
});
