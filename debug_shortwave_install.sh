#!/bin/bash

echo "=== Shortwave Installation Debug ==="
echo ""

echo "1. Checking if shortwave package is installed:"
pacman -Qi shortwave-mpris-git 2>/dev/null && echo "✓ Package installed" || echo "✗ Package not found"

echo ""
echo "2. Checking where shortwave binary is located:"
which shortwave 2>/dev/null && echo "✓ Found in PATH: $(which shortwave)" || echo "✗ Not found in PATH"

echo ""
echo "3. Searching for shortwave binary in common locations:"
for dir in /usr/bin /usr/local/bin /opt/bin; do
    if [ -f "$dir/shortwave" ]; then
        echo "✓ Found at: $dir/shortwave"
        ls -la "$dir/shortwave"
    fi
done

echo ""
echo "4. Current PATH:"
echo "$PATH"

echo ""
echo "5. Checking desktop file:"
if [ -f /usr/share/applications/de.haeckerfelix.Shortwave.desktop ]; then
    echo "✓ Desktop file found"
    grep "Exec=" /usr/share/applications/de.haeckerfelix.Shortwave.desktop
else
    echo "✗ Desktop file not found"
fi

echo ""
echo "6. Manual test - try running with full path:"
if [ -f /usr/bin/shortwave ]; then
    echo "Testing: /usr/bin/shortwave --version"
    /usr/bin/shortwave --version 2>&1 | head -3
else
    echo "✗ /usr/bin/shortwave not found"
fi
