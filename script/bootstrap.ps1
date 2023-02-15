# script\boostrap: Resolve all dependencies that the application needs to run

Set-Location "$PSScriptRoot\.."

Write-Output "checking if cargo is installed"
Get-Date 

if (Get-Command 'cargo' -errorAction SilentlyContinue) {
    "cargo installed"
} else {
    Write-Output "cargo not found installing"
    Get-Date
    scoop install rustup
}
if (Get-Command 'grcov' -errorAction SilentlyContinue) {
    "grcov installed"
} else {
    Write-Output "grcov not found installing"
    Get-Date
    cargo install grcov
}


cargo install cargo-audit
