//! Reads damaged copies of a real `.pub` and reports any panic: bytes changed at random, the file cut
//! short, runs of bytes zeroed or set. The original is only read. Usage: pubfuzz FILE.pub ROUNDS

use std::panic::{AssertUnwindSafe, catch_unwind};

/// A small repeatable random source: the same round always damages the file the same way.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(path), Some(rounds)) = (args.get(1), args.get(2).and_then(|r| r.parse::<u64>().ok())) else {
        println!("usage: pubfuzz FILE.pub ROUNDS");
        return;
    };
    let Ok(original) = std::fs::read(path) else {
        println!("can't read {path}");
        return;
    };
    let folder = std::env::temp_dir().join(format!("pubfuzz-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&folder);
    std::panic::set_hook(Box::new(|_| {}));
    let (mut read_ok, mut read_err, mut panics) = (0u64, 0u64, 0u64);
    let mut kinds = std::collections::BTreeMap::<String, u64>::new();
    for round in 1..=rounds {
        let mut rng = Rng(round.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut d = original.clone();
        match round % 4 {
            // A few bytes anywhere. The first 2 MB hold the small streams of most files.
            0 => {
                for _ in 0..1 + rng.below(40) {
                    let at = rng.below(d.len().min(2 * 1024 * 1024));
                    if let Some(b) = d.get_mut(at) {
                        *b = rng.next() as u8;
                    }
                }
            }
            // Cut short.
            1 => d.truncate(rng.below(d.len())),
            // A run set to all ones or all zeros.
            2 => {
                let at = rng.below(d.len().min(2 * 1024 * 1024));
                let len = 1 + rng.below(4096);
                let v = if rng.next() & 1 == 0 { 0xFF } else { 0 };
                for b in d.iter_mut().skip(at).take(len) {
                    *b = v;
                }
            }
            // Numbers set to the largest there are, four bytes at a time.
            _ => {
                for _ in 0..1 + rng.below(20) {
                    let at = rng.below(d.len().min(2 * 1024 * 1024).saturating_sub(4));
                    for b in d.iter_mut().skip(at).take(4) {
                        *b = 0xFF;
                    }
                }
            }
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| match designcraft_pubread::read(&d) {
            Ok(p) => {
                let out = designcraft_pubread::steps::steps(&p, &designcraft_pubread::steps::PlainFonts, &folder);
                (true, out.steps.len(), String::new())
            }
            Err(e) => (false, 0, e.to_string()),
        }));
        match outcome {
            Ok((true, _, _)) => read_ok += 1,
            Ok((false, _, e)) => {
                read_err += 1;
                *kinds.entry(e).or_default() += 1;
            }
            Err(_) => {
                panics += 1;
                println!("PANIC in round {round}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&folder);
    println!("{rounds} damaged copies: {read_ok} read, {read_err} refused with a message, {panics} panics");
    for (k, v) in kinds {
        println!("  {v} x {k}");
    }
}
