use crate::protocol::{
    EventFrame, AppTicketMessage, EntAuthCodeMessage, 
    OrderStatusMessage, AppNoticeMessage
};
use crate::crypto;
use std::collections::HashMap;
use serde_json::Value;
use anyhow::{Result, anyhow};
use std::sync::Arc;

pub type MessageHandler = Arc<dyn Fn(Value) -> bool + Send + Sync>;

pub struct MessageDispatcher {
    handlers: HashMap<String, MessageHandler>,
    fallback_handler: Option<MessageHandler>,
}

impl MessageDispatcher {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            fallback_handler: None,
        }
    }

    pub fn set_fallback_handler(&mut self, handler: MessageHandler) {
        self.fallback_handler = Some(handler);
    }

    pub fn register(&mut self, msg_type: &str, handler: MessageHandler) {
        self.handlers.insert(msg_type.to_string(), handler);
    }

    pub fn on_app_ticket<F>(&mut self, handler: F) 
    where F: Fn(AppTicketMessage) -> bool + Send + Sync + 'static {
        self.register("APP_TICKET", Arc::new(move |val| {
            if let Ok(msg) = serde_json::from_value::<AppTicketMessage>(val) {
                handler(msg)
            } else {
                false
            }
        }));
    }

    pub fn on_ent_auth_code<F>(&mut self, handler: F) 
    where F: Fn(EntAuthCodeMessage) -> bool + Send + Sync + 'static {
        self.register("TEMP_AUTH_CODE", Arc::new(move |val| {
            if let Ok(msg) = serde_json::from_value::<EntAuthCodeMessage>(val) {
                handler(msg)
            } else {
                false
            }
        }));
    }

    pub fn on_order_status<F>(&mut self, handler: F) 
    where F: Fn(OrderStatusMessage) -> bool + Send + Sync + 'static {
        self.register("PAY_ORDER_SUCCESS", Arc::new(move |val| {
            if let Ok(msg) = serde_json::from_value::<OrderStatusMessage>(val) {
                handler(msg)
            } else {
                false
            }
        }));
    }

    pub fn on_app_notice<F>(&mut self, bo_name: &str, trans_type: Option<&str>, handler: F) 
    where F: Fn(AppNoticeMessage) -> bool + Send + Sync + 'static {
        let key = match trans_type {
            Some(tt) => format!("APP_NOTICE:{}:{}", bo_name, tt),
            None => format!("APP_NOTICE:{}", bo_name),
        };
        self.register(&key, Arc::new(move |val| {
            if let Ok(msg) = serde_json::from_value::<AppNoticeMessage>(val) {
                handler(msg)
            } else {
                false
            }
        }));
    }

    pub fn dispatch(&self, frame: &EventFrame, decrypt_key: &str) -> Result<bool> {
        let mut root: Value = serde_json::from_str(&frame.payload)
            .map_err(|e| anyhow!("Failed to parse payload: {}", e))?;
// ... (rest of the code remains the same)

        let mut _payload_json = frame.payload.clone();

        // 1. Auto Decrypt
        if let Some(encrypt_msg) = root.get("encryptMsg").and_then(|v| v.as_str()) {
            _payload_json = crypto::aes_decrypt(encrypt_msg, decrypt_key)?;
            root = serde_json::from_str(&_payload_json)?;
        }

        // 2. Route
        let mut msg_type = root.get("msgType")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();

        // Handle APP_NOTICE composite keys
        if msg_type == "APP_NOTICE" {
            if let Some(biz) = root.get("bizContent") {
                let bo_name = biz.get("boName").and_then(|v| v.as_str()).unwrap_or("");
                let trans_type = biz.get("transactionTypeEnum").and_then(|v| v.as_str()).unwrap_or("");
                
                let full_key = format!("APP_NOTICE:{}:{}", bo_name, trans_type);
                let bo_key = format!("APP_NOTICE:{}", bo_name);
                
                if self.handlers.contains_key(&full_key) {
                    msg_type = full_key;
                } else if self.handlers.contains_key(&bo_key) {
                    msg_type = bo_key;
                }
            }
        }

        if let Some(handler) = self.handlers.get(&msg_type) {
            // Pre-process: inject headers into JSON if possible
            if let Some(obj) = root.as_object_mut() {
                let headers_val = serde_json::to_value(&frame.headers)?;
                obj.insert("headers".to_string(), headers_val);
            }
            Ok(handler(root))
        } else if let Some(fallback) = &self.fallback_handler {
            if let Some(obj) = root.as_object_mut() {
                let headers_val = serde_json::to_value(&frame.headers)?;
                obj.insert("headers".to_string(), headers_val);
            }
            Ok(fallback(root))
        } else {
            tracing::warn!("No handler for msgType: {}. Skipping.", msg_type);
            Ok(true)
        }
    }
}
