package com.chanjet.connector.server.config;

import jakarta.servlet.*;
import jakarta.servlet.http.HttpServletRequest;
import org.slf4j.MDC;
import org.springframework.core.Ordered;
import org.springframework.core.annotation.Order;
import org.springframework.stereotype.Component;

import java.io.IOException;
import java.util.UUID;

/**
 * 链路追踪过滤器。
 * 在请求进入 Servlet 层的第一时间提取并注入 TraceId，确保 MDC 贯穿整个处理链路。
 */
@Component
@Order(Ordered.HIGHEST_PRECEDENCE)
public class TraceIdFilter implements Filter {

    private static final String TRACE_ID_KEY = "traceId";
    private static final String HEADER_TRACE_ID = "X-Trace-Id";
    private static final String HEADER_MSG_ID = "X-MSG-ID";

    @Override
    public void doFilter(ServletRequest request, ServletResponse response, FilterChain chain) 
            throws IOException, ServletException {
        
        if (request instanceof HttpServletRequest httpRequest) {
            // 优先级：Header(MsgId) > Header(TraceId) > Auto-Generated
            // 强制使用 MsgId 作为全链路追踪 ID，以完美解决跨节点及 WebSocket 异步 ACK 时的日志割裂问题
            String traceId = httpRequest.getHeader(HEADER_MSG_ID);
            if (traceId == null || traceId.isEmpty()) {
                traceId = httpRequest.getHeader(HEADER_TRACE_ID);
            }
            if (traceId == null || traceId.isEmpty()) {
                traceId = UUID.randomUUID().toString().replace("-", "");
            }
            
            MDC.put(TRACE_ID_KEY, traceId);
        }

        try {
            chain.doFilter(request, response);
        } finally {
            MDC.remove(TRACE_ID_KEY);
        }
    }
}
