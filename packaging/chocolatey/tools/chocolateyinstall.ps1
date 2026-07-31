$ErrorActionPreference = 'Stop'

# $version and $checksum64 are rewritten by the release workflow on each tag.
$packageName = 'structurizrx'
$version     = '0.1.0'
$url64       = "https://github.com/pomali/structurizrx/releases/download/v$version/structurizrx-x86_64-pc-windows-msvc.zip"
$checksum64  = '0000000000000000000000000000000000000000000000000000000000000000'
$toolsDir    = Split-Path -Parent $MyInvocation.MyCommand.Definition

# Unzips structurizrx.exe into the tools dir; Chocolatey auto-shims the .exe onto PATH.
Install-ChocolateyZipPackage `
  -PackageName    $packageName `
  -Url64bit       $url64 `
  -Checksum64     $checksum64 `
  -ChecksumType64 'sha256' `
  -UnzipLocation  $toolsDir
