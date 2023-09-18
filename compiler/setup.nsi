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
!ifndef EXTENSION
; Normally the extension is .exe, but when building dummy tests on
; Linux, the extension is empty. This produces a valid installer
; containing an application that cannot run.
!define EXTENSION ".exe"
!endif

;--------------------------------
; Custom defines

!define NAME "IronPLC Compiler"
!define APPFILE "ironplcc${EXTENSION}"
!define SLUG "${NAME} v${VERSION}"

;--------------------------------
; General

Name "${NAME}"
OutFile "${OUTFILE}"
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

;--------------------------------
; Pages
  
; Installer pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
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
    File "target\release\${APPFILE}" 

    SetOutPath "$INSTDIR\examples"
    File "..\examples\getting_started.st"

    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\${APPFILE}" "" $INSTDIR\bin\${APPFILE}

    WriteRegStr HKCU "Software\${NAME}" "" $INSTDIR
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${NAME}" \
                 "DisplayName" "${NAME}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${NAME}" \
                 "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
SectionEnd

Section "Uninstall"
    RMDir /r /REBOOTOK $INSTDIR
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\${APPFILE}"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${NAME}"
SectionEnd