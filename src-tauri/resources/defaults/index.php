<?php
// KaradelikDev başlangıç sayfası. phpinfo yalnızca bu bilgisayardan, localhost adresiyle görüntülenebilir
// (tünel veya DNS rebinding ile gelen istekler de 127.0.0.1'den gelir; Host başlığı ayırt eder).
$isLocal = in_array($_SERVER['REMOTE_ADDR'] ?? '', ['127.0.0.1', '::1'], true)
    && preg_match('/^(localhost|127\.0\.0\.1|\[::1\])(:\d+)?$/i', $_SERVER['HTTP_HOST'] ?? '');
if (isset($_GET['q'])) {
    if ($_GET['q'] === 'info' && $isLocal) {
        phpinfo();
        exit;
    }
    http_response_code($_GET['q'] === 'info' ? 403 : 404);
    exit('Yetkisiz veya geçersiz istek.');
}
$projects = array_values(array_filter(scandir(__DIR__), function ($d) {
    return $d[0] !== '.' && is_dir(__DIR__ . '/' . $d) && preg_match('/^[A-Za-z0-9][A-Za-z0-9.-]*$/', $d);
}));
$server = $_SERVER['SERVER_SOFTWARE'] ?? 'PHP';
?>
<!doctype html>
<html lang="tr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>KaradelikDev</title>
<style>
  :root { color-scheme: dark; }
  body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: radial-gradient(circle at 50% 35%, #1c1208 0, #0b0b10 55%); color: #e9e7ef; font: 15px/1.5 "Segoe UI", system-ui, sans-serif; }
  main { width: min(640px, calc(100% - 32px)); text-align: center; }
  .hole { width: 120px; height: 120px; margin: 0 auto 18px; border-radius: 50%; background: #06060c; box-shadow: 0 0 0 3px #ffdca0, 0 0 30px 10px #ff8a1f, 0 0 80px 30px rgba(255, 95, 31, .35); }
  h1 { margin: 0; font-size: 34px; letter-spacing: .5px; }
  p.meta { color: #8d8a9c; margin: 6px 0 26px; }
  ul { list-style: none; padding: 0; display: flex; flex-wrap: wrap; gap: 8px; justify-content: center; }
  a { color: #ffb347; text-decoration: none; }
  li a { display: inline-block; padding: 6px 14px; border: 1px solid #262633; border-radius: 999px; background: #12121a; }
  li a:hover { border-color: #ff8a1f; }
  footer { margin-top: 28px; font-size: 13px; color: #8d8a9c; }
</style>
</head>
<body>
<main>
  <div class="hole"></div>
  <h1>KaradelikDev</h1>
  <p class="meta"><?= htmlspecialchars($server) ?> · PHP <?= PHP_VERSION ?></p>
  <?php if ($projects): ?>
  <ul>
    <?php foreach ($projects as $p): ?>
      <li><a href="http://<?= htmlspecialchars(strtolower($p)) ?>.test"><?= htmlspecialchars($p) ?></a></li>
    <?php endforeach; ?>
  </ul>
  <?php else: ?>
  <p>Document Root'a bir klasör koyun; <code>klasor.test</code> adresi otomatik oluşur.</p>
  <?php endif; ?>
  <footer><?php if ($isLocal): ?><a href="?q=info">phpinfo()</a><?php endif; ?></footer>
</main>
</body>
</html>
