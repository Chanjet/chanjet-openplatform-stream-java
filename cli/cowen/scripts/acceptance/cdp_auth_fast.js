// cdp_auth_fast.js
const { spawn } = require('child_process');
const http = require('http');

const CHROME_DEBUG_PORT = 9222;

function get(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => resolve(data));
    }).on('error', reject);
  });
}

async function findChromeTab() {
  try {
    const jsonStr = await get(`http://127.0.0.1:${CHROME_DEBUG_PORT}/json`);
    const tabs = JSON.parse(jsonStr);
    const existing = tabs.find(t => t.url.includes('chanjet.com') && t.webSocketDebuggerUrl);
    if (existing) return existing;
    const page = tabs.find(t => t.type === 'page' && t.webSocketDebuggerUrl);
    return page;
  } catch (e) {
    console.error('无法连接 Chrome，请确保 Chrome 已开启 9222 端口调试。', e.message);
    process.exit(1);
  }
}

function sendCDP(ws, method, params = {}) {
  const id = Math.floor(Math.random() * 100000);
  return new Promise((resolve) => {
    const handler = (event) => {
      const data = JSON.parse(event.data);
      if (data.id === id) {
        ws.removeEventListener('message', handler);
        resolve(data.result);
      }
    };
    ws.addEventListener('message', handler);
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function main() {
  if (!process.env.OAUTH2_APP_KEY || !process.env.OAUTH2_MESSAGE_SECRET) {
    console.error('❌ 错误: 缺少必要的环境变量 OAUTH2_APP_KEY 或 OAUTH2_MESSAGE_SECRET！请通过 run_all_acceptance.sh 启动。');
    process.exit(1);
  }

  console.log('🚀 启动 cowen init...');
  
  const child = spawn('cowen', [
    'init', 
    '-p', 'oauth2_app', 
    '--app-mode', 'oauth2',
    '--app-key', process.env.OAUTH2_APP_KEY,
    '--encrypt-key', process.env.OAUTH2_MESSAGE_SECRET
  ], {
    env: { ...process.env, COWEN_SKIP_BROWSER: 'true' }
  });

  child.stdout.on('data', (data) => {
    // Strip ANSI escape codes to prevent color codes like \u001b[0m from appending to URL
    const line = data.toString().replace(/\u001b\[[0-9;]*m/g, '');
    console.log(`[cowen stdout] ${line.trim()}`);
    
    const match = line.match(/(https:\/\/market\.chanjet\.com\/user\/v2\/authorize[a-zA-Z0-9$_.+!*'(),;&%=\-~?/:@#]+)/);
    if (match) {
      const authUrl = match[1];
      console.log(`🎯 捕获 URL: ${authUrl}`);
      triggerCDP(authUrl);
    }
  });

  child.stderr.on('data', (data) => {
    const line = data.toString().replace(/\u001b\[[0-9;]*m/g, '');
    console.error(`[cowen stderr] ${line.trim()}`);
  });

  child.on('close', (code) => {
    console.log(`[cowen] 进程退出，退出码: ${code}`);
    process.exit(code);
  });

  async function triggerCDP(url) {
    const tab = await findChromeTab();
    if (!tab) {
      console.error('未找到可用标签页');
      return;
    }

    console.log(`🔌 连接 CDP: ${tab.webSocketDebuggerUrl}`);
    const ws = new WebSocket(tab.webSocketDebuggerUrl);

    ws.onopen = async () => {
      console.log('✅ CDP 已连接');
      await sendCDP(ws, 'Page.enable');
      await sendCDP(ws, 'Runtime.enable');

      console.log('🌐 导航...');
      await sendCDP(ws, 'Page.navigate', { url });

      // 使用状态机轮询页面状态并执行对应操作
      let attempts = 0;
      let state = 'unknown';
      let orgSelected = false;
      let appSelected = false;

      while (attempts < 25) {
        const stateRes = await sendCDP(ws, 'Runtime.evaluate', {
          expression: `(() => {
            if (document.querySelector('input[type="checkbox"]') && Array.from(document.querySelectorAll('label, span, div')).some(el => el.innerText && el.innerText.includes('阅读并同意'))) {
              return 'login';
            }
            if (Array.from(document.querySelectorAll('div, span, p, a, h3, h4')).some(e => e.innerText && e.innerText.trim() === '马嘟嘟中心三')) {
              return 'selectOrg';
            }
            if (Array.from(document.querySelectorAll('div, span, p, a, h3, h4, figure, figcaption')).some(e => e.innerText && e.innerText.trim() === '好业财')) {
              return 'selectApp';
            }
            const authButton = Array.from(document.querySelectorAll('button, a, input[type="button"]')).find(b => {
              const text = b.innerText || b.value || '';
              return text.includes('授权') || text.includes('同意') || text.includes('确定') || text.includes('确认');
            });
            if (authButton) {
              return 'confirmAuth';
            }
            return 'loading';
          })()`
        });
        state = stateRes.result.value;
        console.log(`⏳ 当前页面状态: ${state} (第 ${attempts+1} 次尝试)`);

        if (state === 'login') {
          console.log('🔒 检测到登录页，自动登录中...');
          await sendCDP(ws, 'Runtime.evaluate', {
            expression: `(() => {
              const checkboxes = document.querySelectorAll('input[type="checkbox"]');
              checkboxes.forEach(cb => { if (!cb.checked) cb.click(); });
              const agreeLabel = Array.from(document.querySelectorAll('label, span, div')).find(el => el.innerText && el.innerText.includes('阅读并同意'));
              if (agreeLabel) agreeLabel.click();

              const buttons = Array.from(document.querySelectorAll('button, input[type="button"], input[type="submit"]'));
              const loginBtn = buttons.find(b => {
                const txt = b.innerText || b.value || '';
                return txt.includes('登录') || txt.includes('Log In');
              });
              if (loginBtn) loginBtn.click();
            })()`
          });
        } else if (state === 'selectOrg' && !orgSelected) {
          console.log('🏢 检测到选择企业页，正在选择 马嘟嘟中心三...');
          orgSelected = true;
          await sendCDP(ws, 'Runtime.evaluate', {
            expression: `(() => {
              const orgEl = Array.from(document.querySelectorAll('div, span, p, a, h3, h4')).find(e => 
                e.innerText && e.innerText.trim() === '马嘟嘟中心三'
              );
              if (orgEl) {
                const target = orgEl.closest('a') || orgEl.closest('button') || orgEl.closest('div') || orgEl;
                target.click();
              }
            })()`
          });
        } else if (state === 'selectApp' && !appSelected) {
          console.log('📱 检测到选择应用页，正在选择 好业财...');
          appSelected = true;
          await sendCDP(ws, 'Runtime.evaluate', {
            expression: `(() => {
              const appEl = Array.from(document.querySelectorAll('div, span, p, a, h3, h4, figure, figcaption')).find(e => 
                e.innerText && e.innerText.trim() === '好业财'
              );
              if (appEl) {
                const target = appEl.closest('a') || appEl.closest('button') || appEl.closest('div') || appEl;
                target.click();
              }
            })()`
          });
        } else if (state === 'confirmAuth') {
          console.log('🖱️ 检测到确认授权页，点击确认授权按钮...');
          const authResult = await sendCDP(ws, 'Runtime.evaluate', {
            expression: `(() => {
              const buttons = Array.from(document.querySelectorAll('button, a, input[type="button"]'));
              const authButton = buttons.find(b => {
                const text = b.innerText || b.value || '';
                return text.includes('授权') || text.includes('同意') || text.includes('确定') || text.includes('确认');
              });
              if (authButton) {
                authButton.click();
                return 'Authorized Clicked';
              }
              return 'Auth Button Not Found';
            })()`
          });
          console.log(`👉 授权结果: ${authResult.result.value}`);
          break;
        }

        await new Promise(r => setTimeout(r, 1000));
        attempts++;
      }

      console.log('⏳ 等待最终回调...');
      await new Promise(r => setTimeout(r, 3000));
      ws.close();
    };
  }
}

main();
