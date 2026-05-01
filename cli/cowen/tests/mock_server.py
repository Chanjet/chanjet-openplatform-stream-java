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
    "webhook_messages": [],
    "token_expiration_mode": False, # If True, tokens will expire very quickly or return error
}

def hmac_sha256(data, key):
    return hmac.new(key.encode('utf-8'), data.encode('utf-8'), hashlib.sha256).hexdigest()

# --- Auth Handlers ---

async def handle_self_built_token(request):
    """Self-Built mode token generation"""
    return web.json_response({
        "result": True,
        "value": {
            "accessToken": f"mock_at_sb_{uuid.uuid4().hex[:8]}",
            "expiresIn": 3600 
        }
    })

async def handle_oauth2_token(request):
    """OAuth2 mode token exchange (code/refresh)"""
    data = await request.post()
    grant_type = data.get("grant_type", "authorization_code")
    print(f"📥 [MOCK] OAuth2 Token Request: grant_type={grant_type}")
    
    return web.json_response({
        "access_token": f"mock_at_oa2_{uuid.uuid4().hex[:8]}",
        "refresh_token": f"mock_rt_oa2_{uuid.uuid4().hex[:8]}",
        "expires_in": 3600,
        "refresh_expires_in": 86400
    })

async def handle_store_app_token(request):
    """Store-App mode token exchange"""
    print(f"📥 [MOCK] Store-App Token Request")
    return web.json_response({
        "result": {
            "appAccessToken": f"mock_at_sa_{uuid.uuid4().hex[:8]}",
            "expiresIn": 3600
        }
    })

async def handle_resend(request):
    """Trigger AppTicket push for Self-Built"""
    app_key = request.headers.get("appKey", "unknown")
    print(f"   [MOCK] AppTicket Resend Requested for {app_key}. Proactively pushing to all WS clients...")
    
    # Broadcast APP_TICKET to all active WS connections
    for ws in list(MOCK_STATE["active_ws_clients"]):
        if not ws.closed:
            try:
                await ws.send_json({
                    "msg_type": "APP_TICKET",
                    "msgType": "APP_TICKET",
                    "msgId": uuid.uuid4().hex,
                    "appKey": app_key,
                    "time": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                    "biz_content": {
                        "app_ticket": f"mock_ticket_{uuid.uuid4().hex[:8]}",
                        "appTicket": f"mock_ticket_{uuid.uuid4().hex[:8]}"
                    },
                    "bizContent": {
                        "appTicket": f"mock_ticket_{uuid.uuid4().hex[:8]}"
                    }
                })
            except Exception as e:
                print(f"   [ERROR] WS push failed: {e}")
    
    return web.json_response({"code": "200", "message": "success"})

# --- OpenAPI Spec ---

async def handle_spec(request):
    return web.json_response({
        "openapi": "3.0.1",
        "info": {"title": "Mock Platform API (Full Capability)", "version": "1.0.0"},
        "paths": {
            "/v1/mock/ping": {"get": {"responses": {"200": {"description": "OK"}}}},
            "/v1/mock/secure": {"get": {"responses": {"200": {"description": "OK"}}}},
            "/v1/mock/admin": {"post": {"responses": {"200": {"description": "OK"}}}}
        }
    })

async def handle_ping(request):
    return web.json_response({"status": "ok", "timestamp": time.time()})

async def handle_secure(request):
    token = request.headers.get("openToken") or request.headers.get("Authorization", "none")
    print(f"📥 [MOCK] Received SECURE request with token: {token[:10]}...")
    return web.json_response({"status": "verified", "token_used": token})

# --- WebSocket Handlers ---

async def handle_challenge(request):
    app_key = request.query.get("app_key", "unknown")
    nonce = uuid.uuid4().hex[:16]
    return web.json_response({"data": {"nonce": nonce}})

async def handle_ws(request):
    app_key = request.query.get("app_key")
    client_id = request.query.get("client_id")
    print(f"🔌 [MOCK] WS Connection: {app_key} ({client_id})")
    
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    MOCK_STATE["active_ws_clients"].add(ws)
    
    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                if data.get("msg_type") == "ping":
                    await ws.send_str(json.dumps({"msg_type": "pong"}))
    finally:
        MOCK_STATE["active_ws_clients"].remove(ws)
        print(f"🔌 [MOCK] WS Disconnected: {app_key}")
    return ws

async def handle_webhook_sink(request):
    """Receive forwarded messages from Cowen Daemon"""
    body = await request.read()
    data = json.loads(body)
    print(f"📥 [MOCK SINK] Received forwarded webhook: {data.get('msg_type')}")
    MOCK_STATE["webhook_messages"].append(data)
    return web.json_response({"status": "received"})

async def handle_get_webhook_messages(request):
    return web.json_response(MOCK_STATE["webhook_messages"])

async def handle_broadcast(request):
    """Trigger a custom WS broadcast for testing"""
    data = await request.json()
    msg_type = data.get("msg_type", "DATA_PUSH")
    payload = data.get("payload", {})
    
    count = 0
    for ws in list(MOCK_STATE["active_ws_clients"]):
        if not ws.closed:
            await ws.send_json({
                "msg_type": msg_type,
                "msgId": uuid.uuid4().hex,
                "biz_content": payload
            })
            count += 1
    return web.json_response({"broadcast_to": count})

async def handle_kill_connections(request):
    """Force close all active WS connections to simulate network drop or server restart"""
    count = 0
    for ws in list(MOCK_STATE["active_ws_clients"]):
        if not ws.closed:
            await ws.close()
            count += 1
    print(f"🔪 [MOCK] Force closed {count} WS connections.")
    return web.json_response({"killed": count})

# --- Server Start ---

async def run_server():
    app = web.Application()
    
    # Auth Endpoints
    app.router.add_post("/v1/common/auth/selfBuiltApp/generateToken", handle_self_built_token)
    app.router.add_post("/auth/appTicket/resend", handle_resend)
    app.router.add_post("/v1/common/auth/oauth2/token", handle_oauth2_token)
    app.router.add_post("/oauth2/token", handle_oauth2_token)
    app.router.add_post("/auth/appAuth/getAppAccessToken", handle_store_app_token)
    
    # OpenAPI Endpoints
    app.router.add_get("/v1/common/openapi/spec", handle_spec)
    app.router.add_get("/v1/mock/ping", handle_ping)
    app.router.add_get("/v1/mock/secure", handle_secure)
    
    # WebSocket Endpoints
    app.router.add_get("/v1/ws/challenge", handle_challenge)
    app.router.add_get("/connect", handle_ws)
    
    # Webhook & Control
    app.router.add_post("/webhook_sink", handle_webhook_sink)
    app.router.add_get("/control/webhooks", handle_get_webhook_messages)
    app.router.add_post("/control/broadcast", handle_broadcast)
    app.router.add_post("/control/kill_connections", handle_kill_connections)
    
    runner = web.AppRunner(app)
    await runner.setup()
    site = web.TCPSite(runner, '127.0.0.1', 9299)
    await site.start()
    print("🚀 Mock Server (Full Capability) running on http://127.0.0.1:9299")
    await asyncio.Event().wait()

if __name__ == "__main__":
    try:
        asyncio.run(run_server())
    except KeyboardInterrupt:
        pass
