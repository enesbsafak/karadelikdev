// Genel diyalog: input | confirm | info. İçerik Rust tarafında pencere etiketine göre tutulur.
const $ = (id) => document.getElementById(id);

document.addEventListener('DOMContentLoaded', async () => {
  await loadI18n();
  const spec = await invoke('dialog_get');
  if (!spec) { currentWindow().close(); return; }

  $('title').textContent = spec.title;
  document.title = spec.title;
  $('message').textContent = spec.message || '';
  $('message').classList.toggle('hidden', !spec.message);

  if (spec.text) {
    $('text').textContent = spec.text;
    $('text').classList.remove('hidden');
  }
  if (spec.url) {
    $('url').value = spec.url;
    $('urlRow').classList.remove('hidden');
    $('copyUrl').addEventListener('click', () => {
      invoke('copy_text', { text: spec.url });
      $('copyUrl').textContent = T('ui.copied');
    });
    $('openUrl').addEventListener('click', () => invoke('open_url', { url: spec.url }));
  }
  if (spec.svg) {
    // SVG, Rust tarafında qrcode crate'iyle üretilir (kullanıcı girdisi değil)
    $('qr').innerHTML = spec.svg;
    $('qr').classList.remove('hidden');
  }

  const form = $('form');
  for (const f of spec.fields || []) {
    form.append(el('div', { class: 'row' },
      el('label', { class: 'lbl', for: `f-${f.name}`, style: 'width:120px' }, f.label),
      el('input', { class: 'field grow', id: `f-${f.name}`, name: f.name, type: f.kind === 'password' ? 'password' : 'text', value: f.value || '', autocomplete: 'off', spellcheck: 'false' })));
  }
  const first = form.querySelector('input');
  if (first) { first.focus(); first.select(); }

  const ok = $('ok');
  const cancel = $('cancel');
  if (spec.kind === 'info') {
    cancel.classList.add('hidden');
    ok.textContent = T('btn.close');
  } else if (spec.danger) {
    ok.classList.remove('primary');
    ok.classList.add('danger');
    ok.textContent = T('btn.delete');
  }
  if (spec.ok) ok.textContent = spec.ok;

  const submit = async () => {
    if (spec.kind === 'info' && !spec.action) { invoke('dialog_close'); return; }
    const values = [...form.querySelectorAll('input')].map((i) => i.value);
    ok.disabled = true;
    try {
      await invoke('dialog_submit', { values });
    } catch (e) {
      $('error').textContent = e;
      $('error').classList.remove('hidden');
      ok.disabled = false;
    }
  };
  ok.addEventListener('click', submit);
  form.addEventListener('submit', (e) => { e.preventDefault(); submit(); });
  cancel.addEventListener('click', () => invoke('dialog_close'));
  document.addEventListener('keydown', (e) => { if (e.key === 'Enter' && spec.kind !== 'input') submit(); });
});
