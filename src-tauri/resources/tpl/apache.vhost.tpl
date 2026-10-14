<VirtualHost *:<<PORT>>>
    DocumentRoot "<<PROJECT_DIR>>"
    ServerName <<HOSTNAME>>
    ServerAlias *.<<HOSTNAME>>
    <Directory "<<PROJECT_DIR>>">
        AllowOverride All
        Require all granted
    </Directory>
</VirtualHost>

# Bu dosya otomatik üretildi.
# KaradelikDev'in bu dosyaya dokunmamasını istiyorsanız adındaki [auto.] önekini kaldırın.
# SSL için: Menü > Apache > SSL > Etkin
