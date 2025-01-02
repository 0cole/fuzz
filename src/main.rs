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
    time::Instant,
};

const SEG_SIG: i32 = 11; // seg fault
const FPE_SIG: i32 = 8; // floating point exception
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
    #[arg(short, long, default_value = "images/Canon_40D.jpg")]
    path: String,

    // mutation rate
    #[arg(short, long, default_value_t = 0.01)]
    mutation_rate: f64,

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

    let crash_dir = Path::new("crashes/");
    let dos_dir = Path::new("dos");

    // create crash dir
    if fs::exists("crashes/").unwrap_or(false) {
        fs::remove_dir_all(crash_dir)?;
        fs::create_dir(crash_dir)?;
    } else {
        fs::create_dir(crash_dir)?;
    }

    // create dos dir
    if fs::exists("dos/").unwrap_or(false) {
        fs::remove_dir_all(dos_dir)?;
        fs::create_dir(dos_dir)?;
    } else {
        fs::create_dir(dos_dir)?;
    }

    // create dos dir
    Ok((rng, data))
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn apply_bitflip(rng: &mut ThreadRng, data: &mut [u8], mutation_rate: f64) {
    let mutation_num = (((data.len() as f64) - 4.0) * mutation_rate) as i64;
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

fn mutate(
    rng: &mut ThreadRng,
    data_buf: &mut [u8],
    mutation_rate: f64,
) -> io::Result<&'static str> {
    // manipulate jpg, save the method used for statistics if it does produce a crash
    let fuzz_method = if rng.gen_bool(0.5) {
        apply_bitflip(rng, data_buf, mutation_rate);
        "bitflip"
    } else {
        magic(rng, data_buf);
        "magic"
    };
    // write manipulated data to a temp mutate file
    utils::write_to_file(data_buf, "images/mutate.jpg")?;

    Ok(fuzz_method)
}

fn handle_dos(data: &[u8], index: u32, method: &str) -> io::Result<()> {
    let path = format!("dos/dos.{method}.{index}.jpg");
    utils::write_to_file(data, &path)?;
    Ok(())
}

fn handle_crash(data: &[u8], index: u32, method: &str) -> io::Result<()> {
    let path = format!("crashes/crash.{method}.{index}.jpg");
    utils::write_to_file(data, &path)?;
    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut crash_counter = 0;
    let mut dos_counter = 0;
    let mut seg_fault_crashes = 0;
    let mut floating_point_crashes = 0;
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
        let fuzz_method = mutate(&mut rng, &mut mutate_buffer, args.mutation_rate)?;

        // execute command
        let now = Instant::now();
        let output = Command::new("binaries/ok-jpg-size")
            // .args(["images/mutate.jpg"])
            .output()?;
        let elapsed_time = now.elapsed();

        // check for dos
        if elapsed_time.as_secs() > 2 {
            handle_dos(&mutate_buffer, i, fuzz_method)?;
            dos_counter += 1;
        }

        // uncomment for debug
        if i == 0 {
            println!(
                "stdout for first attempt: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }

        if let Some(signal) = output.status.signal() {
            handle_crash(&mutate_buffer, i, fuzz_method)?;

            // stats stuff
            match signal {
                SEG_SIG => seg_fault_crashes += 1,
                FPE_SIG => floating_point_crashes += 1,
                _ => println!("Unknown signal encountered: {signal}"),
            }
            match fuzz_method {
                "bitflip" => bitflip_crashes += 1,
                "magic" => magic_crashes += 1,
                _ => println!("this message should not be printed"),
            }
            crash_counter += 1;
        }
    }
    println!(
        "\rFuzzing finished
Total crashes             : {crash_counter}
Total denials of service  : {dos_counter}
Segmentation faults       : {seg_fault_crashes}
Floating point exceptions : {floating_point_crashes}
Bitflip crashes           : {bitflip_crashes}
Magic crashes             : {magic_crashes}"
    );

    // Create reports
    if args.triage {
        println!("Beginning triaging...");
        triage::triage_crashes()?;
        println!("Finished triaging... Ending program.");
    }
    Ok(())
}
