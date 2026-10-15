<VirtualHost *:<<PORT>>>
    ServerName <<HOSTNAME>>
    ServerAlias *.<<HOSTNAME>>
    # Geliştirme sunucusuna (Vite, Next.js, Node...) ters vekil; WebSocket (HMR) dahil
    ProxyPass / http://127.0.0.1:<<PROXY_PORT>>/ upgrade=websocket
    ProxyPassReverse / http://127.0.0.1:<<PROXY_PORT>>/
    RequestHeader set X-Forwarded-Proto "http"
</VirtualHost>

<VirtualHost *:<<SSL_PORT>>>
    ServerName <<HOSTNAME>>
    ServerAlias *.<<HOSTNAME>>
    ProxyPass / http://127.0.0.1:<<PROXY_PORT>>/ upgrade=websocket
    ProxyPassReverse / http://127.0.0.1:<<PROXY_PORT>>/
    RequestHeader set X-Forwarded-Proto "https"
    SSLEngine on
    SSLCertificateFile      "<<SSL_DIR>>/<<HOSTNAME>>.crt"
    SSLCertificateKeyFile   "<<SSL_DIR>>/<<HOSTNAME>>.key"
</VirtualHost>

# Bu dosya otomatik üretildi (Projeler > Proxy portu).
# KaradelikDev'in bu dosyaya dokunmamasını istiyorsanız adındaki [auto.] önekini kaldırın.
