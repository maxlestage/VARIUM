; Installateur Windows de Varium (Inno Setup 6), compilé par le workflow de release :
;   iscc /DVersion=0.6.0 /DSource=dist packaging\windows\varium.iss
; Installation par utilisateur (sans droits administrateur), raccourcis, et
; associations de fichiers :
;   - Varium apparaît dans « Ouvrir avec » pour .json (documents, classeurs et
;     présentations Varium : .varium.json, .varium-calc.json, .varium-slides.json —
;     Windows ne connaît que la dernière extension), .docx, .odt, .md, .txt,
;     .xlsx, .ods, .csv, .odp et .pptx ;
;   - l'extension .varium (ancienne) s'ouvre directement dans Varium.
; Varium ne s'impose pas comme programme par défaut de ces formats : Windows le
; demande à l'utilisateur au premier double-clic (« Toujours utiliser cette application »).

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Source
  #define Source "..\..\dist"
#endif

[Setup]
AppId={{6F1E4C7A-2B7D-4C5E-9D63-5A0B7C3E9F21}
AppName=Varium
AppVersion={#Version}
AppPublisher=Varium
AppPublisherURL=https://maxlestage.github.io/VARIUM/
DefaultDirName={localappdata}\Programs\Varium
DefaultGroupName=Varium
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..
OutputBaseFilename=varium-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\varium.exe
ChangesAssociations=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "it"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "pt"; MessagesFile: "compiler:Languages\Portuguese.isl"

[Tasks]
Name: "bureau"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\varium.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\LICENSE-OFL.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Varium"; Filename: "{app}\varium.exe"
Name: "{autodesktop}\Varium"; Filename: "{app}\varium.exe"; Tasks: bureau

[Registry]
; Le programme et sa commande d'ouverture (identifiant de programme « Varium.Fichier »).
Root: HKA; Subkey: "Software\Classes\Varium.Fichier"; ValueType: string; ValueName: ""; ValueData: "Fichier Varium"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Varium.Fichier\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\varium.exe,0"
Root: HKA; Subkey: "Software\Classes\Varium.Fichier\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\varium.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Varium"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\varium.exe"" ""%1"""
; .varium : directement Varium.
Root: HKA; Subkey: "Software\Classes\.varium"; ValueType: string; ValueName: ""; ValueData: "Varium.Fichier"; Flags: uninsdeletevalue
; « Ouvrir avec » : Varium proposé sans prendre la place du programme par défaut.
Root: HKA; Subkey: "Software\Classes\.json\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.docx\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.odt\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.txt\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.xlsx\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.ods\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.csv\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.odp\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.pptx\OpenWithProgids"; ValueType: none; ValueName: "Varium.Fichier"; Flags: uninsdeletevalue
; Les types reconnus par Varium (« Ouvrir avec » de l'Explorateur).
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".json"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".varium"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".docx"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".odt"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".xlsx"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".ods"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".csv"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".odp"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\varium.exe\SupportedTypes"; ValueType: string; ValueName: ".pptx"; ValueData: ""

[Run]
Filename: "{app}\varium.exe"; Description: "{cm:LaunchProgram,Varium}"; Flags: nowait postinstall skipifsilent
