extern crate enthunter;

use enthunter::entropy_estimate;
use std::fs::File;
use std::io;
use std::process::exit;
use std::mem::uninitialized;
use std::cmp::Ordering;
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};

pub const SECTOR_SIZE: usize = 512;

fn greedy_read<R: io::Read>(i: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut amount_read = i.read(buf)?;
    if amount_read == 0 { Ok(0) }
    else {
        while amount_read < buf.len() {
            let result = i.read(&mut buf[amount_read..]);
            match result {
                Ok(0) | Err(_) => break,
                Ok(x) => amount_read += x,
            }
        }
        Ok(amount_read)
    }
}

fn stats(path: &str) -> i32 {
    let mut file = match File::open(path) {
        Ok(x) => x,
        Err(e) => { eprintln!("Error opening {}: {}", path, e); return 1 }
    };
    let mut buf: [u8; SECTOR_SIZE] = unsafe { uninitialized() };
    let mut entropies = Vec::new();
    loop {
        let len = match greedy_read(&mut file, &mut buf) {
            Err(e) => { eprintln!("Read error: {}", e); return 1 },
            Ok(SECTOR_SIZE) => SECTOR_SIZE,
            Ok(0) => break,
            Ok(x) => {
                eprintln!("Warning: Last read was less than one sector ({})",
                          x);
                x
            }
        };
        let entropy = entropy_estimate(&buf[..len]);
        entropies.push(entropy as f32);
    }
    entropies.sort_by(|a, b| return (*a).partial_cmp(b).unwrap_or(Ordering::Equal));
    match entropies.len() {
        0 => println!("File is empty. No entropy. Zero bytes, ZERO BITS!!"),
        1 => println!("File has only one sector. Its entropy is {} bits.",
                      entropies[0]),
        _ => {
            let len = entropies.len();
            let pos99 = len * 99 / 100 + 1;
            let pos90 = len * 90 / 100 + 1;
            let pos50 = len * 50 / 100;
            println!("Min entropy: {} bits", entropies[0]);
            println!("Mean entropy: {} bits",
                     entropies.iter().fold(0.0, |a, x| a + x)
                     / entropies.len() as f32);
            println!("Median entropy: {} bits",
                     entropies[pos50]);
            if pos90 != pos50 && pos90 < len {
                println!("Top 10% entropy: {} bits",
                         entropies[pos90]);
                if pos99 != pos90 && pos99 < len {
                    println!("Top 1% entropy: {} bits",
                             entropies[pos99]);
                }
            }
        }
    }
    0
}

const BUFFERED_SECTORS: usize = 4000;
const READ_BUF_SIZE: usize = SECTOR_SIZE * (BUFFERED_SECTORS / 4);

struct FileStreamer {
    pub clean_tx: Sender<Vec<u8>>,
    pub dirty_rx: Receiver<io::Result<Vec<u8>>>,
}

impl FileStreamer {
    pub fn new(path: String) -> FileStreamer {
        let (clean_tx, clean_rx) = channel();
        let (dirty_tx, dirty_rx) = channel();
        for _ in 0..BUFFERED_SECTORS {
            clean_tx.send(Vec::with_capacity(SECTOR_SIZE)).unwrap();
        }
        thread::spawn(move || {
            // if the dirty channel is closed, we silently exit
            let mut file = match File::open(&path) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("Error opening {}: {}", &path, e);
                    return
                }
            };
            let mut buf: [u8; READ_BUF_SIZE] = unsafe { uninitialized() };
            loop {
                let len = match greedy_read(&mut file, &mut buf) {
                    Err(e) => {
                        dirty_tx.send(Err(e)).is_ok();
                        return
                    },
                    Ok(0) => break,
                    Ok(x) => x,
                };
                let mut pos = 0;
                while pos < len {
                    let mut v = match clean_rx.recv() {
                        Ok(v) => v,
                        Err(e) => {
                            dirty_tx.send(Err(io::Error::new(io::ErrorKind::Other, e))).is_ok();
                            return
                        },
                    };
                    let capacity = v.capacity();
                    let copy_len = capacity.min(len - pos);
                    unsafe { v.set_len(copy_len) };
                    v[..].copy_from_slice(&buf[pos..(pos+copy_len)]);
                    pos += v.len();
                    dirty_tx.send(Ok(v)).is_ok();
                }
                if len < READ_BUF_SIZE { break }
            }
            if let Ok(mut v) = clean_rx.recv() {
                v.clear();
                dirty_tx.send(Ok(v)).is_ok();
            }
        });
        FileStreamer { clean_tx, dirty_rx }
    }
}

const ALMOST_CERTAINLY_UNENCRYPTED_ENTROPY: f64 = 3700.0/(SECTOR_SIZE as f64);

fn slide(path_a: &str, path_b: &str) -> i32 {
    let stream_a = FileStreamer::new(path_a.to_owned());
    let stream_b = FileStreamer::new(path_b.to_owned());
    let mut pos: usize = 0;
    loop {
        let a = stream_a.dirty_rx.recv().unwrap().unwrap();
        let b = stream_b.dirty_rx.recv().unwrap().unwrap();
        assert_eq!(a.len(), b.len());
        if a.len() == 0 || b.len() == 0 { break }
        let ent_a = entropy_estimate(&a[..]) / a.len() as f64;
        let ent_b = entropy_estimate(&b[..]) / a.len() as f64;
        let plain_a = ent_a < ALMOST_CERTAINLY_UNENCRYPTED_ENTROPY;
        let plain_b = ent_b < ALMOST_CERTAINLY_UNENCRYPTED_ENTROPY;
        if plain_a {
            if plain_b {
                println!("At byte number {}, BOTH sides appear unencrypted.",
                         pos);
                return 0
            }
            else {
                println!("Our best guess is byte number {}.",
                         pos);
                return 0
            }
        }
        if a.len() < SECTOR_SIZE { break }
        debug_assert!(a.len() == SECTOR_SIZE);
        stream_a.clean_tx.send(a).is_ok();
        stream_b.clean_tx.send(b).is_ok();
        pos += SECTOR_SIZE;
    }
    println!("It looks like the left side is fully encrypted.");
    0
}

fn print_usage() {
    eprintln!("Usage: enthunter file (to get some nice statistics and burn some RAM)
   OR: enthunter fileA fileB (to search for where in-place encryption may have
       ended)");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    exit(match args.len() {
        2 => stats(&args[1]),
        3 => slide(&args[1], &args[2]),
        _ => { print_usage(); 1 },
    })
}
