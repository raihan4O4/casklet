; Notify Explorer after replacing executable icon resources at the same path.
; Do not delete icon caches or restart Explorer.
!macro NSIS_HOOK_POSTINSTALL
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
