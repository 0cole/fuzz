use regex::Regex;
use std::{fs, io, path::Path, process::Command};

use crate::utils::{self};

fn parse_output(output: &str) -> Option<(String, String, String)> {
    let crash_condition_re = Regex::new(r"AddressSanitizer: (\S+) on").unwrap();
    let address_re = Regex::new(r"on address (\S+)").unwrap();
    let io_type_re = Regex::new(r"(READ|WRITE)").unwrap();

    // find crash condition
    let crash_condition = crash_condition_re
        .captures(output)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))?;

    // find address of crash
    let address = address_re
        .captures(output)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or("0x000000000000".to_string());

    // determine io type
    let io_type = io_type_re
        .captures(output)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))?;

    Some((crash_condition, address, io_type))
}

fn create_report(path: &Path) -> io::Result<()> {
    let file_details = path.file_name().unwrap().to_string_lossy().to_string();
    let file_details = file_details.strip_suffix(".jpg").unwrap();

    let path_str = path.to_str().unwrap();
    let output = Command::new("./binaries/exifsan")
        .args([path_str])
        .output()?;
    let combined = format!(
        "STDOUT:\n{}\n\nSTDERR:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // If one of the three is missing, label that report as unparsable
    let mut report_path = String::new();
    if let Some((crash_condition, address, io_type)) = parse_output(&combined) {
        report_path = format!("reports/{file_details}.{crash_condition}.{address}.{io_type}");
    } else {
        report_path = format!("reports/UNPARSABLE-ERROR.{file_details}.jpg");
    };
    utils::write_to_file(combined.as_bytes(), &report_path)?;

    Ok(())
}

#[allow(clippy::module_name_repetitions)]
pub fn triage_crashes() -> io::Result<()> {
    let crash_dir_path = "./crashes/";

    if !fs::exists("reports/").unwrap_or(false) {
        fs::create_dir(Path::new("reports/"))?;
    }

    for entry in fs::read_dir(crash_dir_path)? {
        let path = entry.unwrap().path();
        create_report(&path)?;
    }

    Ok(())
}
