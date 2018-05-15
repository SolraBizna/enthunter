use std::mem::uninitialized;

/// Estimates the Shannon entropy of the given byte buffer, which must be less
/// than or equal to 65536 bytes long.
pub fn entropy_estimate(src: &[u8]) -> f64 {
    assert!(src.len() < 65536);
    let mut probabilities: [u16; 256] = unsafe { uninitialized() };
    for n in 0..256 { probabilities[n] = 0 }
    for byt in src {
        probabilities[*byt as usize] += 1;
    }
    let float_len = src.len() as f64;
    probabilities.iter()
        .map(|&x| if x == 0 { 0.0 }
             else {(x as f64 / float_len).log2() * x as f64})
        .fold(0.0, |a,e|  a - e)
}

#[cfg(test)]
mod tests {
    use super::entropy_estimate;
    #[test]
    fn simple_entropies() {
        assert_eq!(entropy_estimate(b"AAAAAAAAAAAAAAAAAAAA"), 0.0);
        assert_eq!(entropy_estimate(b"ABAABBAAABBBAAAABBBB"), 20.0);
        assert_eq!(entropy_estimate(b"ABCCBACCCCABABCCCABC"), 30.0);
    }
}
