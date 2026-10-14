# KaradelikDev tarafından üretildi. Kalıcı değişiklik için: usr\tpl\my.ini.tpl
[client]
port=<<PORT>>

[mysqld]
port=<<PORT>>
bind-address=127.0.0.1
basedir="<<BASEDIR>>"
datadir="<<DATADIR>>"
log-error="<<LOG_DIR>>/mysqld.log"
key_buffer_size=256M
max_allowed_packet=512M
table_open_cache=256
sort_buffer_size=1M
read_buffer_size=1M
read_rnd_buffer_size=4M
myisam_sort_buffer_size=64M
thread_cache_size=8
secure-file-priv=""
explicit_defaults_for_timestamp=1
character-set-server=utf8mb4

[mysqldump]
quick
max_allowed_packet=512M
