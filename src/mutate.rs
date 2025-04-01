use super::utils;
use rand::{rngs::ThreadRng, Rng};
use std::io;

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
fn bitflip(rng: &mut ThreadRng, data: &mut [u8], mutation_rate: f64) {
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

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn insertion(rng: &mut ThreadRng, data: &mut Vec<u8>, insertion_rate: f64) {
    let insertion_num = (((data.len() as f64) - 4.0) * insertion_rate) as i64;
    let mut indicies = vec![];

    // collect indicies to insert fake bytes at. do so in a bottom-up approach
    for _ in 4..insertion_num {
        let chosen_index = rng.gen_range(4..(data.len() - 4));
        indicies.push(chosen_index);
    }
    indicies.sort_unstable();
    indicies.reverse();
    for index in indicies {
        // i think it would be a good idea to start from 0x00 and go to 0xff
        let insertion_byte: u8 = rng.gen_range(0..255);
        data.insert(index, insertion_byte);
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn deletion(rng: &mut ThreadRng, data: &mut Vec<u8>, deletion_rate: f64) {
    let deletion_num = (((data.len() as f64) - 4.0) * deletion_rate) as usize;
    let mut indicies = vec![];

    for _ in 4..deletion_num {
        // in the super unlikely case where every index occurs consecutively at the end,
        // this function will panic, my immediate solution is to generate an index up until
        // the last 'deletion_num' indicies
        let chosen_index = rng.gen_range(4..data.len() - deletion_num);
        indicies.push(chosen_index);
    }
    indicies.sort_unstable();
    indicies.reverse();

    for index in indicies {
        data.remove(index);
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

#[allow(clippy::module_name_repetitions)]
pub fn mutate_input(
    rng: &mut ThreadRng,
    data_buf: &mut Vec<u8>,
    mutation_rate: f64,
) -> io::Result<&'static str> {
    let fuzz_method = rng.gen_range(0..4);
    let method_name = match fuzz_method {
        0 => {
            bitflip(rng, data_buf, mutation_rate);
            "bitflip"
        }
        1 => {
            insertion(rng, data_buf, mutation_rate);
            "insertion"
        }
        2 => {
            deletion(rng, data_buf, mutation_rate);
            "deletion"
        }
        3 => {
            magic(rng, data_buf);
            "magic"
        }
        _ => unreachable!(),
    };

    // write manipulated data to a temp mutate file
    utils::write_to_file(data_buf, "images/mutate.jpg")?;

    Ok(method_name)
}
