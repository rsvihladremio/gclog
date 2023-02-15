# script/lint: Validate formatting and code quality 

Set-Location "$PSScriptRoot\.."

echo "==> Running code quality check"

cargo clippy

echo "==> Running fmt"

cargo fmt --all -- --check
