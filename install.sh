#!/usr/bin/env bash
set -e

BINARY_NAME="rex"
INSTALL_PATH="/usr/local/bin/$BINARY_NAME"
DESKTOP_ENTRY_PATH="/usr/share/applications/$BINARY_NAME.desktop"
ICON_PATH="/usr/share/icons/hicolor/128x128/apps/$BINARY_NAME.png" # Change if needed

echo "[*] Building in release mode..."
cargo build --release

echo "[*] Stripping binary..."
strip "target/release/$BINARY_NAME"

if [ "$EUID" -ne 0 ]; then
    echo "[*] Root privileges required to install. Prompting for sudo..."
    echo "[*] Installing to $INSTALL_PATH..."
    exec sudo install -Dm755 "target/release/$BINARY_NAME" "$INSTALL_PATH"
else
    echo "[*] Installing to $INSTALL_PATH..."
    install -Dm755 "target/release/$BINARY_NAME" "$INSTALL_PATH"
fi

desktop_env=""
if command -v xfce4-session >/dev/null 2>&1; then
    desktop_env="xfce4"
elif command -v plasmashell >/dev/null 2>&1; then
    desktop_env="kde"
fi

if [ -n "$desktop_env" ]; then
    echo "[*] Detected desktop environment: $desktop_env"
    echo "[*] Creating .desktop shortcut..."

    cat > "$DESKTOP_ENTRY_PATH" <<EOF
[Desktop Entry]
Type=Application
Name=Rex
Exec=$INSTALL_PATH
Icon=$BINARY_NAME
Terminal=false
Categories=Utility;
EOF

    chmod 644 "$DESKTOP_ENTRY_PATH"
    echo "[+] Shortcut installed to $DESKTOP_ENTRY_PATH"
else
    echo "[!] No supported desktop environment detected. Skipping .desktop creation."
fi

echo "[✓] Installation complete."

