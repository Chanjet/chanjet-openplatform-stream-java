import hmac
import hashlib
import requests
import websocket
import threading
import time
import json
import os

APP_KEY = "3qMYSkA5"
APP_SECRET = os.getenv("APP_SECRET", "")
GW_A_URL = "http://localhost:8080"
GW_B_URL = "http://localhost:8082"

def get_hmac_sha256(data, secret):
    return hmac.new(secret.encode('utf-8'), data.encode('utf-8'), hashlib.sha256).hexdigest().lower()

def verify_p2p():
    print("\n--- [Step 1: Get Nonce from GW-A] ---")
    pre_auth = get_hmac_sha256(APP_KEY, APP_SECRET)[:16]
    resp = requests.get(f"{GW_A_URL}/v1/ws/challenge?app_key={APP_KEY}", headers={"X-CJT-PreAuth": pre_auth})
    nonce = resp.json()['data']['nonce']
    print(f"Nonce: {nonce}")

    print("\n--- [Step 2: Client Connects to GW-A (8080)] ---")
    sign = get_hmac_sha256(f"{APP_KEY}&{nonce}", APP_SECRET)
    ws_url = f"ws://localhost:8080/connect?app_key={APP_KEY}&nonce={nonce}&sign={sign}&client_id=p2p-test-client"
    
    received_messages = []
    def on_message(ws, message):
        print(f"WS Received from GW-A: {message}")
        try:
            msg = json.loads(message)
            if msg.get("msg_type") == "event":
                received_messages.append(msg)
        except: pass

    ws = websocket.WebSocketApp(ws_url, on_message=on_message)
    wst = threading.Thread(target=ws.run_forever)
    wst.daemon = True
    wst.start()
    time.sleep(3)
    print("WebSocket Connected to GW-A.")

    print("\n--- [Step 3: Send Webhook to GW-B (8082)] ---")
    webhook_headers = {
        "X-C-APP_KEY": APP_KEY,
        "X-MSG-ID": "p2p-msg-" + str(int(time.time())),
        "Content-Type": "application/json"
    }
    # 消息发给 8082，但连接在 8080，触发 P2P
    dispatch_resp = requests.post(f"{GW_B_URL}/internal/v1/webhook/dispatch", 
                                 headers=webhook_headers, data=json.dumps({"p2p": "is_working"}))
    print(f"GW-B Dispatch Response: {dispatch_resp.status_code}")

    time.sleep(3)
    
    print("\n--- [Step 4: Verification Result] ---")
    if len(received_messages) > 0:
        print("✅ SUCCESS: P2P Forwarding Verified!")
        print(f"Payload received via P2P: {received_messages[0].get('payload')}")
    else:
        print("❌ FAILED: P2P Forwarding did not reach the target instance.")

    ws.close()

if __name__ == "__main__":
    verify_p2p()
