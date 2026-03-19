package com.chanjet.connector.server.exception;

import com.chanjet.connector.api.exception.NoOnlineClientException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.MissingRequestHeaderException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.util.Map;

/**
 * 全局异常处理器，将领域异常及框架异常映射为标准的 HTTP 状态码。
 */
@RestControllerAdvice
public class GlobalExceptionHandler {

    private static final Logger log = LoggerFactory.getLogger(GlobalExceptionHandler.class);

    /**
     * 处理必填 Header 缺失异常 (400)。
     */
    @ExceptionHandler(MissingRequestHeaderException.class)
    public ResponseEntity<Map<String, String>> handleMissingHeader(MissingRequestHeaderException e) {
        return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                .body(Map.of("error", "bad_request", "message", e.getMessage()));
    }

    /**
     * 处理无在线客户端异常 (503)。
     */
    @ExceptionHandler(NoOnlineClientException.class)
    public ResponseEntity<Map<String, String>> handleNoOnlineClient(NoOnlineClientException e) {
        log.warn("Dispatch failed: {}", e.getMessage());
        return ResponseEntity.status(HttpStatus.SERVICE_UNAVAILABLE)
                .body(Map.of("error", "no_online_client", "message", e.getMessage()));
    }

    /**
     * 处理通用的领域逻辑异常或其他未知异常 (500)。
     */
    @ExceptionHandler(Exception.class)
    public ResponseEntity<Map<String, String>> handleGenericException(Exception e) {
        // 如果异常的消息包含特定的领域关键字，也可在此进一步细化映射
        if (e.getMessage() != null && e.getMessage().contains("No client online")) {
            return ResponseEntity.status(HttpStatus.SERVICE_UNAVAILABLE)
                    .body(Map.of("error", "no_online_client", "message", e.getMessage()));
        }
        
        log.error("Internal server error", e);
        return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(Map.of("error", "internal_error", "message", e.getMessage()));
    }
}
