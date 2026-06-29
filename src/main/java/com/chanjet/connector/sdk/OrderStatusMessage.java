package com.chanjet.connector.sdk;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.List;

/**
 * 畅捷通标准：订单支付成功消息 (PAY_ORDER_SUCCESS)。
 */
public class OrderStatusMessage extends BaseMessage {

    @JsonProperty("bizContent")
    private BizContent bizContent;

    public BizContent getBizContent() { return bizContent; }
    public void setBizContent(BizContent bizContent) { this.bizContent = bizContent; }

    public static class BizContent {
        private String orderNo;
        private String orgId;
        private OrderDetail detail;

        public String getOrderNo() { return orderNo; }
        public void setOrderNo(String orderNo) { this.orderNo = orderNo; }

        public String getOrgId() { return orgId; }
        public void setOrgId(String orgId) { this.orgId = orgId; }

        public OrderDetail getDetail() { return detail; }
        public void setDetail(OrderDetail detail) { this.detail = detail; }
    }

    public static class OrderDetail {
        private String orderNo;
        private Double orderTotal;
        private Integer orderType;
        private Double payTotal;
        private String paidTime;
        private String createdTime;
        private Long userId;
        private Long orgId;
        private List<OrderItem> orderItems;

        // Getters and Setters
        public String getOrderNo() { return orderNo; }
        public void setOrderNo(String orderNo) { this.orderNo = orderNo; }

        public Double getOrderTotal() { return orderTotal; }
        public void setOrderTotal(Double orderTotal) { this.orderTotal = orderTotal; }

        public Integer getOrderType() { return orderType; }
        public void setOrderType(Integer orderType) { this.orderType = orderType; }

        public Double getPayTotal() { return payTotal; }
        public void setPayTotal(Double payTotal) { this.payTotal = payTotal; }

        public String getPaidTime() { return paidTime; }
        public void setPaidTime(String paidTime) { this.paidTime = paidTime; }

        public String getCreatedTime() { return createdTime; }
        public void setCreatedTime(String createdTime) { this.createdTime = createdTime; }

        public Long getUserId() { return userId; }
        public void setUserId(Long userId) { this.userId = userId; }

        public Long getOrgId() { return orgId; }
        public void setOrgId(Long orgId) { this.orgId = orgId; }

        public List<OrderItem> getOrderItems() { return orderItems; }
        public void setOrderItems(List<OrderItem> orderItems) { this.orderItems = orderItems; }
    }

    public static class OrderItem {
        private Double payPrice;
        private Long productId;
        private String startDate;
        private String endDate;
        private String amountInfo;

        public Double getPayPrice() { return payPrice; }
        public void setPayPrice(Double payPrice) { this.payPrice = payPrice; }

        public Long getProductId() { return productId; }
        public void setProductId(Long productId) { this.productId = productId; }

        public String getStartDate() { return startDate; }
        public void setStartDate(String startDate) { this.startDate = startDate; }

        public String getEndDate() { return endDate; }
        public void setEndDate(String endDate) { this.endDate = endDate; }

        public String getAmountInfo() { return amountInfo; }
        public void setAmountInfo(String amountInfo) { this.amountInfo = amountInfo; }
    }
}
