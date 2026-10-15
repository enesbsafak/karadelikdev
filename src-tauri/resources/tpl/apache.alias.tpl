# Otomatik üretildi. Yalnızca bu bilgisayardan ve localhost adresiyle açılır (ör. http://localhost/<<ALIAS>>/);
# başka bir alan adı (tünel, DNS rebinding) reddedilir. Elle değiştirmek için dosya adındaki [auto.] önekini kaldırın.
Alias /<<ALIAS>> "<<APP_DIR>>/"
<Directory "<<APP_DIR>>/">
    Options Indexes FollowSymLinks MultiViews
    AllowOverride all
    <RequireAll>
        Require local
        Require expr "%{HTTP_HOST} =~ /^(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?$/i"
    </RequireAll>
</Directory>
