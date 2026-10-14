# Otomatik üretildi. Yalnızca localhost erişebilir.
location ^~ /<<ALIAS>>/ {
    alias "<<APP_DIR>>/";
    index index.php index.html;
    allow 127.0.0.1;
    allow ::1;
    deny all;
    location ~ \.php$ {
        fastcgi_split_path_info ^(.+?\.php)(/.*)$;
        include "<<NGINX_CONF>>/fastcgi.conf";
        fastcgi_param SCRIPT_FILENAME $request_filename;
        fastcgi_pass php_upstream;
    }
}
location = /<<ALIAS>> { return 301 /<<ALIAS>>/; }
