# KaradelikDev tarafından üretildi. Kalıcı değişiklik için: usr\tpl\nginx.conf.tpl
worker_processes  1;
error_log  "<<LOG_DIR>>/nginx_error.log";
pid        "<<LOG_DIR>>/nginx.pid";

events {
    worker_connections  1024;
}

http {
    include       "<<NGINX_CONF>>/mime.types";
    default_type  application/octet-stream;
    access_log    "<<LOG_DIR>>/nginx_access.log";

    sendfile           on;
    keepalive_timeout  65;
    client_max_body_size 2000M;
    server_names_hash_bucket_size 128;
    fastcgi_read_timeout 36000;

    include "<<KD_ROOT>>/etc/nginx/php_upstream.conf";
    include "<<KD_ROOT>>/etc/nginx/sites-enabled/*.conf";
}
