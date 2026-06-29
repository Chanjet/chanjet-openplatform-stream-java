package com.chanjet.connector.sdk;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * 畅捷通标准：应用票据消息 (appTicketMsg)。
 */
public class AppTicketMessage extends BaseMessage {

    @JsonProperty("bizContent")
    private BizContent bizContent;

    public BizContent getBizContent() { return bizContent; }
    public void setBizContent(BizContent bizContent) { this.bizContent = bizContent; }

    public static class BizContent {
        private String appTicket;

        public String getAppTicket() { return appTicket; }
        public void setAppTicket(String appTicket) { this.appTicket = appTicket; }
    }
}
