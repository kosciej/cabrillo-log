#!/bin/bash

# Build the WASM package
wasm-pack build --target web --out-dir pkg

# Copy the HTML file to the pkg directory
cp index.html pkg/

# Serve the static files
cd pkg
python3 -m http.server 8000

echo "Server running at http://localhost:8000"