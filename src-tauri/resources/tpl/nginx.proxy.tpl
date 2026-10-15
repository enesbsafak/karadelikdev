server {
<<LISTEN>>
<<SSL_LISTEN>>
    server_name <<HOSTNAME>> *.<<HOSTNAME>>;

    # Geliştirme sunucusuna (Vite, Next.js, Node...) ters vekil; WebSocket (HMR) dahil
    location / {
        proxy_pass http://127.0.0.1:<<PROXY_PORT>>;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $http_connection;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 3600;
    }
<<SSL_CERT>>
}

# Bu dosya otomatik üretildi (Projeler > Proxy portu).
# KaradelikDev'in bu dosyaya dokunmamasını istiyorsanız adındaki [auto.] önekini kaldırın.
