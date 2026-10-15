; KaradelikDev kurulum kancaları (Tauri NSIS şablonu).
; Program dosyaları kurulum klasöründe, veriler (bin, www, data...) ayrı bir klasörde (varsayılan C:\karadelik) durur;
; kaldırma verilere dokunmaz. Uygulama hangi exe'nin kurulu sürüm olduğunu InstallDir değerinden anlar.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\KaradelikDev" "InstallDir" "$INSTDIR"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Güncellemede (/UPDATE) değil, gerçek kaldırmada: servisleri durdur; otomatik başlatma, sağ tık menüsü,
  ; PATH girdileri ve Windows servisi kayıtlarını kaldır
  ${If} $UpdateMode <> 1
    ExecWait '"$INSTDIR\karadelikdev.exe" --uninstall'
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; DataRoot kalır: yeniden kurulumda aynı veri klasörü kullanılır
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\KaradelikDev" "InstallDir"
  ${EndIf}
!macroend
