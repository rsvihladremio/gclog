# script/update: Update application to run for its current checkout.

Set-Location "$PSScriptRoot\.."

echo "==> Cleaning repository"
cargo clean

echo "==> Running bootstrap"
script\bootstrap.ps1

