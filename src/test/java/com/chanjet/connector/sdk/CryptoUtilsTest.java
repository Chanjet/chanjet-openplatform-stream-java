package com.chanjet.connector.sdk;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import javax.crypto.Cipher;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;

public class CryptoUtilsTest {

    public static byte[] encryptAes(String text, String encryptKey) throws Exception {
        Cipher cipher = Cipher.getInstance("AES/ECB/PKCS5Padding");
        SecretKeySpec keySpec = new SecretKeySpec(encryptKey.getBytes(StandardCharsets.UTF_8), "AES");
        cipher.init(Cipher.ENCRYPT_MODE, keySpec);
        return cipher.doFinal(text.getBytes(StandardCharsets.UTF_8));
    }

    @Test
    public void testAesDecrypt() throws Exception {
        String originalText = "{\"msg\":\"hello world\"}";
        String encryptKey = "<DUMMY_KEY_16>"; // Exactly 16 bytes
        
        byte[] encryptedBytes = encryptAes(originalText, encryptKey);
        String encryptedBase64 = java.util.Base64.getEncoder().encodeToString(encryptedBytes);
        
        // Test Decryption
        String decryptedText = CryptoUtils.aesDecrypt(encryptedBase64, encryptKey);
        assertEquals(originalText, decryptedText);
    }
}
