# script\build.ps1: build binary 
#
Set-Location "$PSScriptRoot\.."

cargo build --release
