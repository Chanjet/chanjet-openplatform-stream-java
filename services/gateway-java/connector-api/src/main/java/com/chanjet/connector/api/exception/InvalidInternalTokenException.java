package com.chanjet.connector.api.exception;

/**
 * 内部 P2P 令牌无效异常。
 */
public class InvalidInternalTokenException extends ConnectorException {
    public InvalidInternalTokenException() {
        super("Invalid or missing X-Internal-Token for P2P request.");
    }
}
