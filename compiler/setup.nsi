;--------------------------------
; Includes

!include "MUI2.nsh"
!include "logiclib.nsh"

;--------------------------------
; Command line options

!ifndef VERSION
!error "VERSION not defined. Requires -DVERSION=<0.0.0>"
!endif
!ifndef OUTFILE
!error "OUTFILE not defined. Requires -DOUTFILE=<FILENAME.EXE>"
!endif
!ifndef ARTIFACTSDIR
!error "ARTIFACTSDIR not defined. Requires -DARTIFACTSDIR=<DIRECTORY>"
!endif
!ifndef EXTENSION
; Normally the extension is .exe, but when building dummy tests on
; Linux, the extension is empty. This produces a valid installer
; containing an application that cannot run.
!define EXTENSION ".exe"
!endif

;--------------------------------
; Custom defines

; This affects the registry key that is how integrations
; find the path to the compiler. Don't change this without
; considering integrations.
!define NAME "IronPLC Compiler"
!define APPFILE "ironplcc${EXTENSION}"
!define SLUG "${NAME} v${VERSION}"
!define REGPATH_APPPATHSUBKEY "Software\Microsoft\Windows\CurrentVersion\App Paths\${APPFILE}"
!define REGPATH_UNINSTSUBKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${NAME}"

;--------------------------------
; General

Name "${NAME}"
OutFile "${OUTFILE}"
; INSTDIR is set as:
; [1] /D command line
; [2] The InstallDirRegKey default value
; [3] The InstallDir directory
InstallDir "$LocalAppData\Programs\${NAME}"
InstallDirRegKey HKCU "Software\${NAME}" ""
RequestExecutionLevel user
ManifestSupportedOS all

;--------------------------------
; UI
  
!define MUI_HEADERIMAGE
!define MUI_WELCOMEFINISHPAGE_BITMAP "nsis\assets\finished-banner.bmp"
!define MUI_HEADERIMAGE_BITMAP "nsis\assets\banner.bmp"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "${SLUG} Setup"
!define MUI_ICON "nsis\assets\logo.ico"
!define MUI_UNICON "nsis\assets\logo.ico"

;--------------------------------
; Pages
  
; Installer pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
  
; Set UI language
!insertmacro MUI_LANGUAGE "English"

;--------------------------------
; Section - Install App

Section "Program files"
    SectionIn RO

    SetOutPath "$INSTDIR"
    File "..\LICENSE" 

    SetOutPath "$INSTDIR\bin"
    File "${ARTIFACTSDIR}\${APPFILE}" 

    SetOutPath "$INSTDIR\examples"
    File "..\examples\getting_started.st"

    WriteRegStr HKCU "${REGPATH_APPPATHSUBKEY}" "" $INSTDIR\bin\${APPFILE}
    WriteRegStr HKCU "${REGPATH_APPPATHSUBKEY}" "Path" $INSTDIR\bin

    WriteRegStr HKCU "Software\${NAME}" "" $INSTDIR
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    WriteRegStr HKCU "${REGPATH_UNINSTSUBKEY}" "DisplayName" "${NAME}"
    WriteRegStr HKCU "${REGPATH_UNINSTSUBKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
    WriteRegStr HKCU "${REGPATH_UNINSTSUBKEY}" "DisplayIcon" "$INSTDIR\bin\${APPFILE},0"
SectionEnd

Section "Uninstall"
    ; Remove the directory with the uninstaller after restart as necessary
    RMDir /r /REBOOTOK $INSTDIR

    ; Remove the App Path and uninstaller information
    DeleteRegKey HKCU "${REGPATH_APPPATHSUBKEY}"
    DeleteRegKey HKCU "${REGPATH_UNINSTSUBKEY}"

    ; Remove the registry key that defines the install path
    DeleteRegKey HKCU "Software\${NAME}"
SectionEnd