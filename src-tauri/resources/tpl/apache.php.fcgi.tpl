# Otomatik üretildi (php-cgi havuzu, mod_proxy_fcgi). PHP sürümü değişince yeniden yazılır.
<Proxy "balancer://kdphp">
<<BALANCER_MEMBERS>>
</Proxy>
ProxyFCGIBackendType GENERIC
ProxyTimeout 36000
<FilesMatch "\.php$">
    SetHandler "proxy:balancer://kdphp/"
    # Windows yolu (C:/...) için: mod_proxy_fcgi betik yolunu "/C:/..." veya "proxy:balancer://..." önekiyle gönderir
    ProxyFCGISetEnvIf "reqenv('SCRIPT_FILENAME') =~ m#^(?:proxy:balancer://[^/]+)?/([A-Za-z]:/.*)$#" SCRIPT_FILENAME "$1"
</FilesMatch>
