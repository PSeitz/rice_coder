use std::ops::Range;

use binggan::{BenchRunner, PeakMemAlloc, INSTRUMENTED_SYSTEM};
use rand::{prelude::Distribution, rngs::StdRng, SeedableRng};
use rice_coder::{create_rice_coder, estimate_optimal_k};

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn bench_group() {
    // Tuples of name and data for the inputs
    let mut rng = StdRng::from_seed([3u8; 32]);
    let zipf = zipf::ZipfDistribution::new(2000, 1.5).unwrap();
    let zipf2 = zipf::ZipfDistribution::new(200000, 1.5).unwrap();
    let mut data: Vec<(&str, Vec<u32>, Range<u8>)> = vec![
        (
            "sequential with gaps",
            (0..255).map(|docid| docid * 10).collect(),
            (1..8),
        ),
        ("sorted values", (0..255).collect(), (1..8)),
        (
            "random values u8::MAX",
            (0..255).map(|_docid| rand::random::<u8>() as u32).collect(),
            (1..8),
        ),
        (
            "random values u16::MAX",
            (0..255)
                .map(|_docid| rand::random::<u16>() as u32)
                .collect(),
            (1..16),
        ),
        (
            "zipfs values max 2000",
            (0..255)
                .map(|_docid| zipf.sample(&mut rng) as u32)
                .collect(),
            (1..8),
        ),
        (
            "zipfs values max 200000",
            (0..255)
                .map(|_docid| zipf2.sample(&mut rng) as u32)
                .collect(),
            (1..8),
        ),
        (
            "random values small range(0..5)",
            (0..255)
                .map(|_docid| rand::random::<u8>() as u32 % 5)
                .collect(),
            (1..8),
        ),
    ];
    for data in data.iter_mut() {
        data.1.sort();
    }
    let mut runner: BenchRunner = BenchRunner::new();
    runner.set_alloc(GLOBAL); // Set the peak mem allocator. This will enable peak memory reporting.

    runner.config().set_cache_trasher(true);

    for (input_name, data, k_range) in data.iter() {
        let mut group = runner.new_group();
        group.set_name(input_name);
        group.set_input_size(data.len() * std::mem::size_of::<u32>());

        for k in k_range.clone() {
            group.register_with_input(format!("write rice code k:{}", k), data, move |data| {
                let mut coder = create_rice_coder(k);

                // Encoding
                let mut encoded: Vec<u8> = Vec::with_capacity(data.len() * 4);
                coder.encode_vals(data, &mut encoded);

                // Decoding
                //let decoded_values = coder.decode(&encoded, original_values.len());

                Some(encoded.len() as u64)
            });
        }
        for percentile in [50, 80, 90, 100].iter() {
            group.register_with_input(
                format!(
                    "write rice code k detect based on {} percentile",
                    percentile
                ),
                data,
                move |data| {
                    let k = estimate_optimal_k(data, *percentile);
                    let mut coder = create_rice_coder(k);

                    let mut encoded: Vec<u8> = Vec::with_capacity(data.len() * 4);
                    coder.encode_vals(data, &mut encoded);
                    //Some(encoded.len() as u64)
                    let mut sorted_values = data.to_vec();
                    sorted_values.sort_unstable();

                    //Some(sorted_values[sorted_values.len() / 2] as u64)
                    //Some(k as u64)
                    Some(encoded.len() as u64)
                },
            );
        }

        group.run();

        let mut encoded_per_k: Vec<(Vec<u8>, u32)> = vec![(Vec::new(), 0); k_range.end as usize];
        for k in k_range.clone() {
            let mut encoded: Vec<u8> = Vec::new();
            let mut coder = create_rice_coder(k);
            coder.encode_vals(data, &mut encoded);
            encoded_per_k[k as usize].0 = encoded;
            encoded_per_k[k as usize].1 = data.len() as u32;
        }
        let mut group = runner.new_group();
        group.set_name(input_name);
        group.set_input_size(data.len() * std::mem::size_of::<u32>());
        #[allow(clippy::needless_range_loop)]
        for k in k_range.clone() {
            let encoded = &encoded_per_k[k as usize];

            group.register_with_input(
                format!("read rice code k:{}", k),
                encoded,
                move |(data, num_vals)| {
                    // Decoding
                    let coder = create_rice_coder(k);
                    let mut decoded_values = Vec::new();
                    coder.decode_into(data, &mut decoded_values, *num_vals);

                    Some(decoded_values.len() as u64)
                },
            );
        }
        group.run();
    }
}

fn main() {
    bench_group();
}
