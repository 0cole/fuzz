mod triage;
mod utils;

use clap::Parser;
use rand::{rngs::ThreadRng, Rng};
use std::{
    fs,
    io::{self, Write},
    os::unix::process::ExitStatusExt,
    path::Path,
    process::Command,
};

const SEG_SIG: i32 = 11;
const MAGIC_NUMBERS: [(usize, u8); 11] = [
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // image path
    #[arg(short, long)]
    path: String,

    // number of attempts
    #[arg(short, long, default_value_t = 10_000)]
    attempts: u32,

    // triage crashes
    #[arg(short, long)]
    triage: bool,
}

fn initialize(path: &str) -> io::Result<(ThreadRng, Vec<u8>)> {
    // init rng and get file contents once
    let rng: ThreadRng = rand::thread_rng();
    let mut data: Vec<u8> = vec![];
    utils::get_file_contents(&mut data, path)?;
    Ok((rng, data))
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn apply_bitflip(rng: &mut ThreadRng, data: &mut [u8]) {
    let mutation_num = (((data.len() as f64) - 4.0) * 0.01) as i64;
    let mut indices = vec![];

    // collect indices
    for _ in 4..mutation_num {
        let chosen_index = rng.gen_range(4..(data.len() - 4));
        indices.push(chosen_index);
    }

    for index in indices {
        let mut bits = utils::to_bits(data[index]);
        let rand_index = rng.gen_range(0..8);
        bits[rand_index] ^= 1;
        data[index] = utils::to_u8(&bits);
    }
}

fn magic(rng: &mut ThreadRng, data: &mut [u8]) {
    let len = data.len() - 8;
    let chosen_index = rng.gen_range(0..len);

    let choice = rng.gen_range(0..MAGIC_NUMBERS.len());
    let (num_magic_bytes, value) = MAGIC_NUMBERS[choice];

    let mut counter = 0;
    while counter < num_magic_bytes {
        data[chosen_index + counter] = value;
        counter += 1;
    }
}

fn mutate(rng: &mut ThreadRng, data_buf: &mut [u8]) -> io::Result<&'static str> {
    // manipulate jpg, save the method used for statistics if it does produce a crash
    let fuzz_method = if rng.gen_bool(0.5) {
        apply_bitflip(rng, data_buf);
        "bitflip"
    } else {
        magic(rng, data_buf);
        "magic"
    };
    // write mainpulated data to a temp mutate file
    utils::write_to_file(data_buf, "images/mutate.jpg")?;

    Ok(fuzz_method)
}

fn handle_crash(data: &[u8], index: u32, method: &str) -> io::Result<()> {
    if !fs::exists("crashes/").unwrap_or(false) {
        fs::create_dir(Path::new("crashes/"))?;
    }
    let path = format!("crashes/crash.{method}.{index}.jpg");
    utils::write_to_file(data, &path)?;

    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut total_crashes = 0;
    let mut bitflip_crashes = 0;
    let mut magic_crashes = 0;

    // init rng and data from input image
    let (mut rng, data) = initialize(&args.path)?;

    // this is a mutable buffer that will be reset after every iteration
    let mut mutate_buffer = vec![0u8; data.len()];

    for i in 0..args.attempts {
        // update status
        if i % 100 == 0 {
            print!("\rAttempt: {i}");
            io::stdout().flush()?;
        }

        // reset buffer and mutate it slightly once again
        mutate_buffer.copy_from_slice(&data);
        let fuzz_method = mutate(&mut rng, &mut mutate_buffer)?;

        // execute command
        let output = Command::new("./binaries/exif")
            .args(["images/mutate.jpg"])
            .output()?;
        let output_signal = output.status.signal().unwrap_or(0);

        // if signal == 11, a crash occurred
        if output_signal == SEG_SIG {
            handle_crash(&mutate_buffer, i, fuzz_method)?;

            if fuzz_method == "bitflip" {
                bitflip_crashes += 1;
            } else {
                magic_crashes += 1;
            }
            total_crashes += 1;
        }
    }
    println!(
        "\rFuzzing finished. Total crashes: {total_crashes}.\nBitflip crashes: {bitflip_crashes}\nMagic crashes: {magic_crashes}"
    );

    // Create reports
    if args.triage {
        println!("Beginning triaging...");
        triage::triage_crashes()?;
        println!("Finished triaging... Ending program.");
    }
    Ok(())
}
