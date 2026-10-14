<VirtualHost _default_:<<PORT>>>
    DocumentRoot "<<DOC_ROOT>>"
    <Directory "<<DOC_ROOT>>">
        AllowOverride All
        Require all granted
    </Directory>
</VirtualHost>
