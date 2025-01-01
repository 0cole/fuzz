use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
};

pub fn get_file_contents(buf: &mut Vec<u8>, path: &str) -> io::Result<()> {
    let mut f = File::open(Path::new(path))?;
    f.read_to_end(buf)?;
    Ok(())
}

pub fn write_to_file(data: &[u8], path: &str) -> io::Result<()> {
    if fs::exists(path).unwrap() {
        fs::remove_file(path)?;
    }
    let mut f = File::create_new(path)?;
    f.write_all(data)?;
    Ok(())
}

pub fn to_bits(number: u8) -> Vec<u8> {
    (0..8).rev().map(|i| (number >> i) & 1).collect()
}

pub fn to_u8(bits: &[u8]) -> u8 {
    let mut answer: u8 = 0;
    for (index, bit) in bits.iter().enumerate() {
        if *bit == 1 {
            // answer += 2 << index;
            answer |= bit << (7 - index);
        }
    }
    answer
}
