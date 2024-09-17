pub struct RiceCoder {
    k: u8,          // The parameter k, determines the divisor (2^k)
    buffer: u64,    // A 64-bit buffer to store bits before flushing
    buffer_len: u8, // Number of bits currently in the buffer
}

// Precomputed table for unary encoding of small quotients (up to 7)
// Each entry is a tuple of the bits and the number of bits in that pattern
const UNARY_TABLE: [u32; 21] = [
    (0b0),                     // quotient = 0
    (0b10),                    // quotient = 1
    (0b110),                   // quotient = 2
    (0b1110),                  // quotient = 3
    (0b11110),                 // quotient = 4
    (0b111110),                // quotient = 5
    (0b1111110),               // quotient = 6
    (0b11111110),              // quotient = 7
    (0b111111110),             // quotient = 8
    (0b1111111110),            // quotient = 9
    (0b11111111110),           // quotient = 10
    (0b111111111110),          // quotient = 11
    (0b1111111111110),         // quotient = 12
    (0b11111111111110),        // quotient = 13
    (0b111111111111110),       // quotient = 14
    (0b1111111111111110),      // quotient = 15
    (0b11111111111111110),     // quotient = 16
    (0b111111111111111110),    // quotient = 17
    (0b1111111111111111110),   // quotient = 18
    (0b11111111111111111110),  // quotient = 19
    (0b111111111111111111110), // quotient = 20
];

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

impl RiceCoder {
    /// Constructor to create a RiceCoder with a given k value
    pub fn new(k: u8) -> Self {
        RiceCoder {
            k,
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
    fn write_bits_to_buffer(&mut self, value: u32, num_bits: u8, output: &mut Vec<u8>) {
        self.buffer = (self.buffer << num_bits) | (value as u64 & ((1 << num_bits) - 1));
        self.buffer_len += num_bits;

        self.flush_buffer(output);
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
    fn encode(&mut self, value: u32, output: &mut Vec<u8>) {
        let quotient = value >> self.k; // value / 2^k
        let remainder = value & ((1 << self.k) - 1); // value % 2^k

        // Use the precomputed table for encoding the quotient
        if quotient < UNARY_TABLE.len() as u32 {
            let unary_pattern = UNARY_TABLE[quotient as usize];
            let num_bits = quotient + 1; // Number of bits is quotient + 1 for the unary encoding
            self.write_bits_to_buffer(unary_pattern, num_bits as u8, output);
        } else {
            // For large quotients, fall back to the original loop method
            for _ in 0..quotient {
                self.write_bits_to_buffer(1, 1, output);
            }
            self.write_bits_to_buffer(0, 1, output);
        }

        // Write the remainder in binary form (k bits)
        self.write_bits_to_buffer(remainder, self.k, output);
    }

    /// Finalize encoding by flushing any remaining bits in the buffer
    pub fn finalize(&mut self, output: &mut Vec<u8>) {
        // If any bits remain in the buffer, flush them to the output
        if self.buffer_len > 0 {
            self.write_bits_to_buffer(0, 8 - self.buffer_len, output);
            self.flush_buffer(output);
        }
    }

    /// Rice decoding for multiple integers from a byte stream
    pub fn decode(&self, input: &[u8]) -> Vec<u32> {
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
            if let Some(remainder) = read_bits(input, self.k, &mut byte_pos, &mut bit_pos) {
                results.push((quotient << self.k) + remainder);
            } else {
                break; // Not enough bits to complete the number
            }
        }

        results
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_rice_coding() {
        let mut coder = RiceCoder::new(3); // Example with k = 3
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

    // Property-based test for random values
    proptest! {
        #[test]
        fn test_rice_coding_random_values(values in prop::collection::vec(0u32..=500_000, 0..20), k in 1u8..8) {
            let mut coder = RiceCoder::new(k); // Example with k = 3

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
