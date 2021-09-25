# ser-io

[![crates.io](https://img.shields.io/crates/v/riff-io.svg)](https://crates.io/crates/riff-io)

Rust crate for reading SER files used in astrophotography.

## Example

```text,no_run
$ cargo run --example view-ser ~/Documents/2021-09-20-0323_1-CapObj.SER
 
Image size: 4144 x 2822
Frame count: 100
Frame size: 23388736
Bytes per pixel: 2
Bayer: RGGB
Endianness: LittleEndian
```

## Resources

- [SER File Format](http://www.grischa-hahn.homepage.t-online.de/astro/ser/)