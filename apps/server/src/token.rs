/// Generates a 32-byte CSPRNG token rendered as lowercase hex.
///
/// Returns `None` only when the operating system entropy source fails.
pub(crate) fn random_token() -> Option<String> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    Some(encoded)
}

/// Compares two tokens without leaking their common prefix length.
pub(crate) fn tokens_match(expected: &str, actual: &str) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    expected
        .as_bytes()
        .iter()
        .zip(actual.as_bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::{random_token, tokens_match};

    #[test]
    fn tokens_are_hex_and_unique() {
        let first = random_token().expect("token");
        let second = random_token().expect("token");

        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn comparison_rejects_different_lengths_and_contents() {
        assert!(tokens_match("abcd", "abcd"));
        assert!(!tokens_match("abcd", "abce"));
        assert!(!tokens_match("abcd", "abcde"));
    }
}
