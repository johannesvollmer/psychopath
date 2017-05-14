// Copyright (c) 2012 Leonhard Gruenschloss (leonhard@gruenschloss.org)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
// of the Software, and to permit persons to whom the Software is furnished to do
// so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//
// Adapted from Python to Rust and to generate Rust instead of C by Nathan Vegdahl

// Generate Rust code for evaluating Halton points with Faure-permutations for different bases.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;


/// How many components to generate.
const NUM_DIMENSIONS: usize = 256;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("halton.rs");
    let mut f = File::create(&dest_path).unwrap();

    // Init prime number array.
    let primes = {
        let mut primes = Vec::new();
        let mut candidate = 1;
        for _ in 0..NUM_DIMENSIONS {
            loop {
                candidate += 1;
                if is_prime(candidate) {
                    primes.push(candidate);
                    break;
                }
            }
        }
        primes
    };

    // Init Faure permutations.
    let faure = {
        let mut faure: Vec<Vec<usize>> = Vec::new();
        for b in 0..(primes.last().unwrap() + 1) {
            let perm = get_faure_permutation(&faure, b);
            faure.push(perm);
        }
        faure
    };

    // Write the beginning bits of the file
    f.write_all(
            format!(
                r#"
// Copyright (c) 2012 Leonhard Gruenschloss (leonhard@gruenschloss.org)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
// of the Software, and to permit persons to whom the Software is furnished to do
// so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// This file is automatically generated.

// Compute points of the Halton sequence with with Faure-permutations for different bases.

pub const MAX_DIMENSION: u32 = {};
"#,
                NUM_DIMENSIONS
            )
                    .as_bytes()
        )
        .unwrap();

    // Write the sampling function
    f.write_all(
            format!(
                r#"
#[inline]
pub fn sample(dimension: u32, index: u32) -> f32 {{
    match dimension {{"#
            )
                    .as_bytes()
        )
        .unwrap();

    for i in 0..NUM_DIMENSIONS {
        f.write_all(
                format!(
                    r#"
        {} => halton{}(index),"#,
                    i,
                    primes[i]
                )
                        .as_bytes()
            )
            .unwrap();
    }

    f.write_all(
            format!(
                r#"
        _ => panic!("Exceeded max dimensions."),
    }}
}}
    "#
            )
                    .as_bytes()
        )
        .unwrap();


    // Write the special-cased first dimension
    f.write_all(
            format!(
                r#"
// Special case: radical inverse in base 2, with direct bit reversal.
fn halton2(mut index: u32) -> f32 {{
    index = (index << 16) | (index >> 16);
    index = ((index & 0x00ff00ff) << 8) | ((index & 0xff00ff00) >> 8);
    index = ((index & 0x0f0f0f0f) << 4) | ((index & 0xf0f0f0f0) >> 4);
    index = ((index & 0x33333333) << 2) | ((index & 0xcccccccc) >> 2);
    index = ((index & 0x55555555) << 1) | ((index & 0xaaaaaaaa) >> 1);
    return (index as f32) * (1.0 / ((1u64 << 32) as f32));
}}
    "#
            )
                    .as_bytes()
        )
        .unwrap();

    for i in 1..NUM_DIMENSIONS {
        // Skip base 2.
        let base = primes[i];

        // Based on the permutation table size, we process multiple digits at once.
        let mut digits = 1;
        let mut pow_base = base;
        while pow_base * base <= 500 {
            // Maximum permutation table size.
            pow_base *= base;
            digits += 1;
        }

        let mut max_power = pow_base;
        let mut powers = Vec::new();
        while (max_power * pow_base) < (1 << 32) {
            // 32-bit unsigned precision
            powers.push(max_power);
            max_power *= pow_base;
        }

        // Build the permutation table.
        let perm = (0..pow_base)
            .map(|j| invert(&faure, base, j, digits))
            .collect::<Vec<_>>();
        let perm_string = {
            let mut perm_string = String::new();
            for i in perm.iter() {
                let s = format!("{}, ", i);
                perm_string.push_str(&s);
            }
            perm_string
        };

        let mut power = max_power / pow_base;
        f.write_all(
                format!(
                    r#"
fn halton{}(index: u32) -> f32 {{
    const PERM{}: [u16; {}] = [{}];"#,
                    base,
                    base,
                    perm.len(),
                    perm_string
                )
                        .as_bytes()
            )
            .unwrap();;

        f.write_all(
                format!(
                    r#"
    return (unsafe{{*PERM{}.get_unchecked((index % {}) as usize)}} as u32 * {} +"#,
                    base,
                    pow_base,
                    power
                )
                        .as_bytes()
            )
            .unwrap();;

        // Advance to next set of digits.
        let mut div = 1;
        while power / pow_base > 1 {
            div *= pow_base;
            power /= pow_base;
            f.write_all(
                    format!(
                        r#"
            unsafe{{*PERM{}.get_unchecked(((index / {}) % {}) as usize)}} as u32 * {} +"#,
                        base,
                        div,
                        pow_base,
                        power
                    )
                            .as_bytes()
                )
                .unwrap();;
        }

        f.write_all(
                format!(
                    r#"
            unsafe{{*PERM{}.get_unchecked(((index / {}) % {}) as usize)}} as u32) as f32 *
                   (0.999999940395355224609375f32 / ({}u32 as f32)); // Results in [0,1).
}}
        "#,
                    base,
                    div * pow_base,
                    pow_base,
                    max_power
                )
                        .as_bytes()
            )
            .unwrap();;
    }
}


/// Check primality. Not optimized, since it's not performance-critical.
fn is_prime(p: usize) -> bool {
    for i in 2..p {
        if (p % i) == 0 {
            return false;
        }
    }
    return true;
}

/// Computes the Faure digit permutation for 0, ..., b - 1.
fn get_faure_permutation(faure: &Vec<Vec<usize>>, b: usize) -> Vec<usize> {
    if b < 2 {
        return vec![0];
    } else if b == 2 {
        return vec![0, 1];
    } else if (b & 1) != 0 {
        // odd
        let c = (b - 1) / 2;

        return (0..b)
                   .map(
            |i| {
                if i == c {
                    return c;
                }

                let f: usize = faure[b - 1][i - ((i > c) as usize)];
                f + ((f >= c) as usize)
            }
        )
                   .collect();
    } else {
        // even
        let c = b / 2;

        return (0..b)
                   .map(
            |i| if i < c {
                2 * faure[c][i]
            } else {
                2 * faure[c][i - c] + 1
            }
        )
                   .collect();
    }
}

/// Compute the radical inverse with Faure permutations.
fn invert(faure: &Vec<Vec<usize>>, base: usize, mut index: usize, digits: usize) -> usize {
    let mut result = 0;
    for _ in 0..digits {
        let remainder = index % base;
        index = index / base;
        result = result * base + faure[base][remainder];
    }
    return result;
}
