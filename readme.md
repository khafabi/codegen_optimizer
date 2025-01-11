# Flutter Build Configuration Manager

A Rust-based tool to automate Flutter/Dart build configuration management and build processes.

## Features

- Automatically updates `build.yaml` based on Dart file annotations
- Supports common annotations:
  - `@CopyWith`
  - `@JsonSerializable`
  - `@HiveType`
- Runs essential Flutter build commands sequentially:
  - `flutter clean`
  - `flutter pub upgrade`
  - `flutter pub get`
  - `flutter pub run build_runner build --delete-conflicting-outputs`

## Requirements

- Rust (latest stable version)
- Flutter SDK (must be in PATH)
- Dart SDK (comes with Flutter)

## Installation

1. Clone this repository
2. Build the project:

```bash
cargo build --release
```

3. The binary will be in `target/release/`

## Usage

Run the tool from your Flutter project's root directory:

```bash
./flutter_build_manager
```

The tool will:
1. Verify Flutter installation
2. Scan Dart files for annotations
3. Update `build.yaml` accordingly
4. Run the full Flutter build process

## Configuration

The tool automatically detects your project structure. Place a `build.yaml` file in your project root if you don't have one.

## Error Handling

The tool provides detailed error messages for:
- Missing Flutter installation
- Command execution failures
- File processing errors
- YAML parsing errors

## Logging

The tool uses `log` and `simple_logger` for detailed logging. You'll see:
- Command execution times
- File processing status
- Build progress updates
- Error details

## Contributing

Contributions are welcome! Please open an issue or pull request.

## License

[MIT License](LICENSE)
