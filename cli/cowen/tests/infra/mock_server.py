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
    "active_ws_clients": {}, # keyed by client_id
    "webhook_messages": [],
    "token_expiration_mode": False, # If True, tokens will expire very quickly or return error
    "token_expires_in": 3600,
    "force_error": None, # e.g. {"code": "401", "message": "invalid_token"}
    "webhook_sink_status": 200,
    "webhook_delay_ms": 0,
}

def hmac_sha256(data, key):
    return hmac.new(key.encode('utf-8'), data.encode('utf-8'), hashlib.sha256).hexdigest()

# --- Auth Handlers ---

async def handle_self_built_token(request):
    """Self-Built mode token generation - Non-deterministic to support fast rotation tests"""
    app_key = request.headers.get("appKey", "unknown")
    token_suffix = uuid.uuid4().hex[:8]
    return web.json_response({
        "result": True,
        "value": {
            "accessToken": f"mock_at_sb_{token_suffix}",
            "expiresIn": MOCK_STATE["token_expires_in"]
        }
    })

async def handle_generic_success(request):
    open_token = request.headers.get("openToken", "")
    app_key = request.headers.get("appKey", "")
    return web.json_response({
        "code": "200", 
        "message": "success", 
        "data": {
            "openToken": open_token,
            "appKey": app_key
        }
    })


async def handle_oauth2_token(request):
    """OAuth2 mode token exchange - Non-deterministic to support fast rotation tests"""
    data = await request.post()
    grant_type = data.get("grant_type", "authorization_code")
    client_id = data.get("client_id", "unknown")
    token_suffix = uuid.uuid4().hex[:8]
    print(f"📥 [MOCK] OAuth2 Token Request: grant_type={grant_type}, client_id={client_id}")
    
    return web.json_response({
        "access_token": f"mock_at_oa2_{grant_type}_{token_suffix}",
        "refresh_token": f"mock_rt_oa2_{token_suffix}",
        "expires_in": MOCK_STATE["token_expires_in"],
        "refresh_expires_in": 86400
    })

async def handle_store_app_token(request):
    """Store-App mode token exchange - Non-deterministic to support fast rotation tests"""
    try:
        data = await request.json()
    except:
        data = {}
    
    org_id = data.get("orgId") or data.get("org_id", "unknown")
    token_suffix = uuid.uuid4().hex[:4]
    print(f"📥 [MOCK] Store-App Token Request for Org: {org_id}")
    
    return web.json_response({
        "result": {
            "appAccessToken": f"mock_at_sa_{org_id}_{token_suffix}",
            "expiresIn": MOCK_STATE["token_expires_in"]
        }
    })

async def handle_rotate_token(request):
    """Force mock server to change the salt so next token requests yield different values"""
    MOCK_STATE["salt"] = uuid.uuid4().hex[:8]
    print(f"🔄 [MOCK] Token Salt Rotated: {MOCK_STATE['salt']}")
    return web.json_response({"result": True, "new_salt": MOCK_STATE["salt"]})

async def handle_push(request):
    """Platform trigger for proactive push"""
    app_key = request.headers.get("appKey", "unknown")
    active_count = len(MOCK_STATE["active_ws_clients"])
    print(f"   [MOCK] AppTicket Push Requested for {app_key}. Active WS Clients: {active_count}")
    
    # Broadcast APP_TICKET to all active WS connections
    for ws in list(MOCK_STATE["active_ws_clients"].values()):
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

async def handle_permanent_auth_code(request):
    """Store-App mode exchange: temp code -> permanent code"""
    try:
        data = await request.json()
        temp_code = data.get("tempAuthCode", "unknown")
    except:
        temp_code = "unknown"
        
    print(f"📥 [MOCK] Permanent Auth Code Exchange Request for: {temp_code}")
    
    # Extract orgId from temp_code to allow dynamic testing
    org_id = temp_code.replace("code_", "") if temp_code.startswith("code_") else "900000000"

    return web.json_response({
        "result": {
            "appName": "MockStoreApp",
            "appId": "12345",
            "permanentAuthCode": f"mock_opc_{uuid.uuid4().hex[:8]}",
            "orgId": org_id
        },
        "code": "200",
        "message": "success"
    })

async def handle_org_access_token(request):
    """Store-App mode exchange: permanent code -> org access token"""
    try:
        data = await request.json()
        opc = data.get("permanentAuthCode", "unknown")
    except:
        opc = "unknown"
    print(f"📥 [MOCK] Org Access Token Request for OPC: {opc}")
    return web.json_response({
        "result": {
            "accessToken": f"mock_at_oa2_permanent_code_{uuid.uuid4().hex[:8]}",
            "expireTime": MOCK_STATE["token_expires_in"]
        },
        "code": "200",
        "message": "success"
    })

async def handle_user_access_token(request):
    """Store-App mode exchange: user permanent code -> user access token"""
    try:
        data = await request.json()
        upc = data.get("userPermanentCode", "unknown")
    except:
        upc = "unknown"
    print(f"📥 [MOCK] User Access Token Request for UPC: {upc}")
    return web.json_response({
        "result": {
            "accessToken": f"mock_at_oa2_user_permanent_code_{uuid.uuid4().hex[:8]}",
            "expireTime": MOCK_STATE["token_expires_in"]
        },
        "code": "200",
        "message": "success"
    })

# --- OpenAPI Spec ---

async def handle_openapi_spec(request):
    return web.json_response({
        "paths": {
            "/v1/app/data/get": {"post": {}},
            "/v1/app/data/save": {"post": {}},
        }
    })

async def handle_get_interface_list(request):
    return web.json_response({
        "value": {
            "currentPage": 1,
            "totalPages": 1,
            "resultList": [
                {
                    "requestPath": "/v1/app/data/get",
                    "requestHttpMethod": "POST",
                    "interfaceName": "Get Data"
                },
                {
                    "requestPath": "/v1/app/data/save",
                    "requestHttpMethod": "POST",
                    "interfaceName": "Save Data"
                }
            ]
        }
    })

async def handle_spec(request):
    return web.json_response({
        "openapi": "3.0.1",
        "info": {"title": "Mock Platform API (Full Capability)", "version": "1.0.0"},
        "paths": {
            "/v1/mock/ping": {"get": {"responses": {"200": {"description": "OK"}}}},
            "/v1/mock/secure": {"get": {"responses": {"200": {"description": "OK"}}}},
            "/v1/mock/admin": {"post": {"responses": {"200": {"description": "OK"}}}},
            "/webhook_sink": {
                "post": {
                    "parameters": [
                        { "name": "Authorization", "in": "header", "required": True }
                    ],
                    "responses": {"200": {"description": "OK"}}
                }
            }
        }
    })

async def handle_ping(request):
    return web.json_response({"status": "ok", "timestamp": time.time()})

async def handle_secure(request):
    token = request.headers.get("openToken") or request.headers.get("Authorization", "none")
    print(f"📥 [MOCK] Received SECURE request with token: {token[:10]}...")
    return web.json_response({"status": "verified", "token_used": token})

# --- WebSocket Handlers ---

async def handle_nonce(request):
    app_key = request.query.get("app_key", "unknown")
    nonce = uuid.uuid4().hex[:16]
    return web.json_response({"data": {"nonce": nonce}})

async def handle_interface_list(request):
    """Mock platform API list for Self-Built mode (matched to client.rs expectation)"""
    return web.json_response({
        "result": True,
        "value": {
            "currentPage": 0,
            "totalPages": 1,
            "resultList": [
                {
                    "requestPath": "/webhook_sink",
                    "interfaceName": "Webhook Sink",
                    "openApi": {
                        "paths": {
                            "/webhook_sink": {
                                "post": {
                                    "parameters": [
                                        { "name": "Authorization", "in": "header", "required": True }
                                    ],
                                    "responses": {"200": {"description": "OK"}}
                                }
                            }
                        }
                    }
                },
                {
                    "requestPath": "/v1/mock/ping",
                    "interfaceName": "Mock Ping",
                    "openApi": {
                        "paths": {
                            "/v1/mock/ping": {
                                "get": {
                                    "responses": {"200": {"description": "OK"}}
                                }
                            }
                        }
                    }
                }
            ]
        }
    })

async def handle_ws(request):
    app_key = request.query.get("app_key", "unknown")
    client_id = request.query.get("client_id", "default")
    print(f"🔌 [MOCK] WS Connection: {app_key} ({client_id})")
    
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    
    # Protocol Support: Handle exclusive mode eviction
    is_exclusive = request.query.get("exclusive") == "true"
    if is_exclusive:
        # Evict all other clients for the same app_key
        to_evict = []
        for cid, old_ws in MOCK_STATE["active_ws_clients"].items():
            if cid != client_id and not old_ws.closed:
                # In a real system, we'd check if this cid belongs to the same app_key
                # For mock simplicity, we assume one app_key per test or check prefix
                if cid.startswith(app_key + "@") or cid == app_key:
                    to_evict.append(cid)
        
        for cid in to_evict:
            print(f"🔪 [MOCK] Exclusive Eviction: AppKey {app_key} requested exclusive access. Kicking client {cid}", flush=True)
            old_ws = MOCK_STATE["active_ws_clients"].pop(cid, None)
            if old_ws and not old_ws.closed:
                try:
                    await old_ws.close()
                except:
                    pass

    # Standard Support: Close old connection for same client_id if exists
    if client_id in MOCK_STATE["active_ws_clients"]:
        old_ws = MOCK_STATE["active_ws_clients"].get(client_id)
        if old_ws and not old_ws.closed:
            try:
                await old_ws.close()
            except:
                pass
            
    MOCK_STATE["active_ws_clients"][client_id] = ws
    
    try:
        async for msg in ws:
            if msg.type == web.WSMsgType.TEXT:
                data = json.loads(msg.data)
                if data.get("msg_type") == "ping":
                    await ws.send_str(json.dumps({"msg_type": "pong"}))
    finally:
        if MOCK_STATE["active_ws_clients"].get(client_id) == ws:
            del MOCK_STATE["active_ws_clients"][client_id]
        print(f"🔌 [MOCK] WS Disconnected: {app_key} ({client_id})")
    return ws

async def handle_webhook_sink(request):
    """Mock receiver for webhooks sent by the Sidecar/Daemon"""
    body = await request.read()
    try:
        data = json.loads(body)
    except:
        data = {"raw": body.decode('utf-8')}

    headers = dict(request.headers)
    print(f"📥 [MOCK SINK] Received forwarded webhook: {headers.get('Authorization', 'no-auth')}")
    MOCK_STATE["webhook_messages"].append({
        "body": data,
        "headers": headers
    })
    
    delay_ms = MOCK_STATE.get("webhook_delay_ms", 0)
    if delay_ms > 0:
        print(f"   [MOCK SINK] Delaying response for {delay_ms}ms...")
        await asyncio.sleep(delay_ms / 1000.0)

    status = MOCK_STATE.get("webhook_sink_status", 200)
    if status != 200:
        return web.Response(status=status, text=f"Mocking HTTP {status} Error")

    return web.json_response({"status": "received"})

async def handle_get_webhook_messages(request):
    return web.json_response(MOCK_STATE["webhook_messages"])

async def handle_broadcast(request):
    """Trigger a custom WS broadcast for testing (Supports Load Balancing)"""
    data = await request.json()
    msg_type = data.get("msgType") or data.get("msg_type", "DATA_PUSH")
    app_key = data.get("appKey") or data.get("app_key", "unknown")
    payload = data.get("payload") or data.get("bizContent") or {}
    mode = data.get("mode", "broadcast") # broadcast or lb
    
    # Prune dead connections first
    dead_keys = [k for k, ws in MOCK_STATE["active_ws_clients"].items() if ws.closed]
    for k in dead_keys:
        del MOCK_STATE["active_ws_clients"][k]
    
    active = list(MOCK_STATE["active_ws_clients"].items())
    if not active:
        return web.json_response({"broadcast_to": 0, "total_connections": 0})

    count = 0
    failed_keys = []
    
    msg_id = data.get("msgId") or data.get("msg_id") or uuid.uuid4().hex
    
    msg_payload = {
        "msgType": msg_type,
        "msg_type": msg_type,
        "msgId": msg_id,
        "appKey": app_key,
        "app_key": app_key,
        "headers": data.get("headers", {}),
        "bizContent": payload,
        "biz_content": payload,
        "time": datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    }

    if mode == "lb":
        import random
        cid, ws = random.choice(active)
        try:
            await ws.send_json(msg_payload)
            count = 1
        except Exception as e:
            print(f"   [LB] Send to {cid} failed: {e}")
            failed_keys.append(cid)
    else:
        for cid, ws in active:
            try:
                await ws.send_json(msg_payload)
                count += 1
            except Exception as e:
                print(f"   [BROADCAST] Send to {cid} failed: {e}")
                failed_keys.append(cid)
    
    for k in failed_keys:
        MOCK_STATE["active_ws_clients"].pop(k, None)
            
    return web.json_response({"broadcast_to": count, "mode": mode, "total_connections": len(MOCK_STATE["active_ws_clients"])})

async def handle_kill_connections(request):
    """Force close all active WS connections to simulate network drop or server restart"""
    count = 0
    for ws in list(MOCK_STATE["active_ws_clients"].values()):
        if not ws.closed:
            await ws.close()
            count += 1
    MOCK_STATE["active_ws_clients"].clear()
    print(f"🔪 [MOCK] Force closed {count} WS connections.")
    return web.json_response({"killed": count})

async def handle_clear_webhooks(request):
    """Clear accumulated webhook messages for test isolation"""
    count = len(MOCK_STATE["webhook_messages"])
    MOCK_STATE["webhook_messages"].clear()
    return web.json_response({"cleared": count})

async def handle_connection_count(request):
    """Return exact count of active WS connections (prunes dead ones first)"""
    # Force prune
    to_delete = []
    for k, ws in MOCK_STATE["active_ws_clients"].items():
        if ws.closed:
            to_delete.append(k)
    for k in to_delete:
        del MOCK_STATE["active_ws_clients"][k]
        
    clients = {k: True for k in MOCK_STATE["active_ws_clients"].keys()}
    return web.json_response({"count": len(clients), "clients": clients})
    
async def handle_config(request):
    """Update MOCK_STATE configuration"""
    data = await request.json()
    for k, v in data.items():
        if k in MOCK_STATE:
            MOCK_STATE[k] = v
    print(f"⚙️ [MOCK] Config updated: {data}")
    return web.json_response({"status": "ok", "current_state": {k: v for k, v in MOCK_STATE.items() if k != "active_ws_clients"}})

# --- Server Start ---

async def run_server():
    app = web.Application()
    
    # Auth Endpoints
    app.router.add_post("/v1/common/auth/selfBuiltApp/generateToken", handle_self_built_token)
    app.router.add_post("/auth/appTicket/resend", handle_push)
    app.router.add_get("/developer/api/apiPermissions/isv/open/getInterfaceList", handle_interface_list)
    app.router.add_post("/v1/common/auth/selfBuiltApp/generateNonce", handle_nonce)
    app.router.add_post("/v1/common/auth/oauth2/token", handle_oauth2_token)
    app.router.add_post("/oauth2/token", handle_oauth2_token)
    app.router.add_post("/auth/orgAuth/getPermanentAuthCode", handle_permanent_auth_code)
    app.router.add_post("/auth/orgAuth/getOrgAccessToken", handle_org_access_token)
    app.router.add_post("/auth/userAuth/getUserAccessToken", handle_user_access_token)
    app.router.add_post("/auth/appAuth/getAppAccessToken", handle_store_app_token)

    
    # OpenAPI Endpoints
    app.router.add_get("/v1/common/auth/openapi/spec", handle_openapi_spec)
    app.router.add_get("/v1/common/openapi/spec", handle_openapi_spec)
    app.router.add_get("/developer/api/apiPermissions/isv/open/getInterfaceList", handle_get_interface_list)
    app.router.add_post("/v1/app/data/get", handle_generic_success)
    app.router.add_post("/v1/app/data/save", handle_generic_success)
    
    app.router.add_get("/v1/mock/ping", handle_ping)
    app.router.add_get("/v1/mock/secure", handle_secure)
    
    # WebSocket Endpoints
    app.router.add_get("/v1/ws/challenge", handle_nonce)
    app.router.add_get("/connect", handle_ws)
    
    # Webhook & Control
    app.router.add_post("/webhook_sink", handle_webhook_sink)
    app.router.add_get("/control/webhooks", handle_get_webhook_messages)
    app.router.add_post("/control/broadcast", handle_broadcast)
    app.router.add_post("/control/kill_connections", handle_kill_connections)
    app.router.add_post("/control/clear_webhooks", handle_clear_webhooks)
    app.router.add_get("/control/connection_count", handle_connection_count)
    app.router.add_post("/control/config", handle_config)
    
    runner = web.AppRunner(app)
    await runner.setup()
    import os
    port = int(os.environ.get("MOCK_PORT", 9299))
    site = web.TCPSite(runner, '0.0.0.0', port)
    await site.start()
    print(f"🚀 Mock Server (Full Capability) running on http://127.0.0.1:{port}")
    await asyncio.Event().wait()

if __name__ == "__main__":
    try:
        asyncio.run(run_server())
    except KeyboardInterrupt:
        pass
