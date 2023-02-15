# script\audit.ps1: security checks

Set-Location "$PSScriptRoot\.."

cargo audit
