from aiohttp import web
import json
import asyncio
import uuid
import hmac
import hashlib
import time
from datetime import datetime

# Global state
MOCK_STATE = {
    "ping_fail_count": 0,
    "active_ws_clients": set(),
}

def hmac_sha256(data, key):
    return hmac.new(key.encode('utf-8'), data.encode('utf-8'), hashlib.sha256).hexdigest()

async def handle_token(request):
    return web.json_response({
        "result": True,
        "value": {
            "accessToken": f"mock_at_sb_{uuid.uuid4().hex[:8]}",
            "expiresIn": 20 # Short expiry for Step 4 testing
        }
    })

async def handle_resend(request):
    app_key = request.headers.get("appKey", "unknown")
    print(f"DEBUG: Received AppTicket resend request for {app_key}")
    
    # Broadcast Ticket via WS
    async def broadcast():
        await asyncio.sleep(1)
        ticket_msg = json.dumps({
            "msg_type": "APP_TICKET",
            "msgType": "APP_TICKET",
            "msgId": uuid.uuid4().hex,
            "appKey": app_key,
            "time": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            "bizContent": {
                "appTicket": f"mock_ticket_ws_{uuid.uuid4().hex[:6]}"
            }
        })
        print(f"📡 [MOCK] Broadcasting AppTicket to {len(MOCK_STATE['active_ws_clients'])} clients")
        for ws in list(MOCK_STATE["active_ws_clients"]):
            try:
                await ws.send_str(ticket_msg)
                print(f"✅ [MOCK] Pushed AppTicket via WS")
            except Exception as e:
                print(f"❌ [MOCK] WS push failed: {e}")

    asyncio.create_task(broadcast())
    return web.json_response({"code": "200", "message": "success"})

async def handle_spec(request):
    return web.json_response({
        "openapi": "3.0.1",
        "info": {"title": "Mock Platform API", "version": "1.0.0"},
        "paths": {
            "/v1/mock/ping": {"get": {"responses": {"200": {"description": "OK"}}}},
            "/v1/mock/secure": {"get": {"responses": {"200": {"description": "OK"}}}}
        }
    })

async def handle_ping(request):
    print(f"📥 [MOCK] Received PING: {request.method} {request.path}")
    return web.json_response({"status": "ok"})

async def handle_secure(request):
    token = request.headers.get("openToken", "none")
    print(f"📥 [MOCK] Received SECURE request with token: {token[:10]}...")
    return web.json_response({"status": "verified", "token_used": token})

async def handle_challenge(request):
    app_key = request.query.get("app_key", "unknown")
    nonce = uuid.uuid4().hex[:16]
    print(f"📥 [MOCK] Challenge request for {app_key} -> {nonce}")
    return web.json_response({
        "data": {
            "nonce": nonce
        }
    })

async def handle_ws(request):
    app_key = request.query.get("app_key")
    client_id = request.query.get("client_id")
    print(f"🔌 [MOCK] WS Connection attempt: app_key={app_key}, client_id={client_id}")
    
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    
    print(f"✅ [MOCK] WS Handshake successful")
    MOCK_STATE["active_ws_clients"].add(ws)
    
    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.TEXT:
                try:
                    data = json.loads(msg.data)
                    if data.get("msg_type") == "ping":
                        await ws.send_str(json.dumps({"msg_type": "pong"}))
                except:
                    pass
    finally:
        MOCK_STATE["active_ws_clients"].remove(ws)
        print("🔌 [MOCK] WS Client disconnected")
    return ws

async def run_server():
    app = web.Application()
    app.router.add_post("/v1/common/auth/selfBuiltApp/generateToken", handle_token)
    app.router.add_post("/auth/appTicket/resend", handle_resend)
    app.router.add_get("/v1/common/openapi/spec", handle_spec)
    app.router.add_get("/v1/mock/ping", handle_ping)
    app.router.add_get("/v1/mock/secure", handle_secure)
    app.router.add_get("/v1/ws/challenge", handle_challenge)
    app.router.add_get("/connect", handle_ws)
    
    runner = web.AppRunner(app)
    await runner.setup()
    site = web.TCPSite(runner, '127.0.0.1', 9299)
    await site.start()
    print("🚀 Mock Server (Full) running on http://127.0.0.1:9299")
    await asyncio.Event().wait()

if __name__ == "__main__":
    try:
        asyncio.run(run_server())
    except KeyboardInterrupt:
        pass
