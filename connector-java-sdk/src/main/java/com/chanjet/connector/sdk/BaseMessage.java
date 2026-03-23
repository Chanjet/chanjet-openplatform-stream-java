package com.chanjet.connector.sdk;

import com.fasterxml.jackson.annotation.JsonAlias;
import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.Map;

/**
 * 业务推送消息基类。
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public abstract class BaseMessage {

    @JsonProperty("msgId")
    @JsonAlias({"id", "msgId"})
    private String msgId;

    @JsonProperty("msgType")
    private String msgType;

    @JsonProperty("appKey")
    private String appKey;

    @JsonProperty("appId")
    private String appId;

    @JsonProperty("requestId")
    private String requestId;

    @JsonProperty("uniqueId")
    private String uniqueId;

    @JsonProperty("bookCode")
    private String bookCode;

    @JsonProperty("orgId")
    private String orgId;

    @JsonProperty("timestamp")
    @JsonAlias({"time", "timestamp"})
    private Long timestamp;

    private Map<String, String> headers;

    // Getters and Setters
    public String getMsgId() { return msgId; }
    public void setMsgId(String msgId) { this.msgId = msgId; }

    public String getMsgType() { return msgType; }
    public void setMsgType(String msgType) { this.msgType = msgType; }

    public String getAppKey() { return appKey; }
    public void setAppKey(String appKey) { this.appKey = appKey; }

    public String getAppId() { return appId; }
    public void setAppId(String appId) { this.appId = appId; }

    public String getRequestId() { return requestId; }
    public void setRequestId(String requestId) { this.requestId = requestId; }

    public String getUniqueId() { return uniqueId; }
    public void setUniqueId(String uniqueId) { this.uniqueId = uniqueId; }

    public String getBookCode() { return bookCode; }
    public void setBookCode(String bookCode) { this.bookCode = bookCode; }

    public String getOrgId() { return orgId; }
    public void setOrgId(String orgId) { this.orgId = orgId; }

    public Long getTimestamp() { return timestamp; }
    public void setTimestamp(Long timestamp) { this.timestamp = timestamp; }

    public Map<String, String> getHeaders() { return headers; }
    public void setHeaders(Map<String, String> headers) { this.headers = headers; }
}
