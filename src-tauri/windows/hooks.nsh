; Instalacao limpa: mata processo, remove legado, limpa cache WebView2.

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Encerrando promptub..."
  nsExec::ExecToLog 'taskkill /F /IM promptub.exe /T'
  Pop $0

  DetailPrint "Removendo arquivos legados em $INSTDIR..."
  Delete "$INSTDIR\mpv.exe"
  Delete "$INSTDIR\*.dll"
  Delete "$INSTDIR\tools\mpv.exe"
  Delete "$INSTDIR\tools\d3dcompiler_43.dll"
  Delete "$INSTDIR\tools\*.dll"
  RMDir /r "$INSTDIR\resources"
  RMDir /r "$INSTDIR\_up_"

  ; Recria tools/ vazio — instalador copia yt-dlp.exe em seguida
  RMDir /r "$INSTDIR\tools"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Desbloqueando arquivos instalados (Unblock-File)..."
  nsExec::ExecToLog 'powershell.exe -NoProfile -WindowStyle Hidden -Command "Get-ChildItem -LiteralPath ''$INSTDIR'' -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object { Unblock-File -LiteralPath $_.FullName -ErrorAction SilentlyContinue }"'
  Pop $0

  DetailPrint "Limpando cache WebView2 (forca tema/CSS novo)..."
  nsExec::ExecToLog 'powershell.exe -NoProfile -WindowStyle Hidden -Command "$d = Join-Path $env:LOCALAPPDATA ''com.promptub''; if (Test-Path $d) { Remove-Item -LiteralPath $d -Recurse -Force -ErrorAction SilentlyContinue }"'
  Pop $0
  DetailPrint "Instalacao limpa concluida."
!macroend
