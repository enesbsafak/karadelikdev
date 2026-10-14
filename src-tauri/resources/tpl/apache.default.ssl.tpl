<VirtualHost _default_:<<PORT>>>
    DocumentRoot "<<DOC_ROOT>>"
    <Directory "<<DOC_ROOT>>">
        AllowOverride All
        Require all granted
    </Directory>
</VirtualHost>

<VirtualHost _default_:<<SSL_PORT>>>
    DocumentRoot "<<DOC_ROOT>>"
    <Directory "<<DOC_ROOT>>">
        AllowOverride All
        Require all granted
    </Directory>
    SSLEngine on
    SSLCertificateFile      "<<SSL_DIR>>/localhost.crt"
    SSLCertificateKeyFile   "<<SSL_DIR>>/localhost.key"
</VirtualHost>
