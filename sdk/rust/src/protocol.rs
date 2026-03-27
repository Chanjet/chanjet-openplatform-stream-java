use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFrame {
    pub msg_type: String,
    pub msg_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    pub app_key: String,
    pub target_client_id: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub payload: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckFrame {
    pub msg_id: String,
    pub code: i32,
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseMessage {
    pub id: Option<String>,
    pub msg_id: Option<String>,
    pub msg_type: String,
    pub app_key: String,
    pub app_id: Option<String>,
    #[serde(rename = "time")]
    pub timestamp: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTicketMessage {
    #[serde(flatten)]
    pub base: BaseMessage,
    #[serde(rename = "bizContent")]
    pub biz_content: AppTicketContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTicketContent {
    #[serde(rename = "appTicket")]
    pub app_ticket: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntAuthCodeMessage {
    #[serde(flatten)]
    pub base: BaseMessage,
    #[serde(rename = "bizContent")]
    pub biz_content: EntAuthCodeContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntAuthCodeContent {
    #[serde(rename = "tempAuthCode")]
    pub temp_auth_code: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusMessage {
    #[serde(flatten)]
    pub base: BaseMessage,
    #[serde(rename = "bizContent")]
    pub biz_content: OrderStatusContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusContent {
    #[serde(rename = "orderNo")]
    pub order_no: String,
    #[serde(rename = "orgId")]
    pub org_id: String,
    pub detail: OrderDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDetail {
    #[serde(rename = "payTotal")]
    pub pay_total: f64,
    #[serde(rename = "orderItems")]
    pub order_items: Vec<OrderItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    #[serde(rename = "productId")]
    pub product_id: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppNoticeMessage {
    #[serde(flatten)]
    pub base: BaseMessage,
    #[serde(rename = "bizContent")]
    pub biz_content: AppNoticeContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppNoticeContent {
    #[serde(rename = "boName")]
    pub bo_name: String,
    #[serde(rename = "transactionTypeEnum")]
    pub transaction_type_enum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPayload {
    #[serde(rename = "encryptMsg")]
    pub encrypt_msg: Option<String>,
    #[serde(rename = "msgType")]
    pub msg_type: String,
}
