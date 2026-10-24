; Ajouts de Révisio à l'installeur NSIS de Tauri (inclus par installer.nsi, après utils.nsh).
;
; Tauri vérifie que l'app est fermée en demandant au Gestionnaire de redémarrage de Windows
; « qui utilise revisio.exe ? ». Un antivirus (ex. McAfee Framework Host) garde souvent ce fichier
; ouvert : l'installeur croyait alors Révisio lancé, ne pouvait pas fermer l'antivirus, et refusait
; d'installer ou de désinstaller jusqu'au redémarrage du PC.
; On cherche donc un vrai PROCESSUS revisio.exe de l'utilisateur, et on propose de le fermer.

!macroundef CheckIfAppIsRunning
!macro CheckIfAppIsRunning executablePath productName
  !define UniqueID ${__LINE__}

  nsis_tauri_utils::StrReplace "$(appRunning)" "{{product_name}}" "${productName}"
  Pop $R1
  nsis_tauri_utils::StrReplace "$(appRunningOkKill)" "{{product_name}}" "${productName}"
  Pop $R2
  nsis_tauri_utils::StrReplace "$(failedToKillApp)" "{{product_name}}" "${productName}"
  Pop $R3

  ; 0 = un processus de ce nom tourne pour l'utilisateur actuel
  nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
  Pop $R0
  ${If} $R0 = 0
    IfSilent kill_${UniqueID} 0
    ${IfThen} $PassiveMode != 1 ${|} MessageBox MB_OKCANCEL $R2 IDOK kill_${UniqueID} IDCANCEL cancel_${UniqueID} ${|}
    kill_${UniqueID}:
      nsis_tauri_utils::KillProcessCurrentUser "${MAINBINARYNAME}.exe"
      Pop $R0
      Sleep 500
      ${If} $R0 = 0
        Goto app_check_done_${UniqueID}
      ${EndIf}
      IfSilent 0 +2
        Abort
      Abort $R3
    cancel_${UniqueID}:
      Abort $R1
  ${EndIf}
  app_check_done_${UniqueID}:

  !undef UniqueID
!macroend
