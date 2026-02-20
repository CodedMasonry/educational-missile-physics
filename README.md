# educational-missile-physics
physics is difficult

## Licensing
- The source code of this project is licensed under the [Apache 2.0 License](LICENSE).
- 3D Assets located in the `/assets` folder are licensed under [CC-BY-NC-4.0](http://creativecommons.org/licenses/by-nc/4.0/).

### Asset Credits
- **"Russian X-555 air-launched cruise missile"** by [Dmitriy Mitroshin](https://sketchfab.com/LtxxwSibeRia), used under [CC-BY-NC-4.0](http://creativecommons.org/licenses/by-nc/4.0/). 
  - Source: [Sketchfab](https://sketchfab.com/3d-models/russian-x-555-air-launched-cruise-missile-204c992ab27c4c1cad6a60b7c20b8c01)

## Compiling

It is highly recommended to compile with the nightly compiler, and when compiling the code on linux,
make sure to install `clang` and `mold`.

When editing the code `cranelift` provides further optimizations.
```bash
rustup component add rustc-codegen-cranelift-preview --toolchain nightly
```
