pub struct RiceCoder {
    k: u8,          // The parameter k, determines the divisor (2^k)
    buffer: u64,    // A 64-bit buffer to store bits before flushing
    buffer_len: u8, // Number of bits currently in the buffer
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
    fn write_bits_to_buffer(&mut self, value: u64, num_bits: u8, output: &mut Vec<u8>) {
        self.buffer = (self.buffer << num_bits) | (value & ((1 << num_bits) - 1));
        self.buffer_len += num_bits;

        // Flush the buffer if we accumulate more than 64 bits
        self.flush_buffer(output);
    }

    /// Rice encoding for a given integer
    pub fn encode(&mut self, value: u64, output: &mut Vec<u8>) {
        let quotient = value >> self.k; // value / 2^k
        let remainder = value & ((1 << self.k) - 1); // value % 2^k

        // Unary encoding of the quotient (using 1s followed by a 0)
        for _ in 0..quotient {
            // Write a `1`. TODO: use a table + shift operation for better performance
            self.write_bits_to_buffer(1, 1, output);
        }
        self.write_bits_to_buffer(0, 1, output); // Write the `0` to end the unary part

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
    pub fn decode(&self, input: &[u8], total_values: usize) -> Vec<u64> {
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
        ) -> Option<u64> {
            let mut value = 0;
            for _ in 0..num_bits {
                if let Some(bit) = read_bit(input, byte_pos, bit_pos) {
                    value = (value << 1) | (bit as u64);
                } else {
                    return None; // Not enough bits
                }
            }
            Some(value)
        }

        while byte_pos < input.len() && results.len() < total_values {
            // Decode unary quotient
            let mut quotient: u64 = 0;
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
        let original_values = vec![37, 12, 5, 150, 255, 0, 10];

        // Encoding
        let mut encoded: Vec<u8> = Vec::new();
        for &value in &original_values {
            coder.encode(value, &mut encoded);
        }
        coder.finalize(&mut encoded);

        // Decoding
        let decoded_values = coder.decode(&encoded, original_values.len());

        // Assert that the decoded values match the original values
        assert_eq!(original_values, decoded_values);
    }

    // Property-based test for random values
    proptest! {
        #[test]
        fn test_rice_coding_random_values(values in prop::collection::vec(0u64..=500_000, 0..20), k in 1u8..8) {
            let mut coder = RiceCoder::new(k); // Example with k = 3

            // Encoding
            let mut encoded: Vec<u8> = Vec::new();
            for &value in &values {
                coder.encode(value, &mut encoded);
            }
            coder.finalize(&mut encoded);

            // Decoding
            let decoded_values = coder.decode(&encoded, values.len());

            // Assert that the decoded values match the original values
            prop_assert_eq!(values, decoded_values);
        }
    }
}
