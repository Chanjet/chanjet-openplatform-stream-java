import * as crypto from 'crypto';

/**
 * 畅捷通开放平台加解密工具类。
 */
export class CryptoUtils {
    /**
     * HMAC-SHA256 签名算法。
     */
    static hmacSha256(data: string, secret: string): string {
        return crypto.createHmac('sha256', secret)
            .update(data)
            .digest('hex')
            .toLowerCase();
    }

    /**
     * AES 解密逻辑 (遵循 AES-128-ECB 规范)。
     * @param encryptedBase64 待解密的 Base64 字符串
     * @param decryptKey 独立的解密密钥
     */
    static aesDecrypt(encryptedBase64: string, decryptKey: string): string {
        if (!decryptKey || decryptKey.length === 0) {
            throw new Error('Invalid decryptKey for AES decryption');
        }

        const decipher = crypto.createDecipheriv('aes-128-ecb', Buffer.from(decryptKey), null);
        let decrypted = decipher.update(encryptedBase64, 'base64', 'utf8');
        decrypted += decipher.final('utf8');

        return decrypted;
    }
}
