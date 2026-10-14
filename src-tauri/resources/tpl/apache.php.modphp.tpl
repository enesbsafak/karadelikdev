# Otomatik üretildi (mod_php). PHP sürümü değişince yeniden yazılır.
<<LOAD_FILES>>
LoadModule <<PHP_MODULE>> "<<PHP_DIR>>/<<PHP_DLL>>"
PHPIniDir "<<PHP_INI_DIR>>"
<FilesMatch "\.php$">
    SetHandler application/x-httpd-php
</FilesMatch>
