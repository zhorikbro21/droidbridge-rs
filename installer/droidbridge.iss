; Inno Setup script for DroidBridge (droidbridge-rs)
; Build: ISCC.exe droidbridge.iss
; Bundles: droidbridge_rs.exe + adb + scrcpy (all runtime deps)

#define MyAppName "DroidBridge"
#define MyAppVersion "0.1.2"
#define MyAppExe "droidbridge_rs.exe"

; paths are overridable from the command line: ISCC /DSrcExe=... etc.
#ifndef SrcExe
#define SrcExe "C:\Users\MasterXP\Documents\droidbridge-rs\target\release"
#endif
#ifndef SrcScrcpy
#define SrcScrcpy "C:\Soft\android\scrcpy-win64-v4.0"
#endif
#ifndef DocsDir
#define DocsDir "C:\Users\MasterXP\Documents\droidbridge-rs"
#endif
#ifndef OutputDir
#define OutputDir "C:\Soft\android\PC_bridge"
#endif

[Setup]
AppId={{7E1C4B2A-9D33-4F58-8A21-C6B5E0D4A7F9}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=DroidBridge contributors
DefaultDirName={autopf}\DroidBridge
DefaultGroupName=DroidBridge
UninstallDisplayIcon={app}\{#MyAppExe}
OutputDir={#OutputDir}
OutputBaseFilename=droidbridge-setup-{#MyAppVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "autostart"; Description: "Start DroidBridge tray when Windows starts"; GroupDescription: "Autostart:"
Name: "desktopicon"; Description: "Desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked

[Files]
Source: "{#SrcExe}\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\scrcpy.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\adb.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\SDL3.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\AdbWinApi.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\AdbWinUsbApi.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\avcodec-62.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\avformat-62.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\avutil-60.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\swresample-6.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\libusb-1.0.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcScrcpy}\scrcpy-server"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DocsDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DocsDir}\config.example.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DocsDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
; tray autostart on Windows login (removed on uninstall)
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; \
    ValueName: "DroidBridgeRs"; ValueData: """{app}\{#MyAppExe}"""; \
    Tasks: autostart; Flags: uninsdeletevalue

[Icons]
Name: "{group}\DroidBridge"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\DroidBridge Settings"; Filename: "{app}\{#MyAppExe}"; Parameters: "--settings"
Name: "{autodesktop}\DroidBridge"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Run]
; register the Bluetooth auto-connect scheduled task (always, hidden)
Filename: "{app}\{#MyAppExe}"; Parameters: "--install"; Flags: runhidden
; optional desktop shortcut file creation is handled by [Icons]/[Tasks]
Filename: "{app}\{#MyAppExe}"; Description: "Launch DroidBridge now"; \
    Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\{#MyAppExe}"; Parameters: "--uninstall"; Flags: runhidden; RunOnceId: "DelBtTask"
