# KaradelikDev tarafından üretildi. Kalıcı değişiklik için: usr\tpl\httpd.conf.tpl
Define KD_ROOT "<<KD_ROOT>>"
ServerRoot "<<SERVER_ROOT>>"
Listen <<PORT>>

LoadModule access_compat_module modules/mod_access_compat.so
LoadModule actions_module modules/mod_actions.so
LoadModule alias_module modules/mod_alias.so
LoadModule allowmethods_module modules/mod_allowmethods.so
LoadModule auth_basic_module modules/mod_auth_basic.so
LoadModule authn_core_module modules/mod_authn_core.so
LoadModule authn_file_module modules/mod_authn_file.so
LoadModule authz_core_module modules/mod_authz_core.so
LoadModule authz_groupfile_module modules/mod_authz_groupfile.so
LoadModule authz_host_module modules/mod_authz_host.so
LoadModule authz_user_module modules/mod_authz_user.so
LoadModule autoindex_module modules/mod_autoindex.so
LoadModule deflate_module modules/mod_deflate.so
LoadModule dir_module modules/mod_dir.so
LoadModule env_module modules/mod_env.so
LoadModule expires_module modules/mod_expires.so
LoadModule filter_module modules/mod_filter.so
LoadModule headers_module modules/mod_headers.so
LoadModule include_module modules/mod_include.so
LoadModule log_config_module modules/mod_log_config.so
LoadModule mime_module modules/mod_mime.so
LoadModule negotiation_module modules/mod_negotiation.so
LoadModule proxy_module modules/mod_proxy.so
LoadModule proxy_balancer_module modules/mod_proxy_balancer.so
LoadModule proxy_fcgi_module modules/mod_proxy_fcgi.so
LoadModule proxy_http_module modules/mod_proxy_http.so
LoadModule proxy_wstunnel_module modules/mod_proxy_wstunnel.so
LoadModule lbmethod_byrequests_module modules/mod_lbmethod_byrequests.so
LoadModule slotmem_shm_module modules/mod_slotmem_shm.so
LoadModule rewrite_module modules/mod_rewrite.so
LoadModule setenvif_module modules/mod_setenvif.so
LoadModule socache_shmcb_module modules/mod_socache_shmcb.so
LoadModule version_module modules/mod_version.so

ServerName localhost
ServerAdmin admin@localhost
ServerTokens Prod
Timeout 300

DocumentRoot "<<DOC_ROOT>>"
<Directory />
    AllowOverride none
    Require all denied
</Directory>
<Directory "<<DOC_ROOT>>">
    Options Indexes FollowSymLinks
    AllowOverride All
    Require all granted
</Directory>

DirectoryIndex index.php index.html index.htm
<Files ".ht*">
    Require all denied
</Files>

ErrorLog "<<LOG_DIR>>/apache_error.log"
LogLevel warn
LogFormat "%h %l %u %t \"%r\" %>s %b \"%{Referer}i\" \"%{User-Agent}i\"" combined
CustomLog "<<LOG_DIR>>/apache_access.log" combined
PidFile "<<LOG_DIR>>/httpd.pid"

TypesConfig conf/mime.types
AddType application/x-compress .Z
AddType application/x-gzip .gz .tgz

AcceptFilter http none
AcceptFilter https none
EnableSendfile Off
EnableMMAP Off

<IfModule mpm_winnt_module>
    ThreadStackSize 8388608
    ThreadsPerChild 150
    MaxConnectionsPerChild 0
</IfModule>

IncludeOptional "<<KD_ROOT>>/etc/apache2/php.conf"
IncludeOptional "<<KD_ROOT>>/etc/apache2/ssl.conf"
IncludeOptional "<<KD_ROOT>>/etc/apache2/alias/*.conf"
IncludeOptional "<<KD_ROOT>>/etc/apache2/sites-enabled/*.conf"
