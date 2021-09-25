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
use std::io::{Error, ErrorKind, Result};
use std::str;

use byteorder::{LittleEndian, ReadBytesExt};
use memmap::{Mmap, MmapOptions};

const HEADER_SIZE: usize = 178;

const MAGIC: &str = "LUCAM-RECORDER";

/// SER file
pub struct SerFile {
    /// Memory-mapped file
    mmap: Mmap,
    /// Image height, in pixels
    pub image_height: u32,
    /// Image width, in pixels
    pub image_width: u32,
    /// Number of frames
    pub frame_count: usize,
    /// Pixel depth per plane
    pub pixel_depth_per_plane: u32,
    /// Number of butes per pixel (1 or 2)
    pub bytes_per_pixel: u8,
    /// Number of bytes per image frame
    pub image_frame_size: u32,
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
    /// Timestamp in UTC of each frame
    pub timestamps: Vec<u64>,
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

        let header = &mmap[0..HEADER_SIZE];

        let magic = parse_string(&header[0..14]);
        if magic != MAGIC {
            return Err(Error::new(ErrorKind::InvalidData, "bad header"));
        }

        // unused
        let _lu_id = parse_u32(&header[14..18]);

        let bayer = parse_u32(&header[18..22]);

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

        let endianness = match parse_u32(&header[22..26]) {
            0 => Endianness::LittleEndian,
            _ => Endianness::BigEndian,
        };

        let image_width = parse_u32(&header[26..30]);
        let image_height = parse_u32(&header[30..34]);
        let pixel_depth_per_plane = parse_u32(&header[34..38]);
        let bytes_per_pixel: u8 = if pixel_depth_per_plane > 8 { 2 } else { 1 };
        let frame_count = parse_u32(&header[38..42]) as usize;
        let image_frame_size = bytes_per_pixel as u32 * image_width * image_height;
        let image_data_bytes = image_frame_size as usize * frame_count;
        let observer = parse_string(&header[42..82]);
        let instrument = parse_string(&header[82..122]);
        let telescope = parse_string(&header[122..162]);
        let date_time = parse_u64(&header[162..170]);
        let date_time_utc = parse_u64(&header[170..HEADER_SIZE]);

        if len < HEADER_SIZE + image_data_bytes as usize {
            // TODO could add an option to be able to read valid frames that were
            // saved in the case of the file being truncated
            return Err(Error::new(
                ErrorKind::InvalidData,
                "not enough bytes for images",
            ));
        }

        // read optional trailer with timestamp per frame
        let trailer_offset = HEADER_SIZE + image_data_bytes as usize;
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
            image_height,
            image_width,
            frame_count,
            pixel_depth_per_plane,
            bytes_per_pixel,
            image_frame_size,
            endianness,
            bayer,
            observer,
            telescope,
            instrument,
            date_time,
            date_time_utc,
            timestamps,
        })
    }

    /// Read the frame at the given offset
    pub fn read_frame(&self, i: usize) -> Result<&[u8]> {
        if i < self.frame_count as usize {
            let offset = HEADER_SIZE + i * self.image_frame_size as usize;
            Ok(&self.mmap[offset..offset + self.image_frame_size as usize])
        } else {
            Err(Error::new(ErrorKind::InvalidData, "invalid frame index"))
        }
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
