use std::io::{self, BufReader, Read};

use crate::{
    bitreader::BitReader,
    huffman::HuffmanTable,
    tables::{
        CL_CODE_LENGTH_ORDER, DISTANCE_BASE, DISTANCE_EXTRA_BITS, FIXED_DISTANCE_CODE_LENGTH,
        FIXED_LITLEN_CODE_LENGTH, LENGTH_BASE, LENGTH_EXTRA_BITS,
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
    DistanceTooFar,
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
        if u16::from_be_bytes(buf) % 31 != 0 {
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
        self.window_len = 1 << (cinfo + 8);

        let fdict = flg >> 5 & 1;
        if fdict != 0 {
            return Err(DecompressError::UnknownDictionary);
        }

        return Ok(());
    }

    fn read_dynamic_code_lengths(
        &mut self,
        code_length_table: &HuffmanTable,
        count: usize,
    ) -> Result<Vec<u8>, DecompressError> {
        let mut code_lengths = Vec::with_capacity(count);
        loop {
            let bits_to_decode = self.inner.peek_bits(16)?;
            let result = code_length_table.decode(bits_to_decode);
            self.inner.consume_bits(result.bits_used as usize)?;
            match result.symbol {
                0..=15 => code_lengths.push(result.symbol as u8),
                16 => {
                    let repeat_count = self.inner.read_bits(2)? + 3;
                    if let Some(&previous_code_length) = code_lengths.last() {
                        for _ in 0..repeat_count {
                            code_lengths.push(previous_code_length);
                        }
                    } else {
                        return Err(DecompressError::InvalidCodeLength);
                    }
                }
                17 => {
                    let repeat_count = self.inner.read_bits(3)? + 3;
                    for _ in 0..repeat_count {
                        code_lengths.push(0);
                    }
                }
                18 => {
                    let repeat_count = self.inner.read_bits(7)? + 11;
                    for _ in 0..repeat_count {
                        code_lengths.push(0);
                    }
                }
                _ => unreachable!(),
            }

            if code_lengths.len() == count {
                break;
            }

            if code_lengths.len() > count {
                return Err(DecompressError::InvalidCodeLength);
            }
        }
        return Ok(code_lengths);
    }

    fn read_compressed_block(
        &mut self,
        litlen_table: &HuffmanTable,
        dist_table: &HuffmanTable,
        res: &mut Vec<u8>,
    ) -> Result<(), DecompressError> {
        loop {
            let bits_to_decode = self.inner.peek_bits(16)?;
            let result = litlen_table.decode(bits_to_decode);
            self.inner.consume_bits(result.bits_used as usize)?;
            match result.symbol {
                0..=255 => res.push(result.symbol as u8),
                256 => break,
                257..=285 => {
                    let len_symbol = (result.symbol - 257) as usize;
                    let len = LENGTH_BASE[len_symbol]
                        + self.inner.read_bits(LENGTH_EXTRA_BITS[len_symbol])? as usize;

                    let bits_to_decode = self.inner.peek_bits(16)?;
                    let result = dist_table.decode(bits_to_decode);
                    self.inner.consume_bits(result.bits_used as usize)?;
                    let dist_symbol = result.symbol as usize;
                    if dist_symbol > 29 {
                        return Err(DecompressError::UnknownDistanceSymbol);
                    }
                    let dist = DISTANCE_BASE[dist_symbol]
                        + self.inner.read_bits(DISTANCE_EXTRA_BITS[dist_symbol])? as usize;

                    if dist > self.window_len {
                        return Err(DecompressError::DistanceTooFar);
                    }

                    let current_index = res.len();
                    res.resize(current_index + len, 0);
                    for i in 0..len {
                        res[current_index + i] = res[current_index + i - dist];
                    }
                }
                _ => return Err(DecompressError::UnknownLengthSymbol),
            }
        }
        return Ok(());
    }

    pub fn run(&mut self) -> Result<Vec<u8>, DecompressError> {
        self.read_zlib_header()?;

        let mut res = Vec::new();
        let mut fixed_litlen_table = None;
        let mut fixed_dist_table = None;
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
                    if fixed_litlen_table.is_none() {
                        fixed_litlen_table =
                            Some(HuffmanTable::try_from(&FIXED_LITLEN_CODE_LENGTH, false).unwrap());
                    }
                    if fixed_dist_table.is_none() {
                        fixed_dist_table = Some(
                            HuffmanTable::try_from(&FIXED_DISTANCE_CODE_LENGTH, true).unwrap(),
                        );
                    }

                    let litlen_table = fixed_litlen_table.unwrap();
                    let dist_table = fixed_dist_table.unwrap();

                    self.read_compressed_block(&litlen_table, &dist_table, &mut res)?;

                    fixed_litlen_table = Some(litlen_table);
                    fixed_dist_table = Some(dist_table);
                }
                0b10 => {
                    let hlit = (self.inner.read_bits(5)? + 257) as usize;
                    let hdist = (self.inner.read_bits(5)? + 1) as usize;
                    let hclen = (self.inner.read_bits(4)? + 4) as usize;

                    let mut clcl = [0; 19];
                    for i in 0..hclen {
                        clcl[CL_CODE_LENGTH_ORDER[i]] = self.inner.read_bits(3)? as u8;
                    }
                    let code_length_table = HuffmanTable::try_from(&clcl, false)?;

                    let litlen_code_lengths =
                        self.read_dynamic_code_lengths(&code_length_table, hlit)?;
                    let dist_code_table =
                        self.read_dynamic_code_lengths(&code_length_table, hdist)?;

                    let litlen_table = HuffmanTable::try_from(&litlen_code_lengths, false)?;
                    let dist_table = HuffmanTable::try_from(&dist_code_table, true)?;

                    self.read_compressed_block(&litlen_table, &dist_table, &mut res)?;
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
