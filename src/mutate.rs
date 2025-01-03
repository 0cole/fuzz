use std::io;

use super::utils;
use rand::{rngs::ThreadRng, Rng};

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

pub fn mutate_input(
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
