#![feature(bufreader_peek)]
mod bitreader;
mod decompress;
mod huffman;
mod tables;

pub use decompress::{DecompressError, Decompressor};
