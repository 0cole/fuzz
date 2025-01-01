use clap::Parser;
use rand::{rngs::ThreadRng, Rng};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::process::ExitStatusExt,
    path::Path,
    process::Command,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // image path
    #[arg(short, long)]
    path: String,
}

fn get_file_contents(buf: &mut Vec<u8>, path: &str) -> io::Result<()> {
    let mut f = File::open(Path::new(path))?;
    f.read_to_end(buf)?;
    Ok(())
}

fn write_to_file(data: &[u8], path: &str) -> io::Result<()> {
    if fs::exists(path).unwrap() {
        fs::remove_file(path)?;
    }
    let mut f = File::create_new(path)?;
    f.write_all(data)?;
    Ok(())
}

fn to_bits(number: u8) -> Vec<u8> {
    (0..8).rev().map(|i| (number >> i) & 1).collect()
}

fn to_u8(bits: &[u8]) -> u8 {
    let mut answer: u8 = 0;
    for (index, bit) in bits.iter().enumerate() {
        if *bit == 1 {
            // answer += 2 << index;
            answer |= bit << (7 - index);
        }
    }
    answer
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn bitflip_data(rng: &mut ThreadRng, data: &mut [u8]) {
    let mutation_num = (((data.len() as f64) - 4.0) * 0.01) as i64;
    let mut indices = vec![];

    // collect indices
    for _ in 4..mutation_num {
        let chosen_index = rng.gen_range(4..(data.len() - 4));
        indices.push(chosen_index);
    }

    for index in indices {
        let mut bits = to_bits(data[index]);
        let rand_index = rng.gen_range(0..8);
        bits[rand_index] ^= 1;
        data[index] = to_u8(&bits);
    }
}

fn magic(rng: &mut ThreadRng, data: &mut [u8]) {
    let magic_numbers = [
        (1, 255),
        (1, 255),
        (1, 127),
        (1, 0),
        (2, 255),
        (2, 0),
        (4, 255),
        (4, 0),
        (4, 128),
        (4, 64),
        (4, 127),
    ];

    let len = data.len() - 8;
    let chosen_index = rng.gen_range(0..len);

    let choice = rng.gen_range(0..magic_numbers.len());
    let (num_magic_bytes, value) = magic_numbers[choice];

    let mut counter = 0;
    while counter < num_magic_bytes {
        data[chosen_index + counter] = value;
        counter += 1;
    }
}

fn handle_crash(data: &[u8], index: i32) -> io::Result<()> {
    if !fs::exists("crashes/").unwrap_or(false) {
        fs::create_dir(Path::new("crashes/"))?;
    }
    let path = format!("crashes/crash-{index}.jpg");
    write_to_file(data, &path)?;

    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let tries = 10_000;
    let mut total_crashes = 0;

    // init rng and get file contents once
    let mut rng: ThreadRng = rand::thread_rng();
    let mut data: Vec<u8> = vec![];
    get_file_contents(&mut data, &args.path)?;

    for i in 0..tries {
        let mut data_clone = data.clone();

        // manipulate jpg
        if rng.gen_bool(0.5) {
            bitflip_data(&mut rng, &mut data_clone);
        } else {
            magic(&mut rng, &mut data_clone);
        }

        write_to_file(&data_clone, "images/mutate.jpg")?;

        // execute command
        let output = Command::new("./binaries/exif")
            .args(["images/mutate.jpg"])
            .output()?;

        // 11 == segfault
        if output.status.signal().unwrap_or(0) == 11 {
            handle_crash(&data_clone, i)?;
            total_crashes += 1;
        }
    }

    println!("Program finished. Total crashes: {total_crashes}.");
    Ok(())
}
