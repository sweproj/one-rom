#!/bin/bash
echo "Starting HTTP server..."
echo "Navigate to: http://sb1:8000/tests/wasm/test.html"
echo ""
python3 -m http.server 8000 --bind 0.0.0.0
