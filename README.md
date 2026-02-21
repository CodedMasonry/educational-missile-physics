# educational-missile-physics
physics is difficult

## Licensing
- The source code of this project is licensed under the [Apache 2.0 License](LICENSE).
- 3D Assets located in the `/assets` folder are licensed under various licenses as noted below.

### Asset Credits
- **"Storm Shadow / SCALP-EG Low-poly"** by [FreshAlexei](https://www.cgtrader.com/freshalexei), used under [CGTrader Royalty Free License](https://help.cgtrader.com/hc/en-us/articles/360015124437-Royalty-Free-License). 
  - Source: [CGTrader](https://www.cgtrader.com/free-3d-models/military/rocketry/storm-shadow-scalp-eg-low-poly)
- **"Exocet MM40 Block 3C Low Poly Game-Ready Missile"** by [lash2145](https://www.cgtrader.com/designers/lash2145), used under [CGTrader Royalty Free License](https://help.cgtrader.com/hc/en-us/articles/360015124437-Royalty-Free-License).
  - Source: [CGTrader](https://www.cgtrader.com/free-3d-models/military/rocketry/exocet-mm40-block-3c-low-poly-game-ready-missile)
  
## Compiling

It is highly recommended to compile with the nightly compiler, and when compiling the code on linux,
make sure to install `clang` and `mold`.

When editing the code `cranelift` provides further optimizations.
```bash
rustup component add rustc-codegen-cranelift-preview --toolchain nightly
```
