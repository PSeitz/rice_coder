use binggan::{BenchRunner, PeakMemAlloc, INSTRUMENTED_SYSTEM};
use rice_coder::RiceCoder;

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn bench_group() {
    // Tuples of name and data for the inputs
    let mut data: Vec<(&str, Vec<u32>)> = vec![
        (
            "sequential with gaps",
            (0..128)
                .filter(|&docid| docid % 10 != 0) // every 10th value is missing
                .collect(),
        ),
        ("sorted values", (0..128).collect()),
        (
            "random values u16::MAX",
            (0..128)
                .map(|_docid| rand::random::<u16>() as u32)
                .collect(),
        ),
        (
            "random values u8::MAX",
            (0..128).map(|_docid| rand::random::<u8>() as u32).collect(),
        ),
        (
            "random values small range(0..5)",
            (0..128)
                .map(|_docid| rand::random::<u8>() as u32 % 5)
                .collect(),
        ),
    ];
    for data in data.iter_mut() {
        data.1.sort();
    }
    let mut runner: BenchRunner = BenchRunner::new();
    runner.set_alloc(GLOBAL); // Set the peak mem allocator. This will enable peak memory reporting.

    runner.config().set_cache_trasher(true);

    for (input_name, data) in data.iter() {
        let mut group = runner.new_group();
        group.set_name(input_name);
        group.set_input_size(data.len() * std::mem::size_of::<usize>());

        for k in 1..8 {
            group.register_with_input(format!("write rice code k:{}", k), data, move |data| {
                let mut coder = RiceCoder::new(k); // Example with k = 3

                // Encoding
                let mut encoded: Vec<u8> = Vec::new();
                for &value in data {
                    coder.encode(value as u64, &mut encoded);
                }
                coder.finalize(&mut encoded);

                // Decoding
                //let decoded_values = coder.decode(&encoded, original_values.len());

                Some(encoded.len() as u64)
            });
        }
        group.run();
    }
}

fn main() {
    bench_group();
}
