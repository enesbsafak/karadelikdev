# Otomatik üretildi (php-cgi havuzu, mod_proxy_fcgi). PHP sürümü değişince yeniden yazılır.
<Proxy "balancer://kdphp">
<<BALANCER_MEMBERS>>
</Proxy>
ProxyFCGIBackendType GENERIC
ProxyTimeout 36000
<FilesMatch "\.php$">
    SetHandler "proxy:balancer://kdphp/"
<<SCRIPT_FIX>></FilesMatch>
