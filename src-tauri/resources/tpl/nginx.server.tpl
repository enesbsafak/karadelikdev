server {
    listen <<PORT>>;
<<SSL_LISTEN>>
    server_name <<HOSTNAME>> *.<<HOSTNAME>>;
    root "<<PROJECT_DIR>>";
    index index.html index.htm index.php;

    location / {
        try_files $uri $uri/ /index.php$is_args$args;
        autoindex on;
    }

    location ~ \.php$ {
        include "<<KD_ROOT>>/etc/nginx/fastcgi-php.conf";
        fastcgi_pass php_upstream;
    }
<<SSL_CERT>>
    charset utf-8;
    location = /favicon.ico { access_log off; log_not_found off; }
    location = /robots.txt  { access_log off; log_not_found off; }
    location ~ /\.ht { deny all; }
}

# Bu dosya otomatik üretildi.
# KaradelikDev'in bu dosyaya dokunmamasını istiyorsanız adındaki [auto.] önekini kaldırın.
