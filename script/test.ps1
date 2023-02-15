#!/bin/sh

# script/test: Run test suite for application. Optionally pass in a path to an
#              individual test file to run a single test.

Set-Location "$PSScriptRoot\.."

Write-Output  "==> Running tests…"

cargo test
remove-item cargo-test*
