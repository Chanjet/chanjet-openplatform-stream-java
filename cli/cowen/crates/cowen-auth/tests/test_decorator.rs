use cowen_auth::RequestDecorator;
use serde_json::json;

#[test]
fn test_get_auth_headers_all() {
    let spec = json!({
        "paths": {
            "/v1/test": {
                "get": {
                    "parameters": [
                        {"name": "appKey", "in": "header"},
                        {"name": "appSecret", "in": "header"},
                        {"name": "openToken", "in": "header"}
                    ]
                }
            }
        }
    });

    let headers =
        RequestDecorator::get_auth_headers(&spec, "/v1/test", "get", "key123", "sec123", "tok123");

    assert_eq!(headers.len(), 3);
    assert!(headers.contains(&("appKey".to_string(), "key123".to_string())));
    assert!(headers.contains(&("appSecret".to_string(), "sec123".to_string())));
    assert!(headers.contains(&("openToken".to_string(), "tok123".to_string())));
}

#[test]
fn test_get_auth_headers_none() {
    let spec = json!({
        "paths": {
            "/v1/test": {
                "get": {
                    "parameters": []
                }
            }
        }
    });

    let headers =
        RequestDecorator::get_auth_headers(&spec, "/v1/test", "get", "key123", "sec123", "tok123");

    assert_eq!(headers.len(), 0);
}
