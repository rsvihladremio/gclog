# script\clean.ps1: cleanup target dir and coverage files

Set-Location "$PSScriptRoot\.."

cargo clean

Remove-Item cargo-test*
