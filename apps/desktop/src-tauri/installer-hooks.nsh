; Notify Explorer after replacing executable icon resources at the same path.
; Do not delete icon caches or restart Explorer.
!macro NSIS_HOOK_POSTINSTALL
  ; Run the installed binary synchronously; registration never opens a WebView.
  nsExec::ExecToLog '"$INSTDIR\casklet-desktop.exe" --register-explorer'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Casklet Explorer integration could not be registered (code $0). Reinstall to retry."
    MessageBox MB_OK|MB_ICONEXCLAMATION "Casklet was installed, but Explorer integration could not be registered. Reinstall Casklet to retry." /SD IDOK
  ${EndIf}
  ; Deliver the association change before completing installation.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
  ; Explorer can retain both handler lists and loaded COM factories. Opening
  ; an existing adapter for write detects a mapped image without writing or
  ; truncating it. Never terminate Explorer or disrupt an active file transfer.
  StrCpy $R4 0
  ReadRegStr $R5 HKCU "Software\Casklet\Explorer" "ClassId"
  ReadRegStr $R6 HKCU "Software\Classes\CLSID\$R5\InprocServer32" ""
  FindFirst $R0 $R1 "$INSTDIR\explorer\casklet-explorer-*.dll"
  ${DoWhile} $R1 != ""
    ${If} "$INSTDIR\explorer\$R1" != $R6
    System::Call 'kernel32::CreateFileW(w "$INSTDIR\explorer\$R1", i 0x40000000, i 7, p 0, i 3, i 0x80, p 0) p .r2'
    ${If} $2 == -1
      StrCpy $R4 1
    ${Else}
      System::Call 'kernel32::CloseHandle(p r2)'
    ${EndIf}
    ${EndIf}
    FindNext $R0 $R1
  ${Loop}
  FindClose $R0
  ${If} $R4 == 1
    DetailPrint "Explorer is using a previous menu adapter. Restart Windows Explorer after file transfers finish to activate the update."
    MessageBox MB_OK|MB_ICONINFORMATION "Casklet is updated, but Windows Explorer is still using a loaded menu adapter. Finish any file transfers, then restart Windows Explorer (or sign out and back in) to activate the updated menu." /SD IDOK
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; The active COM identity changes with the adapter source so Explorer cannot
  ; reuse a class factory from an earlier DLL after an upgrade.
  ReadRegStr $0 HKCU "Software\Classes\*\shellex\ContextMenuHandlers\Casklet" ""
  StrCpy $1 $0 20
  ${If} $1 == "{960e5343-e03e-47cd-"
    DeleteRegKey HKCU "Software\Classes\CLSID\$0"
  ${EndIf}
  DeleteRegKey HKCU "Software\Classes\*\shellex\ContextMenuHandlers\Casklet"
  DeleteRegKey HKCU "Software\Classes\Directory\shellex\ContextMenuHandlers\Casklet"
  DeleteRegKey HKCU "Software\Classes\CLSID\{960e5343-e03e-47cd-b1a3-d58632778824}"
  DeleteRegKey HKCU "Software\Classes\*\shell\Casklet"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\Casklet"
  DeleteRegKey HKCU "Software\Classes\Casklet.Explorer"
  DeleteRegKey HKCU "Software\Casklet\Explorer"
  ; Windows may retain a loaded shell DLL until Explorer releases it.
  Delete /REBOOTOK "$INSTDIR\explorer\casklet-explorer-*.dll"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
