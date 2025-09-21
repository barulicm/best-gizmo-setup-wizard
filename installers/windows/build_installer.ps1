$innoCompilerPath = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"

if (Test-Path $innoCompilerPath) {
    & $innoCompilerPath ".\win_installer_definition.iss"
} else {
    Write-Error "Inno Setup Compiler not found at path: $innoCompilerPath"
}

Write-Host "Installer built to target/release directory"
