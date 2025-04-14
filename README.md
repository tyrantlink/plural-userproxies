# /plu/ral Userproxies

See the [Userproxy self-hosting docs](https://plural.gg/guide/self-hosting) for more information.

## Installation
### Windows
Download the binary from the [releases page](https://github.com/tyrantlink/plural-userproxies/releases).
### Linux and macOS
You can install directly using [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):
```sh
cargo install plural-userproxies
```
### Building from source
You can build from source using [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):
```sh
git clone https://github.com/tyrantlink/plural-userproxies
cd plural-userproxies
cargo build --release
```

## Usage
```sh
plural-userproxies
```

On first run, it will prompt you to enter your Plural API key.
From there, you can use `Ctrl+H` to view the list of keybindings in the application.