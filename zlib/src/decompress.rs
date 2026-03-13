use std::io::{self, BufReader, Read};

use crate::bitreader::BitReader;

pub enum DecompressError {
    IO(io::Error),
    BadZlibHeader,
    UnknownCompressionMethod,
    InvalidWindowLength,
    UnknownDictionary,
    InvalidUncompressedBlockLength,
}

impl From<io::Error> for DecompressError {
    fn from(e: io::Error) -> Self {
        return DecompressError::IO(e);
    }
}

pub struct Decompressor<R: Read> {
    inner: BitReader<R>,
    window_len: usize,
}

impl<R: Read> Decompressor<R> {
    fn read_zlib_header(&mut self) -> Result<(), DecompressError> {
        let mut buf = [0; 2];
        self.inner.read_align_bytes_exact(&mut buf)?;
        if u16::from_le_bytes(buf) % 31 != 0 {
            return Err(DecompressError::BadZlibHeader);
        }

        let (cmf, flg) = (buf[0], buf[1]);

        let cm = cmf & ((1 << 4) - 1);
        let cinfo = cmf >> 4;
        if cm != 8 {
            return Err(DecompressError::UnknownCompressionMethod);
        }
        if cinfo > 7 {
            return Err(DecompressError::InvalidWindowLength);
        }
        self.window_len = 1 << cinfo;

        let fdict = flg >> 5 & 1;
        if fdict != 0 {
            return Err(DecompressError::UnknownDictionary);
        }

        return Ok(());
    }

    fn run(&mut self) -> Result<Vec<u8>, DecompressError> {
        self.read_zlib_header()?;

        let mut res = Vec::new();
        loop {
            let bfinal = self.inner.read_bits(1)?;

            let btype = self.inner.read_bits(2)?;
            match btype {
                0b00 => {
                    self.inner.seek_next_byte();
                    let len = self.inner.read_bits(16)? as u16;
                    let nlen = self.inner.read_bits(16)? as u16;
                    if nlen != !len {
                        return Err(DecompressError::InvalidUncompressedBlockLength);
                    }

                    let old_len = res.len();
                    let new_len = old_len + len as usize;
                    res.resize(new_len, 0);
                    self.inner.read_align_bytes_exact(&mut res[old_len..])?;
                }
                0b01 => {
                    todo!()
                }
                0b10 => {
                    todo!()
                }
                0b11 => {
                    todo!()
                }
                _ => unreachable!(),
            }

            if bfinal != 0 {
                break;
            }
        }

        return Ok(res);
    }

    pub fn new(r: BufReader<R>) -> Self {
        return Decompressor {
            inner: BitReader::new(r),
            window_len: 0,
        };
    }
}
