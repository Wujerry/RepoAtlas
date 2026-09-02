; Tauri NSIS installer hooks. Runs inside the generated installer before the
; file-copy phase. nsExec captures taskkill output so no console window
; flashes; the exit code is popped to keep the NSIS stack balanced. A short
; sleep lets Windows release the file handles before the installer copies
; over them — otherwise an overwrite of a running exe fails with
; "Error opening file for writing".
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM repoatlas.exe /T'
  Pop $0
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM repoatlas-mcp.exe /T'
  Pop $0
  Sleep 1000
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM repoatlas.exe /T'
  Pop $0
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM repoatlas-mcp.exe /T'
  Pop $0
  Sleep 1000
!macroend
