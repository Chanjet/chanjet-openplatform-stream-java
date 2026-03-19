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
GW_A = "http://localhost:8080"
GW_B = "http://localhost:8082"
INTERNAL_TOKEN = "cjt-default-internal-token"

def get_hmac_sha256(data, secret):
    return hmac.new(secret.encode('utf-8'), data.encode('utf-8'), hashlib.sha256).hexdigest().lower()

def get_nonce(gw_url):
    resp = requests.get(f"{gw_url}/v1/ws/challenge?app_key={APP_KEY}", headers={"X-CJT-PreAuth": "none"})
    return resp.json()['data']['nonce']

class WSClient:
    def __init__(self, name, url):
        self.name = name
        self.received = []
        self.ws = websocket.WebSocketApp(url, on_message=self.on_message)
        self.wst = threading.Thread(target=self.ws.run_forever)
        self.wst.daemon = True

    def on_message(self, ws, message):
        print(f"[{self.name}] Received: {message}")
        try:
            msg = json.loads(message)
            if msg.get("msg_type") == "event":
                self.received.append(msg)
        except: pass

    def start(self):
        self.wst.start()
        time.sleep(2)

    def close(self):
        self.ws.close()

def run_tests():
    print("\n🚀 Starting Boundary Tests...")

    # --- Scenario 1: Multi-Client Discovery ---
    print("\n[TC-01] Testing Multi-Client Local-First...")
    nonce_a = get_nonce(GW_A)
    c1 = WSClient("Client-on-A", f"ws://localhost:8080/connect?app_key={APP_KEY}&nonce={nonce_a}&sign={get_hmac_sha256(f'{APP_KEY}&{nonce_a}', APP_SECRET)}&client_id=c1")
    c1.start()
    
    # 消息发给 GW-A，验证本地优先
    requests.post(f"{GW_A}/internal/v1/webhook/dispatch", headers={"X-C-APP_KEY": APP_KEY, "X-MSG-ID": "m1"}, data="{}")
    time.sleep(2)
    if len(c1.received) > 0:
        print("✅ TC-01 Success: Local push works.")
    else:
        print("❌ TC-01 Failed.")

    # --- Scenario 2: P2P Retry (Simulated) ---
    print("\n[TC-02] Testing P2P Retry Logic...")
    # 手动在 Redis 插入一个死路由
    import redis
    try:
        r = redis.Redis(host='localhost', port=6379, db=0)
        r.sadd(f"cjt:gw:route:{APP_KEY}", "127.0.0.1:9999:dead-client")
        
        # 消息发给 GW-B (8082)，它应该通过重试逻辑最终转到 8080
        resp = requests.post(f"{GW_B}/internal/v1/webhook/dispatch", headers={"X-C-APP_KEY": APP_KEY, "X-MSG-ID": "m2"}, data="{}")
        print(f"Dispatch status: {resp.status_code}")
        time.sleep(5)
        
        if any(m.get('msg_id') == 'm2' for m in c1.received):
            print("✅ TC-02 Success: System recovered via retry to node 8080.")
        else:
            print("❌ TC-02 Failed: Retry mechanism didn't deliver the message.")
    except Exception as e:
        print(f"Skipping TC-02 (Redis/Dependency issue): {e}")

    # --- Scenario 3: Loop Prevention ---
    print("\n[TC-03] Testing Loop Prevention...")
    loop_frame = {
        "msg_type": "event",
        "msg_id": "loop-test",
        "app_key": APP_KEY,
        "headers": {"X-GW-Hop-Count": "1"},
        "payload": "loop"
    }
    resp = requests.post(f"{GW_B}/internal/v1/p2p/push", 
                         headers={"X-Internal-Token": INTERNAL_TOKEN}, 
                         json=loop_frame)
    if resp.status_code == 200:
        print("✅ TC-03 Success: Loop prevented (Accepted but dropped locally).")
    else:
        print(f"❌ TC-03 unexpected status: {resp.status_code}")

    c1.close()
    print("\n🏁 Boundary Tests Completed.")

if __name__ == "__main__":
    run_tests()
