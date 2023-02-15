# script\coverage.ps1: Run test suite for application and apply code coverage, finish with opening browser in html to see report.

#
Set-Location "$PSScriptRoot\.."

$Env:CARGO_INCREMENTAL = 0 
$Env:RUSTFLAGS = '-Cinstrument-coverage' 
$Env:LLVM_PROFILE_FILE = 'cargo-test-%p-%m.profraw'
cargo test
Remove-Item target\coverage\html
New-Item  target\coverage\html -ItemType Directory
grcov . --binary-path .\target\debug\deps/ -s . -t html --branch --ignore-not-existing --ignore '../*' --ignore "/*" -o target\coverage\html
Remove-Item cargo-test*
explorer.exe target\coverage\html\index.html

