# OpenJax Install (Windows x86_64)

This package is prebuilt for **Windows x86_64** only.

## Install

```powershell
.\install.ps1
```

Custom prefix:

```powershell
.\install.ps1 -Prefix "$HOME\.local\openjax"
```

## Verify

```powershell
.\bin\openjaxd.exe --help
```

## Uninstall

```powershell
.\uninstall.ps1
```

Keep future user data directory (if present):

```powershell
.\uninstall.ps1 -KeepUserData
```
