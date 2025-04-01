# Fuzz
A tiny fuzzer written in Rust. It manipulates images to sniff out potential bugs in programs that developers may have missed.

When running, an image is mutated at a very small rate (the default value is 1%). Each iteration is then passed into a binary that the user is testing. Images are mutated thousands of times and passed into the binary repeatedly. Any interesting behavior is recorded, and the images that produced the behavior are saved for reproducibility. 

By default, 10,000 attempts are made, but well over 1,000,000 attempts can be made. Further optimizations can definitely be made to speed this up, but it works relatively quickly nonetheless. I would recommend you use a smaller image for testing (~50 kilobytes).

# Findings so far

I have tested this on a few smaller programs, and have seen some cool results. A lot of older programs do not properly verify input, and I found that by manipulating the byte which corresponds to the height/width of a jpg, I found that I can cause a denial of service for the program. 

# How to run
Compile with `cargo build --release`

To invoke, execute the binary and pass in the path to the binary you want to test as the first argument.

A sample would look like this (assuming you are in root dir for the fuzzer)

```
./target/release/fuzz --binary-path path/to/binary
```