mod mutate;
mod triage;
mod utils;

use clap::Parser;
use rand::rngs::ThreadRng;
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

fn handle_dos(data: &[u8], index: u32, method: &str, process_time: u128) -> io::Result<()> {
    let path = format!("dos/dos.{process_time}.ms.{method}.{index}.jpg");
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

    let mut avg_time: u128 = 100; // some small time to start out (100 ms)

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
        let fuzz_method = mutate::mutate_input(&mut rng, &mut mutate_buffer, args.mutation_rate)?;

        // execute command and track runtime
        let now = Instant::now();
        let output = Command::new("binaries/ok-jpg-binary")
            // .args(["images/mutate.jpg"])
            .output()?;
        let process_time = now.elapsed();

        // add new time to avg_time
        avg_time = (avg_time + (i as u128) + process_time.as_millis()) / (i + 1) as u128;

        // check for dos after first 100 attempts
        if i > 100 && process_time.as_millis() > avg_time * 50 {
            handle_dos(&mutate_buffer, i, fuzz_method, process_time.as_millis())?;
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
