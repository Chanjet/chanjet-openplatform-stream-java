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
     * AES 解密逻辑 (遵循 AES-128-CBC 规范)。
     * Key: appSecret 前 16 位
     * IV: appSecret 后 16 位 (如果是 32 位 Secret)
     */
    static aesDecrypt(encryptedBase64: string, appSecret: string): string {
        if (!appSecret || appSecret.length < 16) {
            throw new Error('Invalid appSecret for AES decryption');
        }

        const key = appSecret.substring(0, 16);
        const iv = (appSecret.length >= 32) ? appSecret.substring(16, 32) : appSecret.substring(0, 16);

        const decipher = crypto.createDecipheriv('aes-128-cbc', Buffer.from(key), Buffer.from(iv));
        let decrypted = decipher.update(encryptedBase64, 'base64', 'utf8');
        decrypted += decipher.final('utf8');

        return decrypted;
    }
}
