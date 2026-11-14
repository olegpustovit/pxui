#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
sdk=${1:-iphonesimulator}
case "$sdk" in
    iphoneos) target=aarch64-apple-ios ;;
    iphonesimulator) target=aarch64-apple-ios-sim ;;
    *) echo "usage: $0 [iphoneos|iphonesimulator] [build-root]" >&2; exit 2 ;;
esac
build_root=${2:-"$PWD/target"}
packaging/build-sdl-ios.sh "$sdk" "$build_root"
build_root=$(cd "$build_root" && pwd)
SDL3_DIR="$build_root/sdl-$sdk" CARGO_TARGET_DIR="$build_root" \
    IPHONEOS_DEPLOYMENT_TARGET=18.0 cargo build -p ios_spike --target "$target"
bundle="$build_root/$target/debug/PXUIHostSpike.app"
mkdir -p "$bundle"
cp "$build_root/$target/debug/pxui-ios-spike" "$bundle/PXUIHostSpike"
cat > "$bundle/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>PXUIHostSpike</string>
<key>CFBundleIdentifier</key><string>dev.pxui.host-spike</string>
<key>CFBundleName</key><string>PXUI Host Spike</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1</string>
<key>MinimumOSVersion</key><string>18.0</string>
<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
<key>UILaunchScreen</key><dict/>
<key>UIApplicationSupportsIndirectInputEvents</key><true/>
<key>UISupportedInterfaceOrientations</key><array><string>UIInterfaceOrientationPortrait</string><string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string></array>
</dict></plist>
EOF
plutil -lint "$bundle/Info.plist"
# Device builds need the developer's provisioning profile before installation.
if [ "$sdk" = iphonesimulator ]; then codesign --force --sign - "$bundle"; fi
echo "SDL host capability spike: $bundle"
