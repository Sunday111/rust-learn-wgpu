# How to run

Get some compiler if needed and install rust.

```bash
sudo apt install build-essential
```

## Regular targets (Windows, Linux)

```bash
cargo run --bin tutorial8-depth
```

## WASM

Tell rust that we will target wasm.

```bash
rustup target add wasm32-unknown-unknown
```

Install wasm-apck
```bash
cargo install wasm-pack
```

Then you can use `build_wasm.py` to build all targets.
They will end up in `wasm-build` directory.

You can start a server using python to test it:

```bash
python3 -m http.server 8000 --directory ./wasm-build/
```

and open the list of available pages at http://localhost:8000/ (or open any of them directly http://localhost:8000/tutorial_8.html).
