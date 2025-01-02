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
        let mut bits = utils::to_bits(data[index]);
        let rand_index = rng.gen_range(0..8);
        bits[rand_index] ^= 1;
        data[index] = utils::to_u8(&bits);
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

    // init rng and get file contents once
    let mut rng: ThreadRng = rand::thread_rng();
    let mut data: Vec<u8> = vec![];
    utils::get_file_contents(&mut data, &args.path)?;

    for i in 0..args.attempts {
        // update status
        if i % 100 == 0 {
            print!("\rAttempt: {i}");
            io::stdout().flush()?;
        }

        let mut data_clone = data.clone();
        let mut crash_method = "";

        // manipulate jpg
        if rng.gen_bool(0.5) {
            bitflip_data(&mut rng, &mut data_clone);
            crash_method = "bitflip";
        } else {
            magic(&mut rng, &mut data_clone);
            crash_method = "magic";
        }
        // write mainpulated data to a temp mutate file
        utils::write_to_file(&data_clone, "images/mutate.jpg")?;

        // execute command
        let output = Command::new("./binaries/exif")
            .args(["images/mutate.jpg"])
            .output()?;
        let output_signal = output.status.signal().unwrap_or(0);

        // 11 == segfault
        if output_signal == 11 {
            handle_crash(&data_clone, i, crash_method)?;

            if crash_method == "bitflip" {
                bitflip_crashes += 1;
            } else {
                magic_crashes += 1;
            }

            total_crashes += 1;
        }
    }
    println!(
        "Program finished. Total crashes: {total_crashes}.\nBitflip crashes: {bitflip_crashes}\nMagic crashes: {magic_crashes}"
    );

    // Create reports
    if args.triage {
        triage::triage_crashes()?;
    }
    Ok(())
}
