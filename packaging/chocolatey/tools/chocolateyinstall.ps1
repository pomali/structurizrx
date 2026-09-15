$ErrorActionPreference = 'Stop'

# $version and $checksum64 are rewritten by the release workflow on each tag.
$packageName = 'structurizrx'
$version     = '0.2.0'
$url64       = "https://github.com/pomali/structurizrx/releases/download/v$version/structurizrx-x86_64-pc-windows-msvc.zip"
$checksum64  = '25e4e78dc46939b5f64a3658303b8d6c40beac730c1791f9cb4ea4b7daa2f507'
$toolsDir    = Split-Path -Parent $MyInvocation.MyCommand.Definition

# Unzips structurizrx.exe into the tools dir; Chocolatey auto-shims the .exe onto PATH.
Install-ChocolateyZipPackage `
  -PackageName    $packageName `
  -Url64bit       $url64 `
  -Checksum64     $checksum64 `
  -ChecksumType64 'sha256' `
  -UnzipLocation  $toolsDir
