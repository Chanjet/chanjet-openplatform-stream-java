package com.chanjet.connector.sdk;

import javax.crypto.Cipher;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.util.Base64;

/**
 * 畅捷通开放平台加解密工具类。
 * 遵循 AES-128-CBC 规范。
 */
public class CryptoUtils {

    /**
     * AES 解密逻辑。
     * Key: appSecret 前 16 位
     * IV: appSecret 后 16 位 (如果是 32 位 Secret)
     */
    public static String aesDecrypt(String encryptedBase64, String appSecret) {
        try {
            if (appSecret == null || appSecret.length() < 16) {
                throw new IllegalArgumentException("Invalid appSecret for AES decryption");
            }
            
            String key = appSecret.substring(0, 16);
            String iv = (appSecret.length() >= 32) ? appSecret.substring(16, 32) : appSecret.substring(0, 16);

            byte[] encryptedBytes = Base64.getDecoder().decode(encryptedBase64);
            Cipher cipher = Cipher.getInstance("AES/CBC/PKCS5Padding");
            SecretKeySpec keySpec = new SecretKeySpec(key.getBytes(StandardCharsets.UTF_8), "AES");
            IvParameterSpec ivSpec = new IvParameterSpec(iv.getBytes(StandardCharsets.UTF_8));
            
            cipher.init(Cipher.DECRYPT_MODE, keySpec, ivSpec);
            byte[] decryptedBytes = cipher.doFinal(encryptedBytes);
            
            return new String(decryptedBytes, StandardCharsets.UTF_8);
        } catch (Exception e) {
            throw new RuntimeException("AES decryption failed: " + e.getMessage(), e);
        }
    }
}
