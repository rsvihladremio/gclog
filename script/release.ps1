# script\release.ps1: build several binaries, cut a github release 
#                 and push the new builds up to it

param(
     $VERSION
)

Set-Location "$PSScriptRoot\.."

VERSION=$1

# build mac
cargo build --target x86_64-apple-darwin --release
# build mac m1 
cargo build --target aarch64-apple-darwin --release
# build windows - depends on mingw-w64
cargo build --target x86_64-pc-windows-gnu --release
# build linux - export CC_x86_64_unknown_linux_gnu=x86_64-unknown-linux-gnu-gcc
$Env:CXX_x86_64_unknown_linux_gnu = x86_64-unknown-linux-gnu-g++
$Env:AR_x86_64_unknown_linux_gnu = x86_64-unknown-linux-gnu-ar
$Env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = x86_64-unknown-linux-gnu-gcc
cargo build --target x86_64-unknown-linux-gnu --release

# depends on brew install zip 

zip  target\$VERSION-aarch64-apple-darwin.zip target\aarch64-apple-darwin\release\gclog
zip  target\$VERSION-x86_64-apple-darwin.zip target\x86_64-apple-darwin\release\gclog
zip  target\$VERSION-x86_64-pc-windows-gnu.zip target\x86_64-pc-windows-gnu\release\gclog.exe
zip  target\$VERSION-x86_64-unknown-linux-gnu.zip target\x86_64-unknown-linux-gnu\release\gclog

# depends on brew install gh
gh release create $VERSION --title $VERSION -d -F changelog.md target\$VERSION-aarch64-apple-darwin.zip target\$VERSION-x86_64-apple-darwin.zip  target\$VERSION-x86_64-pc-windows-gnu.zip target\$VERSION-x86_64-unknown-linux-gnu.zip
