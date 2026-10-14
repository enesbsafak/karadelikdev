# Otomatik üretildi. Dışarıdan erişim için "Require local" satırını "Require all granted" yapın
# ve dosya adındaki [auto.] önekini kaldırın.
Alias /<<ALIAS>> "<<APP_DIR>>/"
<Directory "<<APP_DIR>>/">
    Options Indexes FollowSymLinks MultiViews
    AllowOverride all
    Require local
</Directory>
