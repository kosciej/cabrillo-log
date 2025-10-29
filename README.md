# Cabrillo Log

## Project Structure

## Prerequisites

### Install Rust

You need Rust.

### Install WASM Console Tools

To build and run the WebAssembly components, you need:

```bash
# Install wasm-pack (WASM build tool)
cargo install wasm-pack

# For serving static files (choose one)
# Option 1: Python (usually pre-installed)
python3 -m http.server

# Option 2: Node.js with serve
npm install -g serve

# Option 3: Any other static file server
```

## Building and Running

### Console Tools

To build all workspace crates:

```bash
# Build all crates
cargo build

# Run specific crate (example)
cargo run --bin cabrillo-log
```

### Web Interface (web_static)

The web interface can be run in two modes:

#### Development Mode

```bash
cd web_static
chmod +x serve.sh
./serve.sh
```

This will:
1. Build the WASM package for development
2. Copy the HTML file
3. Start a development server at http://localhost:8000
