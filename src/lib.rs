// MIT License
//
// Copyright (c) 2021 Andy Grove
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#![doc = include_str!("../README.md")]

use std::fs::{self, File};
use std::io::{Error, ErrorKind, Result, Write};
use std::str;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap::{Mmap, MmapOptions};

const HEADER_SIZE: usize = 178;

const MAGIC: &str = "LUCAM-RECORDER";

/// SER file
pub struct SerFile {
    /// Memory-mapped file
    mmap: Mmap,
    /// SER header
    pub header: SerHeader,
    /// Timestamp in UTC of each frame
    pub timestamps: Vec<u64>,
}

#[derive(Debug)]
pub struct SerHeader {
    /// Image height, in pixels
    pub image_height: u32,
    /// Image width, in pixels
    pub image_width: u32,
    /// Number of frames
    pub frame_count: usize,
    /// Pixel depth per plane
    pub pixel_depth_per_plane: u32,
    /// Number of butes per pixel (1 or 2)
    /// The endianness of encoded image data. This is only relevant if the image data is 16-bit
    pub endianness: Endianness,
    /// Bayer encoding
    pub bayer: Bayer,
    /// Name of observer
    pub observer: String,
    /// Name of telescope
    pub telescope: String,
    /// Name of instrument
    pub instrument: String,
    /// File timestamp
    pub date_time: u64,
    /// File timestamp in UTC
    pub date_time_utc: u64,
}

impl SerHeader {
    /// Total number of image bytes in the file
    pub fn image_data_bytes(&self) -> usize {
        self.image_frame_size() * self.frame_count
    }

    /// Number of bytes per image frame
    pub fn image_frame_size(&self) -> usize {
        (self.bytes_per_pixel() as u32 * self.image_width * self.image_height) as usize
    }

    /// Number of bytes per pixel (either 1 or 2)
    pub fn bytes_per_pixel(&self) -> usize {
        if self.pixel_depth_per_plane > 8 {
            2
        } else {
            1
        }
    }
}

impl SerFile {
    /// Open a SER file
    pub fn open(filename: &str) -> Result<Self> {
        let file = File::open(&filename)?;
        let metadata = fs::metadata(&filename)?;
        let len = metadata.len() as usize;
        if len < HEADER_SIZE {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "file shorter than header length of 178 bytes",
            ));
        }

        let mmap = unsafe { MmapOptions::new().map(&file)? };

        let header_bytes = &mmap[0..HEADER_SIZE];

        let magic = parse_string(&header_bytes[0..14]);
        if magic != MAGIC {
            return Err(Error::new(ErrorKind::InvalidData, "bad header"));
        }

        // unused
        let _lu_id = parse_u32(&header_bytes[14..18]);

        let bayer = parse_u32(&header_bytes[18..22]);

        let bayer = match bayer {
            0 => Bayer::Mono,
            8 => Bayer::RGGB,
            9 => Bayer::GRBG,
            10 => Bayer::GBRG,
            11 => Bayer::BGGR,
            16 => Bayer::CYYM,
            17 => Bayer::YCMY,
            18 => Bayer::YMCY,
            19 => Bayer::MYYC,
            100 => Bayer::RGB,
            101 => Bayer::BGR,
            _ => Bayer::Unknown(bayer),
        };

        let endianness = match parse_u32(&header_bytes[22..26]) {
            0 => Endianness::LittleEndian,
            _ => Endianness::BigEndian,
        };

        let image_width = parse_u32(&header_bytes[26..30]);
        let image_height = parse_u32(&header_bytes[30..34]);
        let pixel_depth_per_plane = parse_u32(&header_bytes[34..38]);
        let frame_count = parse_u32(&header_bytes[38..42]) as usize;
        let observer = parse_string(&header_bytes[42..82]);
        let instrument = parse_string(&header_bytes[82..122]);
        let telescope = parse_string(&header_bytes[122..162]);
        let date_time = parse_u64(&header_bytes[162..170]);
        let date_time_utc = parse_u64(&header_bytes[170..HEADER_SIZE]);

        let header = SerHeader {
            image_height,
            image_width,
            frame_count,
            pixel_depth_per_plane,
            endianness,
            bayer,
            observer,
            telescope,
            instrument,
            date_time,
            date_time_utc,
        };

        if len < HEADER_SIZE + header.image_data_bytes() {
            // TODO could add an option to be able to read valid frames that were
            // saved in the case of the file being truncated
            return Err(Error::new(
                ErrorKind::InvalidData,
                "not enough bytes for images",
            ));
        }

        // read optional trailer with timestamp per frame
        let trailer_offset = HEADER_SIZE + header.image_data_bytes() as usize;
        let trailer_size = 8_usize * frame_count as usize;
        let timestamps: Vec<u64> = if len >= trailer_offset + trailer_size {
            let trailer = &mmap[trailer_offset..trailer_offset + trailer_size];
            (0..frame_count as usize)
                .map(|i| parse_u64(&trailer[i..i + 8]))
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        Ok(Self {
            mmap,
            header,
            timestamps,
        })
    }

    /// Read the frame at the given offset
    pub fn read_frame(&self, i: usize) -> Result<&[u8]> {
        if i < self.header.frame_count as usize {
            let offset = HEADER_SIZE + i * self.header.image_frame_size();
            Ok(&self.mmap[offset..offset + self.header.image_frame_size()])
        } else {
            Err(Error::new(ErrorKind::InvalidData, "invalid frame index"))
        }
    }
}

pub struct SerWriter<'a> {
    header: &'a SerHeader,
    w: &'a mut dyn Write,
}

impl<'a> SerWriter<'a> {
    pub fn new(w: &'a mut dyn Write, header: &'a SerHeader) -> Result<Self> {
        let mut header_bytes: Vec<u8> = Vec::with_capacity(HEADER_SIZE);
        header_bytes.append(&mut MAGIC.as_bytes().to_vec());
        header_bytes.write_u32::<LittleEndian>(0)?; // lu_id unused
        let bayer_n: u32 = match header.bayer {
            Bayer::Mono => 0,
            Bayer::RGGB => 8,
            Bayer::GRBG => 9,
            Bayer::GBRG => 10,
            Bayer::BGGR => 11,
            Bayer::CYYM => 16,
            Bayer::YCMY => 17,
            Bayer::YMCY => 18,
            Bayer::MYYC => 19,
            Bayer::RGB => 100,
            Bayer::BGR => 101,
            Bayer::Unknown(bayer) => bayer,
        };
        header_bytes.write_u32::<LittleEndian>(bayer_n)?;
        header_bytes.write_u32::<LittleEndian>(match header.endianness {
            Endianness::LittleEndian => 0,
            Endianness::BigEndian => 1,
        })?;
        header_bytes.write_u32::<LittleEndian>(header.image_width)?;
        header_bytes.write_u32::<LittleEndian>(header.image_height)?;
        header_bytes.write_u32::<LittleEndian>(header.pixel_depth_per_plane)?;
        header_bytes.write_u32::<LittleEndian>(header.frame_count as u32)?;

        //TODO check length of strings here and pad/truncate as required
        header_bytes.write_all(header.observer.as_bytes())?;
        header_bytes.write_all(header.instrument.as_bytes())?;
        header_bytes.write_all(header.telescope.as_bytes())?;

        header_bytes.write_u64::<LittleEndian>(header.date_time)?;
        header_bytes.write_u64::<LittleEndian>(header.date_time_utc)?;

        assert!(header_bytes.len() == HEADER_SIZE);

        w.write_all(&header_bytes)?;

        Ok(Self { header, w })
    }

    pub fn write_frame(&mut self, frame: &[u8]) -> Result<()> {
        if self.header.image_frame_size() == frame.len() {
            self.w.write_all(frame)
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Cannot write image with {} bytes when header specifies image size as {} bytes",
                    frame.len(),
                    self.header.image_frame_size()
                ),
            ))
        }
    }

    pub fn write_timestamps(&mut self, timestamps: &[u64]) -> Result<()> {
        let mut header_bytes = Vec::with_capacity(4 * timestamps.len());
        for ts in timestamps {
            header_bytes.write_u64::<LittleEndian>(*ts)?;
        }
        self.w.write_all(&header_bytes)
    }
}

#[derive(Debug)]
pub enum Bayer {
    Mono,
    RGGB,
    GRBG,
    GBRG,
    BGGR,
    CYYM,
    YCMY,
    YMCY,
    MYYC,
    RGB,
    BGR,
    Unknown(u32),
}

#[derive(Debug)]
pub enum Endianness {
    LittleEndian,
    BigEndian,
}

/// Parse a little-endian u32
fn parse_u32(buf: &[u8]) -> u32 {
    let mut buf = buf;
    buf.read_u32::<LittleEndian>().unwrap()
}

/// Parse a little-endian u64
fn parse_u64(buf: &[u8]) -> u64 {
    let mut buf = buf;
    buf.read_u64::<LittleEndian>().unwrap()
}

/// Parse a string
fn parse_string(x: &[u8]) -> String {
    str::from_utf8(x).unwrap_or("").to_string()
}
