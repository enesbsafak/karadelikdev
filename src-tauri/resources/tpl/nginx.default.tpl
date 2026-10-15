server {
<<LISTEN>>
<<SSL_LISTEN>>
    server_name localhost;
    root "<<DOC_ROOT>>";
    index index.html index.htm index.php;

    include "<<KD_ROOT>>/etc/nginx/alias/*.conf";

    location / {
        try_files $uri $uri/ =404;
        autoindex on;
    }

    # .env, .git gibi gizli dosya ve klasörler sunulmaz (/.well-known hariç)
    location ~ /\.(?!well-known) { deny all; }

    location ~ \.php$ {
        include "<<KD_ROOT>>/etc/nginx/fastcgi-php.conf";
        fastcgi_pass php_upstream;
    }
<<SSL_CERT>>
    charset utf-8;
    location = /favicon.ico { access_log off; log_not_found off; }
    location = /robots.txt  { access_log off; log_not_found off; }
}
