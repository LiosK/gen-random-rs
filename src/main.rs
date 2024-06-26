use std::{io, mem};

use zerocopy::AsBytes as _;

const BUF_SIZE: usize = 32 * 1024;
const RESEED_INTERVAL: usize = 512 * 1024;

fn main() -> io::Result<()> {
    run(&mut io::stdout().lock())
}

fn run(out: &mut impl io::Write) -> io::Result<()> {
    const _: () = assert!(BUF_SIZE % mem::size_of::<u64>() == 0);
    let mut buf_seeds = [0u64; BUF_SIZE / mem::size_of::<u64>()];
    let mut buf_rands = [0u64; BUF_SIZE / mem::size_of::<u64>()];

    loop {
        getrandom::getrandom(buf_seeds.as_bytes_mut())?;

        for mut s in buf_seeds {
            if s == 0 {
                continue;
            }

            const _: () = assert!(RESEED_INTERVAL % BUF_SIZE == 0);
            for _ in 0..(RESEED_INTERVAL / BUF_SIZE) {
                for e in buf_rands.iter_mut() {
                    // xorshift64* (Vigna 2016)
                    s ^= s >> 12;
                    s ^= s << 25;
                    s ^= s >> 27;
                    *e = s.wrapping_mul(2685821657736338717);
                }

                match out.write_all(buf_rands.as_bytes()) {
                    Err(e) if e.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
                    ret => ret?,
                }
            }
        }
    }
}

#[cfg(test)]
#[test]
fn quick_randomness_test() {
    const N: usize = 1024 * 1024 * 1024;

    #[derive(Default)]
    struct Logger {
        n_bytes: usize,
        n_ones: usize,
        carry: u8,
        n_twins: usize,
    }

    impl io::Write for Logger {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.n_bytes >= N {
                return Err(io::ErrorKind::BrokenPipe.into());
            }

            for &e in buf {
                self.n_ones += e.count_ones() as usize;

                let shifted = self.carry | e >> 1;
                self.carry = e << 7;
                self.n_twins += (e ^ shifted).count_zeros() as usize;
            }

            self.n_bytes += buf.len();
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut w = Logger::default();
    assert!(run(&mut w).is_ok() && w.n_bytes >= N);

    let n_samples = w.n_bytes as f64 * 8.0;
    let p_ones = w.n_ones as f64 / n_samples;
    let p_twins = w.n_twins as f64 / n_samples;

    // set margin based on binom dist 99.999% confidence interval
    let margin = 4.417173 * (0.5 * 0.5 / n_samples).sqrt();

    assert!(
        (p_ones - 0.5).abs() < margin,
        "% of set bits: {}% ({}/{}; 99.999% CI: {}%-{}%)",
        p_ones * 100.0,
        w.n_ones,
        w.n_bytes * 8,
        (0.5 - margin) * 100.0,
        (0.5 + margin) * 100.0,
    );
    assert!(
        (p_twins - 0.5).abs() < margin,
        "% of twin (00/11) bits: {}% ({}/{}; 99.999% CI: {}%-{}%)",
        p_twins * 100.0,
        w.n_twins,
        w.n_bytes * 8,
        (0.5 - margin) * 100.0,
        (0.5 + margin) * 100.0,
    );
}
