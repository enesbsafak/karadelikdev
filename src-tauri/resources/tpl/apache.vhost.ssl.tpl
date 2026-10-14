<VirtualHost *:<<PORT>>>
    DocumentRoot "<<PROJECT_DIR>>"
    ServerName <<HOSTNAME>>
    ServerAlias *.<<HOSTNAME>>
    <Directory "<<PROJECT_DIR>>">
        AllowOverride All
        Require all granted
    </Directory>
</VirtualHost>

<VirtualHost *:<<SSL_PORT>>>
    DocumentRoot "<<PROJECT_DIR>>"
    ServerName <<HOSTNAME>>
    ServerAlias *.<<HOSTNAME>>
    <Directory "<<PROJECT_DIR>>">
        AllowOverride All
        Require all granted
    </Directory>
    SSLEngine on
    SSLCertificateFile      "<<SSL_DIR>>/<<HOSTNAME>>.crt"
    SSLCertificateKeyFile   "<<SSL_DIR>>/<<HOSTNAME>>.key"
</VirtualHost>

# Bu dosya otomatik üretildi.
# KaradelikDev'in bu dosyaya dokunmamasını istiyorsanız adındaki [auto.] önekini kaldırın.
