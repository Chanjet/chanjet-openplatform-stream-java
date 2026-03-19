package com.chanjet.connector.api.exception;

/**
 * 对应应用无在线客户端异常。
 */
public class NoOnlineClientException extends ConnectorException {
    public NoOnlineClientException(String appKey) {
        super("No online client for app: " + appKey);
    }
}
