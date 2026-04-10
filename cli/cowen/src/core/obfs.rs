/// Runtime deobfuscation support for the `obfs!` macro.
///
/// The macro is defined at crate root (main.rs) to ensure it's available
/// to all modules. This module provides only the runtime deobfuscation function.

/// Deobfuscate a byte slice that was obfuscated at compile time.
/// The key schedule uses a rotating XOR based on the seed.
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
    #[test]
    fn test_obfs_macro_roundtrip() {
        let result = obfs!("/v1/common/auth/selfBuiltApp/generateToken");
        assert_eq!(result, "/v1/common/auth/selfBuiltApp/generateToken");
    }

    #[test]
    fn test_obfs_macro_empty() {
        let result = obfs!("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_obfs_url() {
        let result = obfs!("https://openapi.chanjet.com");
        assert_eq!(result, "https://openapi.chanjet.com");
    }

    #[test]
    fn test_obfs_regex_pattern() {
        let result = obfs!(r#"(?i)("accessToken"\s*:\s*")([^"]+)(")"#);
        assert_eq!(result, r#"(?i)("accessToken"\s*:\s*")([^"]+)(")"#);
    }
}
