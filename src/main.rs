use std::path::PathBuf;
use std::collections::HashMap;
use std::io::{prelude::*, self};
use std::fs::File;
use std::boxed::Box;

use clap::Parser;
use anyhow::{Error, Result, Context};
use sha2::{Sha256, Digest};
use once_cell::sync::Lazy;

#[macro_use]
extern crate anyhow;

const ALPHA: &[u8] = b"ybndrfg8ejkmcpqxot1uwisza345h769";
const PAD_CHAR: u8 = b'=';
const RAW_BYTES_PER_CHUNK: usize = 5; // aka 40 bits aka least common multiple of 5 bits & 8 bits
const ENCODED_BYTES_PER_CHUNK: usize = 8;

static DE_ALPHA: Lazy<HashMap<u8, u8>> = Lazy::new(|| {
    let mut decode_table = HashMap::new();
    for (index, c) in ALPHA.iter().enumerate() {
        decode_table.insert(*c, index as u8);
    }
    decode_table
});

// Left justify aka right pad
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
    for ins in s.chunks(RAW_BYTES_PER_CHUNK) {
        let padded = ljust(ins, RAW_BYTES_PER_CHUNK, 0u8);

        // Basically little endian interpretation of 8 data bytes (LSB at padded[0])
        let mut val: u64 = 0;
        for c in padded.iter().rev() {
            val <<= 8;
            val |= *c as u64;
        }

        let pad_start = match ins.len() {
            // Note that this is basically ceil(len * 8 / 5)
            // d = data, x = padding
            1 => 2, // ddxxxxxx
            2 => 4, // ddddxxxx
            3 => 5, // dddddxxx
            4 => 7, // dddddddx
            5 => 8, // dddddddd
            _ => unreachable!()
        };

        for _ in 0..pad_start {
            out.push(ALPHA[(val & 0x1f) as usize]);
            val >>= 5;
        }
        // new_len = basically ceil(len / 8) * 8
        out.resize(out.len().div_ceil(ENCODED_BYTES_PER_CHUNK) * ENCODED_BYTES_PER_CHUNK, PAD_CHAR);
    }

    out
}

#[test]
fn test_raw_encode() {
    let result = raw_encode(b"0123456789abcdefghijklmnopqrstuvwxyz");
    assert_eq!(result, "ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4d======".as_bytes());
}

fn strip_padding(s: &[u8]) -> Result<(Vec<u8>, usize)> {
    if s.len() < ENCODED_BYTES_PER_CHUNK {
        // Nothing to do, return as-is (after determining appropriate raw count)
        let raw_count = match s.len() {
            // Note these are ceil(len * 5 / 8)
            // So lengths like 4 encoded will yield 3 bytes/24 bits (20 bits in actuality)
            7 => 5,
            6 => 4,
            5 => 4,
            4 => 3,
            3 => 2,
            2 => 2,
            1 => 1,
            _ => bail!("invalid chunk length")
        };
        return Ok((s.to_vec(), raw_count));
    }

    assert_eq!(s.len(), ENCODED_BYTES_PER_CHUNK);

    // We really only expect padding on the final chunk and we can't return partial bytes.
    // So lengths will be shorter than above (and more closely mirror raw_encode).
    let (raw_count, enc_count) = match s.iter().rposition(|&c| c != PAD_CHAR) {
        // Basically the inverse of pad_start in raw_encode.
        // But note we only accept a few values.
        Some(7) => (5, 8),
        Some(6) => (4, 7),
        Some(4) => (3, 5),
        Some(3) => (2, 4),
        Some(1) => (1, 2),
        _ => bail!("invalid padding")
    };

    let mut out = Vec::new();
    out.extend(&s[..enc_count]);

    Ok((out, raw_count))
}

#[test]
fn test_strip_padding() {
    // Max chunk size, padded, valid lengths
    let result = strip_padding(b"yyyyyyyy").unwrap();
    assert_eq!(result, (b"yyyyyyyy".to_vec(), 5));

    let result = strip_padding(b"yyyyyyy=").unwrap();
    assert_eq!(result, (b"yyyyyyy".to_vec(), 4));

    let result = strip_padding(b"yyyyy===").unwrap();
    assert_eq!(result, (b"yyyyy".to_vec(), 3));

    let result = strip_padding(b"yyyy====").unwrap();
    assert_eq!(result, (b"yyyy".to_vec(), 2));

    let result = strip_padding(b"yy======").unwrap();
    assert_eq!(result, (b"yy".to_vec(), 1));

    // Max chunk size, invalid lengths (expect error)
    strip_padding(b"yyyyyy==").unwrap_err();
    strip_padding(b"yyy=====").unwrap_err();
    strip_padding(b"y=======").unwrap_err();

    fn raw_size(s: &[u8]) -> usize {
        let length = s.len() as f32 * 5.0 / 8.0;
        length.ceil() as usize
    }

    // Less than max chunk size (excess bits spill over to an additional decoded byte)
    let input = b"yyyyyyy";
    let result = strip_padding(input).unwrap();
    assert_eq!(result, (input.to_vec(), raw_size(input)));

    let input = b"yyyyyy";
    let result = strip_padding(input).unwrap();
    assert_eq!(result, (input.to_vec(), raw_size(input)));

    let input = b"yyyyy";
    let result = strip_padding(input).unwrap();
    assert_eq!(result, (input.to_vec(), raw_size(input)));

    let input = b"yyyy";
    let result = strip_padding(input).unwrap();
    assert_eq!(result, (input.to_vec(), raw_size(input)));

    let input = b"yyy";
    let result = strip_padding(input).unwrap();
    assert_eq!(result, (input.to_vec(), raw_size(input)));

    let input = b"yy";
    let result = strip_padding(input).unwrap();
    assert_eq!(result, (input.to_vec(), raw_size(input)));

    let input = b"y";
    let result = strip_padding(input).unwrap();
    assert_eq!(result, (input.to_vec(), raw_size(input)));
}

fn raw_decode(s: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for ins in s.chunks(ENCODED_BYTES_PER_CHUNK) {
        // Strip pad characters, if any, keeping track of the number of raw bytes
        let (stripped, raw_count) = strip_padding(ins)?;

        // Then zero pad using the encoding for 0 from the alphabet
        let padded = ljust(&stripped[..], ENCODED_BYTES_PER_CHUNK, ALPHA[0]);

        let mut val: u64 = 0;
        for c in padded.iter().rev() {
            let decoded = match DE_ALPHA.get(c) {
                Some(d) => *d,
                None => bail!("invalid character '{}'", String::from_utf8_lossy(&[*c])) // Wew!
            };
            val <<= 5;
            val |= decoded as u64;
        }

        for _ in 0..raw_count {
            out.push((val & 0xff) as u8);
            val >>= 8;
        }
    }

    Ok(out)
}

#[test]
fn test_raw_decode() {
    let result = raw_decode(b"ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4d======");
    assert_eq!(result.unwrap(), "0123456789abcdefghijklmnopqrstuvwxyz".as_bytes());
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
    let mut buf = Vec::with_capacity(3);
    for _ in 0..3 { // NB Only encode 24 bits
        buf.push((crc & 0xff) as u8);
        crc >>= 8;
    }
    buf
}

fn encode(data: &[u8], width: usize) -> Vec<Vec<u8>> {
    assert_eq!(width % ENCODED_BYTES_PER_CHUNK, 0);

    let raw_width = width * RAW_BYTES_PER_CHUNK / ENCODED_BYTES_PER_CHUNK;
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
    assert_eq!(*result, "ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4d======hkxj".as_bytes());
}

fn decode_crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for val in data.iter().take(3).rev() {
        crc <<= 8;
        crc |= *val as u32;
    }
    crc
}

fn decode(lines: Vec<Vec<u8>>) -> Result<Vec<u8>> {
    let mut line_number = 0;
    let mut crc = 0u32;
    let mut out = Vec::new();

    for raw_line in lines {
        line_number += 1;

        if raw_line.len() < (ENCODED_BYTES_PER_CHUNK + ENCODED_CRC_LEN) {
            bail!("line too short at line {}", line_number);
        }

        let (line, enc_crc) = raw_line.split_at(raw_line.len() - ENCODED_CRC_LEN);

        if line.len() % ENCODED_BYTES_PER_CHUNK != 0 {
            bail!("invalid line length ({}) at line {}", raw_line.len(), line_number);
        }

        let decoded_line = raw_decode(line)
            .with_context(|| format!("decode error at line {line_number}"))?;
        let decoded_crc = raw_decode(enc_crc)
            .with_context(|| format!("decode error at line {line_number}"))?;

        crc = crc_update(&decoded_line[..], crc);

        let dec_crc = decode_crc(&decoded_crc[..]);
        if crc != dec_crc {
            bail!("CRC error at line {}", line_number);
        }

        out.extend(&decoded_line);

        if decoded_line.len() < RAW_BYTES_PER_CHUNK { break; }
    }

    Ok(out)
}

#[test]
fn test_decode() {
    let input = vec!["ojcru3ogitpqdhr8buagg1icg53oswjpmd54gz7pomhrz3tqiu7q8hfx4d======hkxj".as_bytes().to_vec()];
    let result = decode(input).unwrap();
    assert_eq!(result, "0123456789abcdefghijklmnopqrstuvwxyz".as_bytes());
}

#[test]
fn test_encode_decode() {
    let text = b"Hello there.\nGeneral Kenobi..\nYou are a bold one.\n";
    assert_eq!(text.len(), 50); // Otherwise our assumptions break

    for text_len in 0..text.len() {
        let raw_text = &text[..text_len+1];

        let encoded = encode(raw_text, 80); // width is 50 * 8 / 5, so the text at full length will be encoded in 80 bytes
        assert_eq!(encoded.len(), 1);

        // println!("{} -> {}", String::from_utf8_lossy(raw_text), String::from_utf8_lossy(&encoded[0][..]));

        let decoded = decode(encoded).unwrap();
        assert_eq!(decoded, raw_text);
    }
}

fn create_output(filename: &Option<PathBuf>) -> Result<Box<dyn Write>> {
    match filename {
        Some(filename) => Ok(Box::new(io::BufWriter::new(File::create(filename)?))),
        None => Ok(Box::new(io::stdout()))
    }
}

fn encode_main<R>(mut ifile: R, output: &Option<PathBuf>) -> Result<()>
    where R: Read
{
    // TODO Do this better
    let mut buf = Vec::new();
    let length = ifile.read_to_end(&mut buf)?;

    let encoded_lines = encode(&buf[..], 80);

    let mut ofile = create_output(output)?;
    for line in encoded_lines {
        ofile.write_all(&line[..])?;
        ofile.write_all(b"\n")?;
    }

    let mut hasher = Sha256::default();
    hasher.update(&buf[..]);
    let hash = hasher.finalize();

    writeln!(ofile, "# length: {length}")?;
    writeln!(ofile, "# sha256: {hash:x}")?;
    writeln!(ofile, "# alphabet: {}, CRC-20 poly: 0x1c4047, check: 0xa5448", String::from_utf8_lossy(ALPHA))?;

    // Also write out to stderr
    eprintln!("# length: {length}");
    eprintln!("# sha256: {hash:x}");

    Ok(())
}

fn decode_main<R>(ifile: R, output: &Option<PathBuf>) -> Result<()>
    where R: BufRead
{
    let mut lines = Vec::new();
    for line in ifile.lines() {
        let line = match line {
            Ok(line) => line.trim().as_bytes().to_vec(),
            Err(err) => return Err(Error::new(err))
        };

        if line.is_empty() || line.starts_with(b"#") { continue; }

        lines.push(line);
    }

    let decoded = decode(lines)?;

    let mut hasher = Sha256::default();
    hasher.update(&decoded);

    let mut ofile = create_output(output)?;
    ofile.write_all(&decoded[..])?;

    eprintln!("# length: {}", decoded.len());
    eprintln!("# sha256: {:x}", hasher.finalize());

    Ok(())
}

#[derive(Parser, Debug)]
struct Opt {

    /// Decode input
    #[clap(short, long)]
    decode: bool,

    /// Output file. If not specified, will write to stdout.
    #[clap(short, long)]
    output: Option<PathBuf>,

    /// Input file. If not specified, will read from stdin.
    input: Option<PathBuf>,

}

fn main() -> Result<()> {
    let opt = Opt::parse();

    let ifile: Box<dyn BufRead> = match opt.input {
        Some(filename) => Box::new(io::BufReader::new(File::open(filename)?)),
        None => Box::new(io::BufReader::new(io::stdin()))
    };

    if opt.decode {
        decode_main(ifile, &opt.output)
    } else {
        encode_main(ifile, &opt.output)
    }
}
