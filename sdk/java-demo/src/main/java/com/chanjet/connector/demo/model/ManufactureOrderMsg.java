package com.chanjet.connector.demo.model;

import com.chanjet.connector.sdk.BaseMessage;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.Data;
import lombok.EqualsAndHashCode;
import java.util.List;

@Data
@EqualsAndHashCode(callSuper = true)
public class ManufactureOrderMsg extends BaseMessage {

    @JsonProperty("bizContent")
    private BizContent bizContent;

    @Data
    public static class BizContent {
        private String externalCode;
        private String orderCode;
        private String orderDate;
        private String state;
        private List<Detail> details;
    }

    @Data
    public static class Detail {
        private String inventoryCode;
        private Double quantity;
        private String unit;
    }
}
