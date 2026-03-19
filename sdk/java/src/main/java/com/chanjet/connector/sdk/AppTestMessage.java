package com.chanjet.connector.sdk;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * 畅捷通标准：验证消息 (APP_TEST)。
 */
public class AppTestMessage extends BaseMessage {

    @JsonProperty("bizContent")
    private BizContent bizContent;

    public BizContent getBizContent() { return bizContent; }
    public void setBizContent(BizContent bizContent) { this.bizContent = bizContent; }

    public static class BizContent {
        private String message;

        public String getMessage() { return message; }
        public void setMessage(String message) { this.message = message; }
    }
}
