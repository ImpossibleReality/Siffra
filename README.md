<div align="center">
    <img src="src-tauri/app-icon.png" alt="Siffra logo" width="196" height="196">
</div>


# Siffra

Siffra is a beautiful, lightweight, fast, and accurate calculator with support for dimensional analysis.
The project is currently in heavy development, and is not currently stable yet. However, it is usable.

## Installation
### Building from source
To build Siffra from source, you will need to have the following installed:
- [Rust](https://www.rust-lang.org/tools/install)
- [Bun](https://bun.sh)

Once you have these installed, you can clone the repository and build Siffra with the following commands:
```sh
bun install
bun run tauri build
```

You will find the built binary in the `src-tauri/target/release/bundle` directory.

note: Siffra currently only *officially* supports macOS due to testing limitations, but I am working on adding support for Linux and Windows. Please open an issue if you have any problems running Siffra on your platform.

## Contributing

Because Siffra is still under heavy development, I am not currently accepting contributions. However, once the project is more stable, I will be accepting contributions. Please do, however, feel free to [open an issue](https://github.com/ImpossibleReality/Siffra/issues/new) if you find a bug or have a feature request.

## License

Siffra is licensed under the [MIT License](https://opensource.org/license/mit/)
