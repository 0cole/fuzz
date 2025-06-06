mod mutate;
mod triage;
mod utils;

use clap::Parser;
use rand::rngs::ThreadRng;
use std::{
    fs,
    io::{self, Write},
    os::unix::{fs::PermissionsExt, process::ExitStatusExt},
    path::Path,
    process::{self, Command},
    time::Instant,
};

const SEG_SIG: i32 = 11; // seg fault
const FPE_SIG: i32 = 8; // floating point exception

// change this eventually
#[derive(Debug)]
enum ImageType {
    Jpg,
    Png,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // binary path
    #[arg(short, long)]
    binary_path: String,

    // flags passed into the binary
    #[arg(short, long)]
    flags: String,

    // image path
    #[arg(short, long, default_value = "images/Canon_40D.jpg")]
    image_path: String,

    // mutation rate
    #[arg(short, long, default_value_t = 0.01)]
    mutation_rate: f64,

    // number of attempts
    #[arg(short, long, default_value_t = 10_000)]
    attempts: u32,

    // debug
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    // triage crashes
    #[arg(short, long)]
    triage: bool,
}

struct FuzzStats {
    total_crashes: i32,
    total_doses: i32,
    // signals received on error
    seg_faults: i32,
    floating_points: i32,
    // specific mutations
    bitflip_events: i32,
    insertion_events: i32,
    deletion_events: i32,
    magic_events: i32,
}

fn validate_args(args: &Args) {
    // check for valid binary path, validate it is a file and it hsa executable permissions
    let binary_path = Path::new(&args.binary_path);
    if !binary_path.is_file() || binary_path.metadata().unwrap().permissions().mode() & 0o111 == 0 {
        eprintln!(
            "Error: Binary path '{}' does not exist, is not a file, or is not excutable",
            args.binary_path
        );
        process::exit(1);
    }

    // check for valid image path
    let image_path = Path::new(&args.image_path);
    if !image_path.is_file() {
        eprintln!(
            "Error: Image path '{}' does not exist or is a directory",
            args.binary_path
        );
        process::exit(1);
    }

    // make sure 0 < mutation_rate < 1
    let mutation_rate = args.mutation_rate;
    if mutation_rate >= 1.0 {
        eprintln!("Error: Mutation rate must be a value greater than 0 but less than 1");
        process::exit(1);
    }
}

fn validate_input_type(path_string: &String) -> ImageType {
    let extension_start_pos = path_string.find('.').unwrap();
    let extension = &path_string.to_string()[extension_start_pos + 1..];

    match extension {
        "jpeg" | "jpg" => ImageType::Jpg,
        "png" => ImageType::Png,
        _ => {
            eprintln!("Error: Bad file type. This may be thrown if there are multiple periods in the image path");
            process::exit(1);
        }
    }
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

    println!("Created `crashes/` dir");

    // create dos dir
    if fs::exists("dos/").unwrap_or(false) {
        fs::remove_dir_all(dos_dir)?;
        fs::create_dir(dos_dir)?;
    } else {
        fs::create_dir(dos_dir)?;
    }

    println!("Created `dos/` dir");

    Ok((rng, data))
}

fn handle_dos(data: &[u8], index: u32, method: &str, process_time: u128) -> io::Result<()> {
    let path = format!("dos/dos.{process_time}.micros.{method}.{index}.jpg");
    utils::write_to_file(data, &path)?;
    println!("\rCreated an entry in `dos/`: {path}");
    Ok(())
}

fn handle_crash(data: &[u8], index: u32, method: &str) -> io::Result<()> {
    let path = format!("crashes/crash.{method}.{index}.jpg");
    utils::write_to_file(data, &path)?;
    println!("\rCreated an entry in `crashes/`: {path}");
    Ok(())
}

fn print_progress(stats: &FuzzStats, current_attempt: u32, total_attempts: u32) -> io::Result<()> {
    print!(
        "\rAttempt: {current_attempt}/{}     Total Hits:{}",
        total_attempts,
        stats.total_crashes + stats.total_doses
    );
    io::stdout().flush()?;
    Ok(())
}

fn print_final_stats(stats: &FuzzStats) {
    println!(
        "
======= Fuzzing finished =======
Total crashes               : {}
Total denials of service    : {}
Segmentation faults         : {}
Floating point exceptions   : {}

Issues caused by bitflips   : {}
Issues caused by insertions : {}
Issues caused by deletions  : {}
Issues caused by magic      : {}",
        stats.total_crashes,
        stats.total_doses,
        stats.seg_faults,
        stats.floating_points,
        stats.bitflip_events,
        stats.insertion_events,
        stats.deletion_events,
        stats.magic_events
    );
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut avg_time: u128 = 0;
    let mut stats = FuzzStats {
        total_crashes: 0,
        total_doses: 0,
        seg_faults: 0,
        floating_points: 0,
        bitflip_events: 0,
        insertion_events: 0,
        deletion_events: 0,
        magic_events: 0,
    };
    validate_args(&args);

    let input_type: ImageType = validate_input_type(&args.image_path);
    println!("Using input type: {input_type:?}");

    // init rng and data from input image
    let (mut rng, data) = initialize(&args.image_path)?;

    // parse the binary_flags arg into a vector
    let mut flags: Vec<&str> = vec![];
    let parts = args.flags.split(' ');
    for part in parts {
        flags.push(part);
    }

    // this is a mutable buffer that will be reset after every iteration
    // let mut mutate_buffer = vec![0u8; data.len()];

    for i in 1..args.attempts {
        // update status
        if i % 10 == 0 {
            print_progress(&stats, i, args.attempts)?;
        }

        // reset buffer and mutate it slightly once again
        let mut mutate_buffer = data.clone();
        let mut event_occurred = false; // true if crash/dos occurs
        let fuzz_method = mutate::mutate_input(&mut rng, &mut mutate_buffer, args.mutation_rate)?;

        // execute command and track runtime
        // IMPORTANT: if binary requires args, specify them here
        let start = Instant::now();
        // turn this into a run_binary function
        let output = Command::new(args.binary_path.clone()).args([""]).output()?;
        let duration = start.elapsed();

        // print binary's stdout for first attempt if debug is true
        if args.debug && i == 1 {
            println!(
                "stdout for first attempt: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }

        // update avg_time
        avg_time = if i == 1 {
            duration.as_micros()
        } else {
            (avg_time * i as u128 + duration.as_micros()) / (i + 1) as u128
        };

        // check for dos after first 100 attempts
        // TODO: maybe implement a better method than ignoring the first 100 attempts.
        // I am assuming that if 100k+ attempts occur, the likelihood of a bug occuring
        // only once within the first 100 attempts is sorta low
        if i > 100 && duration.as_micros() > avg_time * 100 {
            handle_dos(&mutate_buffer, i, fuzz_method, duration.as_micros())?;
            event_occurred = true;
            stats.total_doses += 1;
        }

        if let Some(signal) = output.status.signal() {
            handle_crash(&mutate_buffer, i, fuzz_method)?;
            // stats stuff
            match signal {
                SEG_SIG => stats.seg_faults += 1,
                FPE_SIG => stats.floating_points += 1,
                _ => println!("Unknown signal encountered: {signal}"),
            }
            event_occurred = true;
            stats.total_crashes += 1;
        }

        if event_occurred {
            match fuzz_method {
                "bitflip" => stats.bitflip_events += 1,
                "insertion" => stats.insertion_events += 1,
                "deletion" => stats.deletion_events += 1,
                "magic" => stats.magic_events += 1,
                _ => unreachable!(),
            }
        }
    }

    print_final_stats(&stats);

    // Create reports, i need to find some way to generalize this
    if args.triage {
        println!("Beginning triaging...");
        triage::triage_crashes()?;
        println!("Finished triaging... Ending program.");
    }
    Ok(())
}
