import os
import hmac
import hashlib
import requests
import websocket
import threading
import time
import json

APP_KEY = "3qMYSkA5"
APP_SECRET = os.getenv("APP_SECRET", "")
GW_HTTP_URL = "http://localhost:8080"
GW_WS_URL = "ws://localhost:8080"

def get_hmac_sha256(data, secret):
    return hmac.new(secret.encode('utf-8'), data.encode('utf-8'), hashlib.sha256).hexdigest().lower()

def verify_flow():
    print("\n--- [Step 1: Fetching Nonce] ---")
    pre_auth = get_hmac_sha256(APP_KEY, APP_SECRET)[:16]
    headers = {"X-CJT-PreAuth": pre_auth}
    
    try:
        resp = requests.get(f"{GW_HTTP_URL}/v1/ws/challenge?app_key={APP_KEY}", headers=headers)
        print(f"Response Status: {resp.status_code}")
        print(f"Response Body: {resp.text}")
        data = resp.json()
        if data.get('code') != 'GW-0000':
            print(f"Server returned error code: {data.get('code')}")
            return
        nonce = data['data']['nonce']
        print(f"Success! Got Nonce: {nonce}")
    except Exception as e:
        print(f"Failed to get nonce: {e}")
        return

    print("\n--- [Step 2: Connecting WebSocket] ---")
    sign = get_hmac_sha256(f"{APP_KEY}&{nonce}", APP_SECRET)
    ws_url = f"{GW_WS_URL}/connect?app_key={APP_KEY}&nonce={nonce}&sign={sign}&client_id=verify-client"
    
    received_messages = []

    def on_message(ws, message):
        print(f"WS Received RAW: {message}")
        try:
            msg = json.loads(message)
            if msg.get("msg_type") == "event":
                received_messages.append(msg)
                # Send ACK
                ack = {"msg_id": msg["msg_id"], "code": 200, "message": "success"}
                ws.send(json.dumps(ack))
                print(f"ACK Sent for {msg['msg_id']}")
        except Exception as ex:
            print(f"Error parsing message: {ex}")

    def on_error(ws, error):
        print(f"WS Error: {error}")

    def on_close(ws, close_status_code, close_msg):
        print("WS Connection Closed")

    ws = websocket.WebSocketApp(ws_url, on_message=on_message, on_error=on_error, on_close=on_close)
    wst = threading.Thread(target=ws.run_forever)
    wst.daemon = True
    wst.start()

    print("Connecting...")
    time.sleep(3) # Wait for handshake
    
    if not ws.sock or not ws.sock.connected:
        print("❌ WebSocket connection failed. Check server logs.")
        return
    print("✅ WebSocket Connected.")

    print("\n--- [Step 3: Dispatching Webhook] ---")
    webhook_headers = {
        "X-C-APP_KEY": APP_KEY,
        "X-MSG-ID": "verify-msg-" + str(int(time.time())),
        "Content-Type": "application/json"
    }
    webhook_body = json.dumps({"biz_data": "hello_from_gemini"})
    
    dispatch_resp = requests.post(f"{GW_HTTP_URL}/internal/v1/webhook/dispatch", 
                                 headers=webhook_headers, data=webhook_body)
    print(f"Dispatch Response: {dispatch_resp.status_code} - {dispatch_resp.text}")

    time.sleep(3) # Wait for delivery
    
    print("\n--- [Step 4: Final Results] ---")
    if len(received_messages) > 0:
        print("🎉 SUCCESS: Webhook successfully bridged to WebSocket!")
        print(f"Payload received in WS: {received_messages[0].get('payload')}")
    else:
        print("💀 FAILED: Message not received by WS client.")

    ws.close()

if __name__ == "__main__":
    verify_flow()
