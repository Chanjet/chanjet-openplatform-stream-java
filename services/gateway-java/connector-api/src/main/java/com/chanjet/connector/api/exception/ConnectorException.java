package com.chanjet.connector.api.exception;

/**
 * 领域异常基类。
 */
public abstract class ConnectorException extends RuntimeException {
    public ConnectorException(String message) {
        super(message);
    }
}
