package com.chanjet.connector.sdk;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * 畅捷通标准：解除授权消息 (APP_CANCEL_AUTHORIZATION)。
 */
public class EntUnauthMessage extends BaseMessage {

    @JsonProperty("bizContent")
    private BizContent bizContent;

    public BizContent getBizContent() { return bizContent; }
    public void setBizContent(BizContent bizContent) { this.bizContent = bizContent; }

    public static class BizContent {
        private String appKey;
        private String appId;
        private String orgId;
        private String userId;
        private String completedTime;

        public String getAppKey() { return appKey; }
        public void setAppKey(String appKey) { this.appKey = appKey; }

        public String getAppId() { return appId; }
        public void setAppId(String appId) { this.appId = appId; }

        public String getOrgId() { return orgId; }
        public void setOrgId(String orgId) { this.orgId = orgId; }

        public String getUserId() { return userId; }
        public void setUserId(String userId) { this.userId = userId; }

        public String getCompletedTime() { return completedTime; }
        public void setCompletedTime(String completedTime) { this.completedTime = completedTime; }
    }
}
