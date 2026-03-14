use std::io::{self, BufReader, Read};

use crate::{
    bitreader::BitReader,
    huffman::HuffmanTable,
    tables::{
        DISTANCE_BASE, DISTANCE_EXTRA_BITS, FIXED_LITLEN_CODE_LENGTH, LENGTH_BASE,
        LENGTH_EXTRA_BITS,
    },
};

#[derive(Debug)]
pub enum DecompressError {
    IO(io::Error),
    BadZlibHeader,
    UnknownCompressionMethod,
    InvalidWindowLength,
    UnknownDictionary,
    InvalidUncompressedBlockLength,
    InvalidCodeLength,
    UnknownCompressedBlockType,
    UnknownLengthSymbol,
    UnknownDistanceSymbol,
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

    pub fn run(&mut self) -> Result<Vec<u8>, DecompressError> {
        self.read_zlib_header()?;

        let mut res = Vec::new();
        let mut fixed_litlen_huffman_table = None;
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
                    if fixed_litlen_huffman_table.is_none() {
                        fixed_litlen_huffman_table =
                            Some(HuffmanTable::try_from(&FIXED_LITLEN_CODE_LENGTH, false).unwrap());
                    }

                    let litlen_huffman_table = fixed_litlen_huffman_table.unwrap();
                    loop {
                        let bits_to_decode = self.inner.peek_bits(16)? as u16;
                        let result = litlen_huffman_table.decode(bits_to_decode);
                        self.inner.consume_bits(result.bits_used as usize)?;
                        match result.symbol {
                            0..=255 => res.push(result.symbol as u8),
                            256 => break,
                            257..=285 => {
                                let len_symbol = (result.symbol & ((1 << 8) - 1)) as usize;
                                let len = LENGTH_BASE[len_symbol]
                                    + self.inner.read_bits(LENGTH_EXTRA_BITS[len_symbol])? as usize;

                                let dist_symbol = self.inner.read_bits(5)? as usize;
                                if dist_symbol > 29 {
                                    return Err(DecompressError::UnknownDistanceSymbol);
                                }
                                let dist = DISTANCE_BASE[dist_symbol]
                                    + self.inner.read_bits(DISTANCE_EXTRA_BITS[dist_symbol])?
                                        as usize;

                                let current_index = res.len();
                                res.resize(current_index + len, 0);
                                for i in 0..len {
                                    res[current_index + i] = res[current_index + i - dist];
                                }
                            }
                            _ => return Err(DecompressError::UnknownLengthSymbol),
                        }
                    }

                    fixed_litlen_huffman_table = Some(litlen_huffman_table);
                }
                0b10 => {
                    todo!()
                }
                0b11 => return Err(DecompressError::UnknownCompressedBlockType),
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
