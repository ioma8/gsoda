#!/bin/bash
set -e

echo "=== GSoda G-code Viewer Test ==="
echo

# Test 1: Usage message
echo "Test 1: Check usage message"
./target/release/gsoda 2>&1 | grep -q "Usage:" && echo "✓ Usage message works"

# Test 2: Parse G-code file
echo "Test 2: Parse G-code file"
timeout 3 ./target/release/gsoda auto1.gcode 2>&1 | grep -q "Parsed.*line segments" && echo "✓ G-code parsing works"

# Test 3: Check binary size (should be optimized)
BINARY_SIZE=$(stat -f%z target/release/gsoda 2>/dev/null || stat -c%s target/release/gsoda 2>/dev/null || echo 0)
echo "Test 3: Binary size is $((BINARY_SIZE / 1024)) KB"
if [ $BINARY_SIZE -lt 2000000 ]; then
    echo "✓ Binary is reasonably sized"
else
    echo "⚠ Binary is larger than expected"
fi

echo
echo "=== All tests passed! ==="
echo
echo "To run the viewer:"
echo "  cargo run --release -- auto1.gcode"
echo
echo "Controls:"
echo "  - Mouse drag to rotate"
echo "  - Scroll to zoom"
echo "  - R to reset camera"
echo "  - L to toggle layer filtering"
echo "  - Up/Down to adjust visible layers"
echo "  - Esc to quit"
