; Post-install: remove Mark-of-the-Web (Zone.Identifier) without touching SAC/Defender.
!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Desbloqueando arquivos instalados (Unblock-File)..."
  nsExec::ExecToLog 'powershell.exe -NoProfile -WindowStyle Hidden -Command "Get-ChildItem -LiteralPath ''$INSTDIR'' -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object { Unblock-File -LiteralPath $_.FullName -ErrorAction SilentlyContinue }"'
  Pop $0
  DetailPrint "Unblock concluido."
!macroend
