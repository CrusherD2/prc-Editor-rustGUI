# PRC Editor (Rust)

A cross-platform GUI application for editing Smash Ultimate parameter files (.prc, .prcx, .stdat, etc.), built in Rust using the egui framework.

## Features

- **Cross-platform**: Works on Windows, macOS, and Linux
- **TreeView navigation**: Navigate parameter hierarchy like the original prcEditor
- **Parameter editing**: View and edit parameter values in a structured format
- **Hash label support**: Load ParamLabels.csv for human-readable parameter names
- **Multiple file formats**: Support for .prc, .prcx, .stdat, .stdatx, .stprm, .stprmx files

## Installation

### Prerequisites

- Rust (latest stable version)
- Git

### Building from source

```bash
git clone <repository-url>
cd prc-editor-rust
cargo build --release
```

### Running

```bash
cargo run
```

Or run the compiled binary:

```bash
./target/release/prc-editor-rust
```

## Usage

1. **Open a file**: Use File > Open to load a .prc or other supported file
2. **Navigate parameters**: Use the tree view on the left to browse parameter hierarchy
3. **View details**: Select a parameter to view its details in the right panel
4. **Load labels**: The app automatically looks for ParamLabels.csv for hash resolution

## Project Structure

- `src/main.rs` - Application entry point
- `src/ui.rs` - Main GUI implementation using egui
- `src/param_file.rs` - File parsing logic
- `src/param_types.rs` - Parameter type definitions
- `src/hash_labels.rs` - Hash label management
- `ParamLabels.csv` - Hash to label mapping file

## Dependencies

- **egui**: Immediate mode GUI framework
- **eframe**: Application framework for egui
- **rfd**: Native file dialogs
- **csv**: CSV file parsing
- **byteorder**: Binary data reading
- **anyhow**: Error handling

## Compatibility

This Rust version aims to be functionally equivalent to the original C# prcEditor while being cross-platform compatible. It uses the same parsing logic and file format understanding.

## TODO

- [ ] Complete parameter tree construction
- [ ] Implement parameter value editing
- [ ] Add file saving functionality
- [ ] Implement label downloading
- [ ] Add keyboard shortcuts
- [ ] Improve error handling and validation

## License

This project follows the same license as the original paracobNET library. 