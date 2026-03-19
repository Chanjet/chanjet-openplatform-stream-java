package com.chanjet.connector.sdk;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.Map;

/**
 * 畅捷通“好系列”（好生意、好业财）标准 APP_NOTICE 消息模型。
 */
public class AppNoticeMessage extends BaseMessage {

    @JsonProperty("bizContent")
    private NoticeContent bizContent;

    public NoticeContent getBizContent() { return bizContent; }
    public void setBizContent(NoticeContent bizContent) { this.bizContent = bizContent; }

    public static class NoticeContent {
        private String appId;
        private String bizDate;
        private Long bizTypeId;
        private String boName;
        private String code;
        private Long dataId;
        private String mobile;
        private String operationContent;
        private String operationTime;
        private String redBlueFlag;
        private String requestId;
        private Long resourceCategoryId;
        private Long resourceId;
        private String sourceTypeEnum;
        private Long tenantId;
        private String transactionTypeEnum;
        private Long userId;
        private String userName;

        // Getters and Setters
        public String getAppId() { return appId; }
        public void setAppId(String appId) { this.appId = appId; }

        public String getBizDate() { return bizDate; }
        public void setBizDate(String bizDate) { this.bizDate = bizDate; }

        public Long getBizTypeId() { return bizTypeId; }
        public void setBizTypeId(Long bizTypeId) { this.bizTypeId = bizTypeId; }

        public String getBoName() { return boName; }
        public void setBoName(String boName) { this.boName = boName; }

        public String getCode() { return code; }
        public void setCode(String code) { this.code = code; }

        public Long getDataId() { return dataId; }
        public void setDataId(Long dataId) { this.dataId = dataId; }

        public String getMobile() { return mobile; }
        public void setMobile(String mobile) { this.mobile = mobile; }

        public String getOperationContent() { return operationContent; }
        public void setOperationContent(String operationContent) { this.operationContent = operationContent; }

        public String getOperationTime() { return operationTime; }
        public void setOperationTime(String operationTime) { this.operationTime = operationTime; }

        public String getRedBlueFlag() { return redBlueFlag; }
        public void setRedBlueFlag(String redBlueFlag) { this.redBlueFlag = redBlueFlag; }

        public String getRequestId() { return requestId; }
        public void setRequestId(String requestId) { this.requestId = requestId; }

        public Long getResourceCategoryId() { return resourceCategoryId; }
        public void setResourceCategoryId(Long resourceCategoryId) { this.resourceCategoryId = resourceCategoryId; }

        public Long getResourceId() { return resourceId; }
        public void setResourceId(Long resourceId) { this.resourceId = resourceId; }

        public String getSourceTypeEnum() { return sourceTypeEnum; }
        public void setSourceTypeEnum(String sourceTypeEnum) { this.sourceTypeEnum = sourceTypeEnum; }

        public Long getTenantId() { return tenantId; }
        public void setTenantId(Long tenantId) { this.tenantId = tenantId; }

        public String getTransactionTypeEnum() { return transactionTypeEnum; }
        public void setTransactionTypeEnum(String transactionTypeEnum) { this.transactionTypeEnum = transactionTypeEnum; }

        public Long getUserId() { return userId; }
        public void setUserId(Long userId) { this.userId = userId; }

        public String getUserName() { return userName; }
        public void setUserName(String userName) { this.userName = userName; }
    }
}
