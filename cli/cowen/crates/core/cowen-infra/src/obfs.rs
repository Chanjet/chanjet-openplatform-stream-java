#[macro_export]
macro_rules! obfs {
    ($s:expr) => {{
        const _LEN: usize = $s.len();
        const fn _obfs_bytes(s: &[u8]) -> [u8; 256] {
            let seed = (s.len() as u8).wrapping_mul(0x37).wrapping_add(0x5A);
            let mut out = [0u8; 256];
            let mut i = 0;
            while i < s.len() {
                let key = seed.wrapping_add(i as u8).wrapping_mul(0x6D);
                out[i] = s[i] ^ key;
                i += 1;
            }
            out
        }
        const _OBFS: [u8; 256] = _obfs_bytes($s.as_bytes());
        const _SEED: u8 = ($s.len() as u8).wrapping_mul(0x37).wrapping_add(0x5A);
        $crate::obfs::deobfs(&_OBFS[.._LEN], _SEED)
    }};
}

#[inline(always)]
pub fn deobfs(data: &[u8], seed: u8) -> String {
    let mut out = Vec::with_capacity(data.len());
    for (i, &b) in data.iter().enumerate() {
        let key = seed.wrapping_add(i as u8).wrapping_mul(0x6D);
        out.push(b ^ key);
    }
    String::from_utf8(out).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obfs_macro_and_deobfs() {
        let original = "secret_value_123";
        let obfuscated_str = obfs!("secret_value_123");
        assert_eq!(obfuscated_str, original);
    }

    #[test]
    fn test_deobfs_manual() {
        let seed = 123u8;
        let original = b"hello world!";
        let mut obfuscated = Vec::new();
        for (i, &b) in original.iter().enumerate() {
            let key = seed.wrapping_add(i as u8).wrapping_mul(0x6D);
            obfuscated.push(b ^ key);
        }

        let deobfuscated = deobfs(&obfuscated, seed);
        assert_eq!(deobfuscated, "hello world!");
    }
}
