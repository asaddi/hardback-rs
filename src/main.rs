use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::io::prelude::*;
use std::fs::File;
use std::io::BufReader;

use structopt::StructOpt;
use anyhow::{Error, Result, Context};
use sha2::{Sha256, Digest};

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate lazy_static;

const ALPHA: &[u8] = b"ybndrfg8ejkmcpqxot1uwisza345h769";

lazy_static! {
    static ref DE_ALPHA: HashMap<u8, u8> = {
        let mut decode_table = HashMap::new();
        for (index, c) in ALPHA.iter().enumerate() {
            decode_table.insert(*c, index as u8);
        }
        decode_table
    };
}

fn ljust(s: &[u8], size: usize, fill: u8) -> Vec<u8> {
    if s.len() >= size {
        s.to_vec()
    }
    else {
        let padding = [fill].repeat(size - s.len());
        [s.to_vec(), padding].concat()
    }
}

#[test]
fn test_ljust() {
    assert_eq!(ljust(b"abc", 3, b'a'), b"abc");
    assert_eq!(ljust(b"abc", 5, b'a'), b"abcaa");
}

fn raw_encode(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for ins in s.chunks(5) {
        let padded = ljust(ins, 5, 0u8);
        let mut val: u64 = 0;
        for c in padded.iter().rev() {
            val <<= 8;
            val |= *c as u64;
        }
        for _ in 0..8 {
            out.push(ALPHA[(val & 0x1f) as usize]);
            val >>= 5;
        }
    }
    out
}

#[test]
fn test_raw_encode() {
    let result = raw_encode(b"0123456789abcdefghijklmnopqrstuvwxyz");
    assert_eq!(result, "ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4dyyyyyy".as_bytes());
}

fn raw_decode(s: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for ins in s.chunks(8) {
        let padded = ljust(ins, 8, ALPHA[0]);
        let mut val: u64 = 0;
        for c in padded.iter().rev() {
            let decoded = match DE_ALPHA.get(c) {
                Some(d) => *d,
                None => bail!("invalid character '{}'", String::from_utf8_lossy(&[*c])) // Wew!
            };
            val <<= 5;
            val |= decoded as u64;
        }
        for _ in 0..5 {
            out.push((val & 0xff) as u8);
            val >>= 8;
        }
    }

    Ok(out)
}

#[test]
fn test_raw_decode() {
    let result = raw_decode(b"ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4dyyyyyy");
    // Note result will be divisible by 8, padded with NULs
    assert_eq!(result.unwrap(), "0123456789abcdefghijklmnopqrstuvwxyz\0\0\0\0".as_bytes());
}

// CRC-20 with poly 0x1c4047
// Detects errors (up to and including Hamming distance 6)
// in 494 bits of data. Good enough for us (400 bits).
// Much thanks to https://users.ece.cmu.edu/~koopman/crc/
fn crc_update(data: &[u8], crc: u32) -> u32 {
    let mut crc = crc;
    for c in data {
        for i in [0x80u8, 0x40u8, 0x20u8, 0x10u8, 0x08u8, 0x04u8, 0x02u8, 0x01u8].iter() {
            let mut bit = (crc & 0x80000) != 0; // The bit about to be shifted out
            if (c & i) != 0 {
                bit = !bit;
            }
            crc <<= 1;
            if bit {
                crc ^= 0xc4047;
            }
        }
    }

    crc & 0xfffff // Constrain to 20 bits
}

#[test]
fn test_crc() {
    let crc = crc_update(b"123456789", 0);
    assert_eq!(crc, 0xa5448);
}

const ENCODED_CRC_LEN: usize = 4; // 5 bits per encoded char * 4 = 20 bits

fn encode_crc(crc: u32) -> Vec<u8> {
    let mut crc = crc;
    // Force little endian
    let mut buf = Vec::with_capacity(4);
    for _ in 0..3 { // NB Only encode 24 bits
        buf.push((crc & 0xff) as u8);
        crc >>= 8;
    }
    buf
}

fn encode(data: &[u8], width: usize) -> Vec<Vec<u8>> {
    assert_eq!(width % 8, 0);
    let raw_width = width * 5 / 8;
    let mut out = Vec::new();
    let mut crc = 0u32;
    for ins in data.chunks(raw_width) {
        crc = crc_update(ins, crc);

        let mut line = Vec::with_capacity(width + ENCODED_CRC_LEN);
        line.extend(raw_encode(ins));
        line.extend(&raw_encode(&encode_crc(crc)[..])[..ENCODED_CRC_LEN]);

        out.push(line);
    }
    out
}

#[test]
fn test_encode() {
    let result = &encode(b"0123456789abcdefghijklmnopqrstuvwxyz", 80)[0];
    assert_eq!(*result, "ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4dyyyyyyhkxj".as_bytes());
}

fn decode_crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for val in data.iter().take(3).rev() {
        crc <<= 8;
        crc |= *val as u32;
    }
    crc
}

fn decode(lines: Vec<Vec<u8>>, length: usize) -> Result<Vec<u8>> {
    let mut length = length;
    let mut line_number = 0;
    let mut crc = 0u32;
    let mut out = Vec::new();

    for raw_line in lines {
        line_number += 1;

        if raw_line.len() < (8 + ENCODED_CRC_LEN) {
            bail!("line too short at line {}", line_number);
        }

        let (line, enc_crc) = raw_line.split_at(raw_line.len() - ENCODED_CRC_LEN);

        if line.len() % 8 != 0 {
            bail!("invalid line length ({}) at line {}", raw_line.len(), line_number);
        }

        let decoded_line = raw_decode(&line[..])
            .with_context(|| format!("decode error at line {}", line_number))?;
        let decoded_crc = raw_decode(enc_crc)
            .with_context(|| format!("decode error at line {}", line_number))?;

        let decoded_line = if decoded_line.len() > length {
            &decoded_line[..length]
        } else {
            &decoded_line[..]
        };

        assert!(length >= decoded_line.len()); // TODO error
        length -= decoded_line.len();

        crc = crc_update(decoded_line, crc);

        let dec_crc = decode_crc(&decoded_crc[..]);
        if crc != dec_crc {
            bail!("CRC error at line {}", line_number);
        }

        out.extend(decoded_line);

        if length == 0 { break };
    }

    Ok(out)
}

#[test]
fn test_decode() {
    let input = vec!["ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4dyyyyyyhkxj".as_bytes().to_vec()];
    let result = decode(input, 36).unwrap();
    assert_eq!(result, "0123456789abcdefghijklmnopqrstuvwxyz".as_bytes());
}

#[derive(StructOpt, Debug)]
struct Opt {

    /// Decode input
    #[structopt(short, long)]
    decode: Option<usize>,

    /// Input file
    input: PathBuf,

    /// Output file
    output: PathBuf,

}

fn encode_main(input_filename: &Path, output_filename: &Path) -> Result<()> {
    let mut ifile = File::open(input_filename)?;

    // TODO Do this better
    let mut buf = Vec::new();
    let length = ifile.read_to_end(&mut buf)?;

    let encoded_lines = encode(&buf[..], 80);

    let mut ofile = File::create(output_filename)?;
    for line in encoded_lines {
        ofile.write_all(&line[..])?;
        ofile.write_all(b"\n")?;
    }

    let mut hasher = Sha256::default();
    hasher.input(&buf[..]);
    let hash = hasher.result();

    writeln!(ofile, "# length: {}", length)?;
    writeln!(ofile, "# alphabet: {}, CRC-20 poly: 0x1c4047, check: 0xa5448", String::from_utf8_lossy(ALPHA))?;
    writeln!(ofile, "# sha256: {:x}", hash)?;

    // Also write out to stderr
    eprintln!("# length: {}", length);
    eprintln!("# sha256: {:x}", hash);

    Ok(())
}

fn decode_main(input_filename: &Path, output_filename: &Path, expected_length: usize) -> Result<()> {
    let ifile = File::open(input_filename)?;

    let mut lines = Vec::new();
    for line in BufReader::new(ifile).lines() {
        let line = match line {
            Ok(line) => line.trim().as_bytes().to_vec(),
            Err(err) => return Err(Error::new(err))
        };

        if line.is_empty() || line.starts_with(b"#") { continue; }

        lines.push(line);
    }

    let decoded = decode(lines, expected_length)?;

    if decoded.len() < expected_length {
        bail!("input not long enough");
    }

    let decoded = &decoded[..expected_length];

    let mut hasher = Sha256::default();
    hasher.input(decoded);

    let mut ofile = File::create(output_filename)?;
    ofile.write_all(decoded)?;

    eprintln!("# sha256: {:x}", hasher.result());

    Ok(())
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    match opt.decode {
        Some(length) => decode_main(&opt.input, &opt.output, length),
        None => encode_main(&opt.input, &opt.output)
    }
}
