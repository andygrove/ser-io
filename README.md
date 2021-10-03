# ser-io

[![crates.io](https://img.shields.io/crates/v/ser-io.svg)](https://crates.io/crates/ser-io)

Rust crate for reading and writing SER video files, commonly used in astrophotography.

## Usage

```rust,no_run
let ser = SerFile::open(filename)?;

println!("Image size: {} x {}", ser.image_width, ser.image_height);
println!("Frame count: {}", ser.frame_count);
println!("Frame size: {}", ser.image_frame_size);
println!("Bytes per pixel: {}", ser.bytes_per_pixel);
println!("Bayer: {:?}", ser.bayer);
println!("Endianness: {:?}", ser.endianness);

for i in 0..ser.frame_count {
    let bytes = ser.read_frame(i)?;
    // do processing ...
}
```

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