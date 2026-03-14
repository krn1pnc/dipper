use std::io::{self, BufRead, BufReader, Read};

pub(crate) struct BitReader<R: Read> {
    inner: BufReader<R>,
    bit_pos: usize,
}

impl<R: Read> BitReader<R> {
    fn peek_u64_le(&mut self, x: &mut u64) -> io::Result<usize> {
        let buf = self.inner.peek(8)?;
        if let Ok(buf) = buf.try_into() {
            *x = u64::from_le_bytes(buf);
            return Ok(64);
        } else {
            *x = 0;
            for i in 0..buf.len() {
                *x |= (buf[i] as u64) << (i * 8);
            }
            return Ok(buf.len() * 8);
        }
    }

    pub fn seek_next_byte(&mut self) {
        if self.bit_pos != 0 {
            self.inner.consume(1);
            self.bit_pos = 0;
        }
    }

    // Creates a new BitReader.
    // `r`: inner byte stream. buffer size must be greater than 8 bytes.
    pub fn new(r: BufReader<R>) -> Self {
        assert!(r.capacity() >= 8);

        return BitReader {
            inner: r,
            bit_pos: 0,
        };
    }

    pub fn peek_bits(&mut self, n: usize) -> io::Result<u64> {
        assert!(n <= 57);

        let mut bits = 0;
        let bits_peeked = self.peek_u64_le(&mut bits)?;
        let bits_available = bits_peeked - self.bit_pos;
        if n > bits_available {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        return Ok(bits >> self.bit_pos & ((1 << n) - 1));
    }

    pub fn consume_bits(&mut self, n: usize) -> io::Result<()> {
        assert!(n <= 57);

        self.inner.consume((self.bit_pos + n) / 8);
        self.bit_pos = (self.bit_pos + n) % 8;
        return Ok(());
    }

    // Read up to 57 bits from the inner byte stream, LSB first.
    // `n`: number of bits to read, must be less than 57.
    pub fn read_bits(&mut self, n: usize) -> io::Result<u64> {
        assert!(n <= 57);

        let res = self.peek_bits(n)?;
        self.consume_bits(n)?;
        return Ok(res);
    }

    // Unwrap the `BitReader<R>`, returning the inner `BufReader<R>`.
    // Note that the last unfinshed byte, if any, is lost.
    pub fn into_inner(mut self) -> BufReader<R> {
        self.seek_next_byte();
        return self.inner;
    }

    // Fill the specified buffer. Returning error when can't.
    // Note that the last unfinshed byte, if any, is lost.
    pub fn read_align_bytes_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.seek_next_byte();
        return self.inner.read_exact(buf);
    }
}
