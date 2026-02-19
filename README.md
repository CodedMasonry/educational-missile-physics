# educational-missile-physics
physics is difficult

## Compiling

It is highly recommended to compile with the nightly compiler, and when compiling the code on linux,
make sure to install `clang` and `mold`.

When editing the code `cranelift` provides further optimizations.
```bash
rustup component add rustc-codegen-cranelift-preview --toolchain nightly
```
