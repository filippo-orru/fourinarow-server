- download musl from win.musl.cc
- add ./bin to path
- rustup target add stable-x86_64-unknown-linux-musl
- cargo build --release --target x86_64-unknown-linux-musl
    If musl error:
    - set CC="x86_64-linux-musl-gcc"
    - set TARGET_CC="x86_64-linux-musl-gcc"
    - clone "x86_64-linux-musl-gcc" and rename to "cc"
    If linker.exe missing:
    run from x64 Native Development CMD - Visual Studio 20XX