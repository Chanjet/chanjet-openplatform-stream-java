package com.chanjet.connector.sdk;

import javax.crypto.Cipher;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.util.Base64;

/**
 * 畅捷通开放平台加解密工具类。
 * 遵循 AES-128-ECB 规范。
 */
public class CryptoUtils {

    /**
     * AES 解密逻辑。
     * @param encryptedBase64 待解密的 Base64 字符串
     * @param decryptKey 独立的解密密钥
     */
    public static String aesDecrypt(String encryptedBase64, String decryptKey) {
        try {
            if (decryptKey == null || decryptKey.isEmpty()) {
                throw new IllegalArgumentException("Invalid decryptKey for AES decryption");
            }
            
            byte[] encryptedBytes = java.util.Base64.getDecoder().decode(encryptedBase64);
            Cipher cipher = Cipher.getInstance("AES/ECB/PKCS5Padding");
            SecretKeySpec keySpec = new SecretKeySpec(decryptKey.getBytes(StandardCharsets.UTF_8), "AES");
            
            cipher.init(Cipher.DECRYPT_MODE, keySpec);
            byte[] decryptedBytes = cipher.doFinal(encryptedBytes);
            
            return new String(decryptedBytes, StandardCharsets.UTF_8);
        } catch (Exception e) {
            throw new RuntimeException("AES decryption failed: " + e.getMessage(), e);
        }
    }
}
