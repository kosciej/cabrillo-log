#!/bin/bash

# Build the WASM package
wasm-pack build --target web --out-dir release --release

# Copy the HTML file to the pkg directory
cp index.html release/

# Serve the static files
cd release
python3 -m http.server 8010

echo "Server running at http://localhost:8010"