package com.chanjet.connector.sdk;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * 畅捷通标准：企业临时授权码消息 (TEMP_AUTH_CODE)。
 */
public class EntAuthCodeMessage extends BaseMessage {

    @JsonProperty("bizContent")
    private BizContent bizContent;

    public BizContent getBizContent() { return bizContent; }
    public void setBizContent(BizContent bizContent) { this.bizContent = bizContent; }

    public static class BizContent {
        private String tempAuthCode;
        private String state;

        public String getTempAuthCode() { return tempAuthCode; }
        public void setTempAuthCode(String tempAuthCode) { this.tempAuthCode = tempAuthCode; }

        public String getState() { return state; }
        public void setState(String state) { this.state = state; }
    }
}
