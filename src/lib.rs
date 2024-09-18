/// Function to estimate the optimal `k` based on a given percentile.
/// `values`: slice of input values to process.
/// `percentile`: desired percentile (e.g., 50.0 for median, 90.0 for 90th percentile).
pub fn estimate_optimal_k(values: &[u32], percentile: f64) -> u8 {
    // Ensure there are values to process
    if values.is_empty() {
        return 0;
    }

    // Sort the values
    let mut sorted_values = values.to_vec();
    sorted_values.sort_unstable();

    // Determine the index for the desired percentile
    let percentile_index = ((percentile / 100.0) * (sorted_values.len() as f64)).round() as usize;

    // Handle case where percentile index is out of bounds
    let percentile_index = std::cmp::min(percentile_index, sorted_values.len() - 1);

    // Get the value at the desired percentile
    let value_at_percentile = sorted_values[percentile_index];

    // Use the log2 of the percentile value to estimate k
    (32 - value_at_percentile.leading_zeros()) as u8
}

// Define a trait for the RiceCoder's functionality
pub trait RiceCoderTrait {
    fn encode_vals(&mut self, values: &[u32], output: &mut Vec<u8>);
    fn decode(&self, input: &[u8]) -> Vec<u32>;
}

pub struct RiceCoder<const K: u8> {
    buffer: u64,    // A 64-bit buffer to store bits before flushing
    buffer_len: u8, // Number of bits currently in the buffer
}

impl<const K: u8> RiceCoder<K> {
    /// Constructor to create a RiceCoder with a const generic k value
    pub fn new() -> Self {
        RiceCoder {
            buffer: 0,
            buffer_len: 0,
        }
    }

    /// Helper function to flush the buffer to the output vector once it's full or when needed
    fn flush_buffer(&mut self, output: &mut Vec<u8>) {
        while self.buffer_len >= 8 {
            let byte = (self.buffer >> (self.buffer_len - 8)) as u8;
            output.push(byte);
            self.buffer_len -= 8;
            self.buffer &= (1 << self.buffer_len) - 1; // Keep only remaining bits in buffer
        }
    }

    /// Helper function to write bits to the buffer
    #[inline]
    fn write_bits_to_buffer(&mut self, value: u32, num_bits: u8) {
        self.buffer <<= num_bits;
        self.buffer |= value as u64;
        self.buffer_len += num_bits;
    }

    pub fn encode_vals(&mut self, values: &[u32], output: &mut Vec<u8>) {
        assert!(values.len() < 256); // Limit the number of values to 255
        output.push(values.len() as u8);
        for value in values {
            self.encode(*value, output);
        }
        self.finalize(output);
    }

    /// Rice encoding for a given integer
    #[inline]
    fn encode(&mut self, value: u32, output: &mut Vec<u8>) {
        let quotient = value >> K; // value / 2^k
        let remainder = value & ((1 << K) - 1); // value % 2^k

        let mut remaining = quotient;

        // Write blocks of 32 `1`s at a time
        while remaining >= 32 {
            self.write_bits_to_buffer(0xFFFFFFFF, 32); // 0xFFFFFFFF is thirty-two 1s
            remaining -= 32;
            self.flush_buffer(output);
        }

        // Write any remaining 1s
        if remaining > 0 {
            let mask = (1u32 << remaining) - 1; // Create a mask of `remaining` 1s
            self.write_bits_to_buffer(mask, remaining as u8);
        }

        // Write the final `0` after all 1s
        self.write_bits_to_buffer(0, 1);

        // Write the remainder in binary form (k bits)
        self.write_bits_to_buffer(remainder, K);
        self.flush_buffer(output);
    }

    /// Finalize encoding by flushing any remaining bits in the buffer
    pub fn finalize(&mut self, output: &mut Vec<u8>) {
        if self.buffer_len > 0 {
            // Pad with 0, so we can flush it
            self.write_bits_to_buffer(0, 8 - self.buffer_len);
            self.flush_buffer(output);
        }
    }

    /// Rice decoding for multiple integers from a byte stream
    pub fn decode(input: &[u8]) -> Vec<u32> {
        let total_values = input[0] as usize;
        let input = &input[1..];
        let mut results = Vec::new();
        let mut bit_pos: u8 = 0;
        let mut byte_pos: usize = 0;

        // Helper function to read a single bit from the input buffer
        fn read_bit(input: &[u8], byte_pos: &mut usize, bit_pos: &mut u8) -> Option<bool> {
            if *byte_pos >= input.len() {
                return None;
            }

            let bit = (input[*byte_pos] >> (7 - *bit_pos)) & 1 == 1;
            *bit_pos = (*bit_pos + 1) % 8;

            if *bit_pos == 0 {
                *byte_pos += 1;
            }

            Some(bit)
        }

        // Helper function to read multiple bits from the input buffer
        fn read_bits(
            input: &[u8],
            num_bits: u8,
            byte_pos: &mut usize,
            bit_pos: &mut u8,
        ) -> Option<u32> {
            let mut value = 0;
            for _ in 0..num_bits {
                if let Some(bit) = read_bit(input, byte_pos, bit_pos) {
                    value = (value << 1) | (bit as u32);
                } else {
                    return None; // Not enough bits
                }
            }
            Some(value)
        }

        while byte_pos < input.len() && results.len() < total_values {
            // Decode unary quotient
            let mut quotient: u32 = 0;
            while let Some(bit) = read_bit(input, &mut byte_pos, &mut bit_pos) {
                if bit {
                    quotient += 1;
                } else {
                    break;
                }
            }

            // Decode the binary remainder
            if let Some(remainder) = read_bits(input, K, &mut byte_pos, &mut bit_pos) {
                results.push((quotient << K) + remainder);
            } else {
                break; // Not enough bits to complete the number
            }
        }

        results
    }
}

impl<const K: u8> Default for RiceCoder<K> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const K: u8> RiceCoderTrait for RiceCoder<K> {
    fn encode_vals(&mut self, values: &[u32], output: &mut Vec<u8>) {
        self.encode_vals(values, output);
    }

    fn decode(&self, input: &[u8]) -> Vec<u32> {
        Self::decode(input)
    }
}
// Function to create a RiceCoder dynamically based on runtime k value (up to 32K)
pub fn create_rice_coder(k: u8) -> Box<dyn RiceCoderTrait> {
    match k {
        0 => Box::new(RiceCoder::<0>::new()),
        1 => Box::new(RiceCoder::<1>::new()),
        2 => Box::new(RiceCoder::<2>::new()),
        3 => Box::new(RiceCoder::<3>::new()),
        4 => Box::new(RiceCoder::<4>::new()),
        5 => Box::new(RiceCoder::<5>::new()),
        6 => Box::new(RiceCoder::<6>::new()),
        7 => Box::new(RiceCoder::<7>::new()),
        8 => Box::new(RiceCoder::<8>::new()),
        9 => Box::new(RiceCoder::<9>::new()),
        10 => Box::new(RiceCoder::<10>::new()),
        11 => Box::new(RiceCoder::<11>::new()),
        12 => Box::new(RiceCoder::<12>::new()),
        13 => Box::new(RiceCoder::<13>::new()),
        14 => Box::new(RiceCoder::<14>::new()),
        15 => Box::new(RiceCoder::<15>::new()),
        16 => Box::new(RiceCoder::<16>::new()),
        17 => Box::new(RiceCoder::<17>::new()),
        18 => Box::new(RiceCoder::<18>::new()),
        19 => Box::new(RiceCoder::<19>::new()),
        20 => Box::new(RiceCoder::<20>::new()),
        21 => Box::new(RiceCoder::<21>::new()),
        22 => Box::new(RiceCoder::<22>::new()),
        23 => Box::new(RiceCoder::<23>::new()),
        24 => Box::new(RiceCoder::<24>::new()),
        25 => Box::new(RiceCoder::<25>::new()),
        26 => Box::new(RiceCoder::<26>::new()),
        27 => Box::new(RiceCoder::<27>::new()),
        28 => Box::new(RiceCoder::<28>::new()),
        29 => Box::new(RiceCoder::<29>::new()),
        30 => Box::new(RiceCoder::<30>::new()),
        31 => Box::new(RiceCoder::<31>::new()),
        _ => panic!("Unsupported k value!"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_rice_coding() {
        let mut coder = RiceCoder::<3>::new(); // Example with k = 3
        let original_values: Vec<u32> = vec![37, 12, 5, 150, 255, 0, 10];

        // Encoding
        let mut encoded: Vec<u8> = Vec::new();
        coder.encode_vals(&original_values, &mut encoded);

        // Decoding
        let decoded_values = coder.decode(&encoded);

        // Assert that the decoded values match the original values
        assert_eq!(original_values, decoded_values);
    }

    #[test]
    fn test_calculate_optimal_k_small_values() {
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let optimal_k = estimate_optimal_k(&values, 50.0);
        assert_eq!(optimal_k, 3);

        let optimal_k_90 = estimate_optimal_k(&values, 90.0);
        assert_eq!(optimal_k_90, 4);
    }

    #[test]
    fn print_test() {
        for val in 1..12 {
            print!("{:0>2} ", val);
            print::<2>(val);
        }
        //print(2, 2);
        //print(2, 3);
        //print(4, 4);
        //print(1, 40);
        //print(1, 41);
        //print(1, 42);
    }

    fn print<const K: u8>(val: u32) {
        let mut coder = RiceCoder::<K>::new(); // Example with k = 3

        // Encoding
        let mut encoded: Vec<u8> = Vec::new();
        coder.encode(val, &mut encoded);
        coder.finalize(&mut encoded);
        print_bits(&encoded);
    }

    fn print_bits(bytes: &[u8]) {
        for byte in bytes.iter() {
            // Print the binary representation of each byte, padded to 8 bits
            print!("{:08b} ", byte);
        }
        println!(); // Newline after printing all bytes
    }

    // Property-based test for random values
    proptest! {
        #[test]
        fn test_rice_coding_random_values(values in prop::collection::vec(0u32..=500_000, 0..20), k in 1u8..8) {
            let mut coder = create_rice_coder(k); // Create a RiceCoder with the given k value

            // Encoding
            let mut encoded: Vec<u8> = Vec::new();
            coder.encode_vals(&values, &mut encoded);

            // Decoding
            let decoded_values = coder.decode(&encoded);

            // Assert that the decoded values match the original values
            prop_assert_eq!(values, decoded_values);
        }
    }
}
