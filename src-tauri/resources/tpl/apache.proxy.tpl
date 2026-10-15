<VirtualHost *:<<PORT>>>
    ServerName <<HOSTNAME>>
    ServerAlias *.<<HOSTNAME>>
    # Geliştirme sunucusuna (Vite, Next.js, Node...) ters vekil; WebSocket (HMR) dahil
    ProxyPass / http://127.0.0.1:<<PROXY_PORT>>/ upgrade=websocket
    ProxyPassReverse / http://127.0.0.1:<<PROXY_PORT>>/
    RequestHeader set X-Forwarded-Proto "http"
</VirtualHost>

# Bu dosya otomatik üretildi (Projeler > Proxy portu).
# KaradelikDev'in bu dosyaya dokunmamasını istiyorsanız adındaki [auto.] önekini kaldırın.
