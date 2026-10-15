; KaradelikDev kurulum kancaları (Tauri NSIS şablonu, tüm kullanıcılar için kurulum: Program Files).
; Program dosyaları kurulum klasöründe, veriler (bin, www, data...) kullanıcının veri klasöründe (varsayılan C:\karadelik)
; durur; kaldırma verilere dokunmaz. Uygulama hangi exe'nin kurulu sürüm olduğunu InstallDir değerinden anlar.

!macro NSIS_HOOK_PREINSTALL
  ; 1.2.0 kullanıcı başına (%LOCALAPPDATA%\KaradelikDev) kuruluyordu: o kurulumu kaldır, kısayollarını sil.
  ; /UPDATE: eski kaldırıcı servisleri durdurup PATH, otomatik başlatma ve sağ tık menüsünü silmesin.
  StrCpy $R9 "0"
  ReadRegStr $R8 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KaradelikDev" "InstallLocation"
  ${If} $R8 != ""
    StrCpy $R9 "1"
    StrCpy $R8 $R8 -1 1
    ${If} ${FileExists} "$R8\uninstall.exe"
      ExecWait '"$R8\uninstall.exe" /S /UPDATE _?=$R8'
      Delete "$R8\uninstall.exe"
      RMDir "$R8"
    ${EndIf}
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KaradelikDev"
    SetShellVarContext current
    Delete "$SMPROGRAMS\KaradelikDev.lnk"
    Delete "$DESKTOP\KaradelikDev.lnk"
    SetShellVarContext all
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; HKLM: bilgisayardaki tüm kullanıcılar; HKCU: kurulumu yapan kullanıcı (1.2.0'ın güncelleyicisi buradan okur)
  WriteRegStr HKLM "Software\KaradelikDev" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\KaradelikDev" "InstallDir" "$INSTDIR"
  ; Kullanıcı başına kurulumdan geçişte eski kısayollar silindi; güncelleme modu (/UPDATE) yenilerini oluşturmaz
  ${If} $R9 == "1"
    CreateShortcut "$SMPROGRAMS\KaradelikDev.lnk" "$INSTDIR\karadelikdev.exe"
    CreateShortcut "$DESKTOP\KaradelikDev.lnk" "$INSTDIR\karadelikdev.exe"
  ${EndIf}
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
    DeleteRegValue HKLM "Software\KaradelikDev" "InstallDir"
    DeleteRegKey /ifempty HKLM "Software\KaradelikDev"
    DeleteRegValue HKCU "Software\KaradelikDev" "InstallDir"
  ${EndIf}
!macroend
