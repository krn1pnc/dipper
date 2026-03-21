use std::env;
use std::fs::File;
use std::io::{BufReader, Write};

use zlib::Decompressor;

fn main() {
    let argv: Vec<String> = env::args().collect();
    let path = argv.get(1).unwrap();

    let input = BufReader::new(File::open(path).unwrap());
    let mut de = Decompressor::new(input);
    let _res = de.run().unwrap();

    // let mut output = File::create("tmp.txt").unwrap();
    // output.write_all(&_res).unwrap();
}
