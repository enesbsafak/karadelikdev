# Otomatik üretildi. Apache > SSL menüsünden açılıp kapatılır.
LoadModule ssl_module modules/mod_ssl.so
Listen <<SSL_PORT>>
SSLCipherSuite HIGH:!aNULL:!MD5:!RC4:!3DES:!CAMELLIA
SSLProxyCipherSuite HIGH:!aNULL:!MD5:!RC4:!3DES:!CAMELLIA
SSLHonorCipherOrder on
SSLProtocol -all +TLSv1.2 +TLSv1.3
SSLProxyProtocol -all +TLSv1.2 +TLSv1.3
SSLSessionCache "shmcb:<<LOG_DIR>>/ssl_scache(512000)"
SSLSessionCacheTimeout 300
