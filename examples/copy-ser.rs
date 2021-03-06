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

use ser_io::{SerFile, SerWriter};
use std::fs::File;
use std::io::Result;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Opt {
    input: String,
    output: String,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let ser = SerFile::open(&opt.input).unwrap();
    let mut f = File::create(&opt.output)?;
    let mut w = SerWriter::new(&mut f, &ser.header)?;
    for i in 0..ser.header.frame_count {
        let frame = ser.read_frame(i)?;
        w.write_frame(frame)?;
    }
    w.write_timestamps(&ser.timestamps)
}
