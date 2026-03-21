use crate::decompress::DecompressError;

fn get_huffman_codes(
    lengths: &[u8],
    is_distance: bool,
    codes: &mut [u16; 288],
) -> Result<(), DecompressError> {
    assert!(lengths.len() <= 288);

    codes.fill(0);

    let mut length_count = [0; 16];
    for &length in lengths {
        length_count[length as usize] += 1;
    }

    let mut max_length = 15;
    while max_length > 0 && length_count[max_length] == 0 {
        max_length -= 1;
    }

    if is_distance {
        if max_length == 0 || (max_length == 1 && length_count[1] == 1) {
            return Ok(());
        }
    }

    let mut used_codes = 0;
    let mut next_code = [0; 16];
    length_count[0] = 0;
    for i in 1..=max_length {
        used_codes = (used_codes + length_count[i - 1]) << 1;
        next_code[i] = used_codes;
    }
    used_codes = used_codes + length_count[max_length];

    if used_codes != 1 << max_length {
        return Err(DecompressError::InvalidCodeLength);
    }

    for i in 0..lengths.len() {
        if lengths[i] != 0 {
            codes[i] = next_code[lengths[i] as usize];
            next_code[lengths[i] as usize] += 1;
        }
    }

    return Ok(());
}

fn get_packed_code(code: u16, length: u8) -> u16 {
    assert!(length != 0);
    return code.reverse_bits() >> (16 - length);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DecodeResult {
    pub symbol: u16,
    pub bits_used: u8,
}

#[derive(Debug)]
pub struct HuffmanTable {
    table: Vec<DecodeResult>,
    mask: u64,
}

impl HuffmanTable {
    pub fn new(table: Vec<DecodeResult>, mask: u64) -> Self {
        return Self { table, mask };
    }

    pub fn try_from(lengths: &[u8], is_distance: bool) -> Result<Self, DecompressError> {
        let code_count = lengths.len();

        if code_count > 288 || (!is_distance && code_count <= 1) {
            return Err(DecompressError::InvalidCodeLength);
        }

        let max_length = lengths.iter().max().cloned().unwrap_or(1);

        let mut codes = [0; 288];
        get_huffman_codes(lengths, is_distance, &mut codes)?;

        let mut table = vec![DecodeResult::default(); 1 << max_length];
        for i in 0..code_count {
            if lengths[i] == 0 {
                continue;
            }

            let packed_code = get_packed_code(codes[i], lengths[i]);
            let variable_bit_count = max_length - lengths[i];
            for variable_bits in 0..(1 << variable_bit_count) {
                let state = variable_bits << lengths[i] | packed_code;
                table[state as usize] = DecodeResult {
                    symbol: i as u16,
                    bits_used: lengths[i],
                }
            }
        }

        return Ok(Self::new(table, (1 << max_length) - 1));
    }

    pub fn decode(&self, state: u64) -> DecodeResult {
        return self.table[(state & self.mask) as usize];
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rfc1951_example1() {
        // https://datatracker.ietf.org/doc/html/rfc1951 gives the following example
        // on page 8:
        //
        //    Symbol  Code
        //    ------  ----
        //    A       10
        //    B       0
        //    C       110
        //    D       111

        let t = HuffmanTable::try_from(&[2, 1, 3, 3], false).unwrap();
        assert_eq!(
            t.decode(0b_10_000000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 0,
                bits_used: 2
            },
        );
        assert_eq!(
            t.decode(0b_0_0000000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 1,
                bits_used: 1
            }
        );
        assert_eq!(
            t.decode(0b_110_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 2,
                bits_used: 3
            },
        );
        assert_eq!(
            t.decode(0b_111_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 3,
                bits_used: 3
            },
        );
    }

    #[test]
    fn test_rfc1951_example2() {
        // https://datatracker.ietf.org/doc/html/rfc1951 gives the following example
        // on page 9:
        //
        //    Symbol Length   Code
        //    ------ ------   ----
        //    A       3        010
        //    B       3        011
        //    C       3        100
        //    D       3        101
        //    E       3        110
        //    F       2         00
        //    G       4       1110
        //    H       4       1111

        let t = HuffmanTable::try_from(&[3, 3, 3, 3, 3, 2, 4, 4], false).unwrap();
        assert_eq!(
            t.decode(0b_010_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 0,
                bits_used: 3
            },
        );
        assert_eq!(
            t.decode(0b_011_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 1,
                bits_used: 3
            }
        );
        assert_eq!(
            t.decode(0b_100_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 2,
                bits_used: 3
            },
        );
        assert_eq!(
            t.decode(0b_101_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 3,
                bits_used: 3
            },
        );
        assert_eq!(
            t.decode(0b_110_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 4,
                bits_used: 3
            },
        );
        assert_eq!(
            t.decode(0b_00_000000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 5,
                bits_used: 2
            }
        );
        assert_eq!(
            t.decode(0b_1110_0000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 6,
                bits_used: 4
            },
        );
        assert_eq!(
            t.decode(0b_1111_0000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 7,
                bits_used: 4
            },
        );
    }

    #[test]
    fn test_full_length() {
        // Copied from image-rs/fdeflate:
        //
        //    Symbol Length                 Code
        //    ------ ------   ------------------
        //    0       1                        0
        //    1       2                       10
        //    2       3                      110
        //    3       4                     1110
        //    4       5                   1_1110
        //    5       6                  11_1110
        //    6       7                 111_1110
        //    7       8                1111_1110
        //    8       9              1_1111_1110
        //    9       10            11_1111_1110
        //    10      11           111_1111_1110
        //    11      12          1111_1111_1110
        //    12      13        1_1111_1111_1110
        //    13      14       11_1111_1111_1110
        //    14      15      111_1111_1111_1110
        //    15      15      111_1111_1111_1111

        let t = HuffmanTable::try_from(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15],
            false,
        )
        .unwrap();
        assert_eq!(
            t.decode(0b_0_0000000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 0,
                bits_used: 1
            },
        );
        assert_eq!(
            t.decode(0b_10_000000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 1,
                bits_used: 2
            },
        );
        assert_eq!(
            t.decode(0b_110_00000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 2,
                bits_used: 3
            },
        );
        assert_eq!(
            t.decode(0b_1110_0000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 3,
                bits_used: 4
            },
        );
        assert_eq!(
            t.decode(0b_11110_000_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 4,
                bits_used: 5
            },
        );
        assert_eq!(
            t.decode(0b_111110_00_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 5,
                bits_used: 6
            },
        );
        assert_eq!(
            t.decode(0b_1111110_0_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 6,
                bits_used: 7
            },
        );
        assert_eq!(
            t.decode(0b_11111110_u8.reverse_bits() as u64),
            DecodeResult {
                symbol: 7,
                bits_used: 8
            },
        );

        // Symbols 8-15: codes exceed 8 bits, use u16 directly
        assert_eq!(
            t.decode(0b_1_1111_1110_0000000_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 8,
                bits_used: 9
            },
        );
        assert_eq!(
            t.decode(0b_11_1111_1110_000000_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 9,
                bits_used: 10
            },
        );
        assert_eq!(
            t.decode(0b_111_1111_1110_00000_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 10,
                bits_used: 11
            },
        );
        assert_eq!(
            t.decode(0b_1111_1111_1110_0000_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 11,
                bits_used: 12
            },
        );
        assert_eq!(
            t.decode(0b_1_1111_1111_1110_000_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 12,
                bits_used: 13
            },
        );
        assert_eq!(
            t.decode(0b_11_1111_1111_1110_00_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 13,
                bits_used: 14
            },
        );
        assert_eq!(
            t.decode(0b_111_1111_1111_1110_0_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 14,
                bits_used: 15
            },
        );
        assert_eq!(
            t.decode(0b_111_1111_1111_1111_0_u16.reverse_bits() as u64),
            DecodeResult {
                symbol: 15,
                bits_used: 15
            },
        );
    }
}
