package com.chanjet.connector.sdk;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import javax.crypto.Cipher;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;

public class CryptoUtilsTest {

    public static byte[] encryptAes(String text, String appSecret) throws Exception {
        String key = appSecret.substring(0, 16);
        String iv = appSecret.substring(16, 32);
        Cipher cipher = Cipher.getInstance("AES/CBC/PKCS5Padding");
        SecretKeySpec keySpec = new SecretKeySpec(key.getBytes(StandardCharsets.UTF_8), "AES");
        IvParameterSpec ivSpec = new IvParameterSpec(iv.getBytes(StandardCharsets.UTF_8));
        cipher.init(Cipher.ENCRYPT_MODE, keySpec, ivSpec);
        return cipher.doFinal(text.getBytes(StandardCharsets.UTF_8));
    }

    @Test
    public void testAesDecrypt() throws Exception {
        String originalText = """
            {"msg":"hello world"}""";
        String appSecret = "12345678901234567890123456789012"; // 32 chars
        
        byte[] encryptedBytes = encryptAes(originalText, appSecret);
        String encryptedBase64 = Base64.getEncoder().encodeToString(encryptedBytes);
        
        // Test Decryption
        String decryptedText = CryptoUtils.aesDecrypt(encryptedBase64, appSecret);
        assertEquals(originalText, decryptedText);
    }
}
