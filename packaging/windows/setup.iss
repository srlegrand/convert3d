; Inno Setup script for the Windows installer (Convert3D-Setup.exe).
; Build:  iscc /DAppVersion=1.2.3 packaging\windows\setup.iss   -> dist\Convert3D-Setup.exe
; Expects target\release\convert3d.exe, or /DExe=path\to\convert3d.exe.
; Installs for the current user only - no admin rights needed.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Exe
  #define Exe "..\..\target\release\convert3d.exe"
#endif

[Setup]
AppId={{B7E4A9D2-3C61-4F0B-9A58-2D7C1E6F8A43}
AppName=Convert 3D
AppVersion={#AppVersion}
AppPublisher=Simon Legrand
AppPublisherURL=https://srlegrand.github.io/convert3d/
AppSupportURL=https://github.com/srlegrand/convert3d/issues
; The folder `convert3d install` uses, so the setup's copy is the installed copy.
DefaultDirName={localappdata}\convert3d
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
OutputDir=..\..\dist
OutputBaseFilename=Convert3D-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName=Convert 3D

[Tasks]
Name: "blender"; Description: "Download and install Blender with winget (required - it was not found on this PC)"; Check: not HasBlender

[Files]
Source: "{#Exe}"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Run]
Filename: "{cmd}"; Parameters: "/c winget install --id BlenderFoundation.Blender -e --accept-source-agreements --accept-package-agreements"; StatusMsg: "Installing Blender (this can take a few minutes)..."; Tasks: blender
; After Blender, so the menu lists everything that was found.
Filename: "{app}\bin\convert3d.exe"; Parameters: "install"; StatusMsg: "Adding the right-click menus..."; Flags: runhidden

[UninstallRun]
Filename: "{app}\bin\convert3d.exe"; Parameters: "uninstall"; Flags: runhidden; RunOnceId: "RemoveMenu"

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Messages]
FinishedLabel=Setup has finished. Right-click any 3D file and choose "Convert 3D to" or "Preview 3D".%n%nOn Windows 11 they are under "Show more options" (or Shift + right-click).

[Code]
function HasBlender: Boolean;
var
  Found: TFindRec;
  Roots: array[0..1] of String;
  I: Integer;
begin
  Result := FileSearch('blender.exe', GetEnv('PATH')) <> '';
  Roots[0] := GetEnv('ProgramW6432');
  Roots[1] := GetEnv('ProgramFiles');
  for I := 0 to 1 do
    if (not Result) and (Roots[I] <> '') and FindFirst(Roots[I] + '\Blender Foundation\Blender*', Found) then
    begin
      Result := True;
      FindClose(Found);
    end;
end;
