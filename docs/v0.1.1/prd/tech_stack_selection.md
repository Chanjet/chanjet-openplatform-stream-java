# 畅捷通 Openplatform Stream Connector CLI - 技术选型方案 (v0.1.1)

结合 PRD v0.1.1 版本的强制指标（Agent-Friendly、跨平台单文件驻留、多租户及高频发隔离代理等），在已敲定大技术栈为 **Go (Golang)** 的前提下，进一步从各个底层业务模块锚定具体开源依赖库及实现骨架的最佳选型。

---

## 一、 核心底座与开发基干 (Core Infrastructure)

### 1. 命令行解析与生命周期管控
- **推荐框架：`spf13/cobra` 搭配 `spf13/viper`**
  - **入选理由**：Go 语言中绝对的 CLI 霸主（Kubernetes、Docker 的底层同款）。它原生支持极具条理的多级子命令嵌套（如 `cli api get ...`）；自带卓越的 Flag 解析绑定以及自动生成标准 Shell 补全脚本。
  - **VIPER 优势**：`viper` 完美承担多数据源优先级仲裁（自动融合命令行 Flag、OS 环境变量 ENV 以及脱敏后的内存 Config），完全满足 PRD 规定的“全面支持非交互式”入参诉求。

### 2. 跨平台后台驻留进程 (Daemon)
- **推荐扩展：`kardianos/service`**
  - **入选理由**：直接帮您把 Go 执行档秒变为后台受控的 OS 级别本地守护程序（Systemd, Launchd, Windows Service）。它抽象了跨三大系统的底层差异，且提供统一的 `Install`、`Start`、`Status`、`Stop` 接口，这能立刻兑现 PRD 里的生命周期干预及“开机自启”需求。

---

## 二、 关键业务引擎 (Business Engines)

### 3. 多租户加密凭据安全宿主区 (Secret Runtime)
- **推荐引擎：`zalando/go-keyring`**（优先选择）+ **`modernc.org/sqlite`**（降级）
  - **入选理由**：既然安全规范不让 `appSecret` 明文落盘，`go-keyring` 能够实现无需用户输入主密码即可无缝存取跨平台的本地钥匙串管理器（Keychain / Secret Service）。
  - **纯 Go SQLite** (`modernc.org/sqlite`)：如果在极端无桌面的 Linux 发行版（如 Docker）内拿不到秘钥挂载点，可以把配置结构体以及后续 Webhook 的 **DLQ(死信队列)** 统一存入这个**没有任何 CGO 依赖**（完全 C-Free）的纯 Go 嵌入式数据库中，加密存储后安全且对交叉编译零痛点。

### 4. 日志解耦管控塔 (Observability Matrix)
- **推荐框架：`uber-go/zap` 搭配 `natefinch/lumberjack`**
  - **入选理由**：
    - PRD 规定要求 100% 结构化脱敏输出，`zap` 是全世界公认的高性能结构化 JSON 第一库，且自带分级路由。
    - PRD 规定防爆磁盘物理分隔，`lumberjack` 负责精准监听四大日志物理体的大小（`Max-Size`）、保存天数（`Max-Age`）与自动切分无缝打包，两者结合犹如坦克加护甲。

---

## 三、 网络吞吐防线 (Networking & Routing)

### 5. 透明 HTTP 代理劫持与 API 拦截器 (Proxy Cache)
- **推荐库：标准库原生 `net/http/httputil` & `net/http`**
  - **入选理由**：你无需上庞大的微服务网关，Go 语言自带的 `httputil.NewSingleHostReverseProxy` 就是实现透明正反向代理最高效的代码。挂载自定义 `RoundTripper`（即 HttpClient 中间件），可以在请求打给畅捷通平台之前，神不知鬼不觉地塞入动态 Hash 签名与 OAuth Token。

### 6. 动态 REST 逆向匹配器 (Router API IndexTree)
- **推荐库：标准库 `net/http` 1.22+ 或纯净的 Trie-Tree 包**
  - **入选理由**：当用户传入 `cli api post /v1/orders/123/cancel` 时，依靠 Go 1.22 的增强正则匹配标准库，能够零三方库反解出 OpenAPI 模板串 `/v1/orders/{id}/cancel`，性能与兼容性兼得。

### 7. 流式链接对接者 (Stream Consumer)
- **推荐依赖：本工作空间配套的内部 Go SDK (`open-streaming-connector` Client SDK)**
  - **入选理由**：既然我们生态内已经配备了专门针对 Connector Server 研发的成套 SDK，那就果断摒弃泛用的开源 WebSocket/SSE 轮子。直接以 Module 形式引用本工作空间产出的 Go SDK，不仅能开箱即用地获得针对我们私有协议高度定制的端到端长链接维系、心跳重连及流控能力封装，还能保证 CLI 在后续迭代中始终与 Connector Server 服务端保持同频共振，降维打击开发难度。

---

## 四、 高阶核心壁垒 (Advanced Agent-First)

### 8. 端侧 Embedding 与相似度计算模型 (Semantic Search)
- **推荐套件：ONNX Runtime (`yalue/onnxruntime_go`) + 纯原生数学余弦切片包**
  - **入选理由**：为了将大模型的找接口幻觉化为乌有：
    1. 在初始化拉取 OpenAPI 列表后，调用捆绑在可执行文件侧的微型 `.onnx` 模型产生 Float32 稠密向量。
    2. 无需引入 Milvus、Chroma 等拖累起步极慢的服务，甚至也不需要任何外部库，直接用不到五十行纯 Go 代码手写计算两个切片的余弦算子并发循环排序即可找到最近似接口文档。
  - **避坑预警**：这是全局**唯一可能依赖 CGO (因为内含 C++)** 的地方，在交叉构建流水线时需要小心处理头文件依赖问题（详见技术可行性分析）。

---

## 五、 DevOps 与自动化发版标准清单

### 9. 自动化发版管理体系
- **推荐基建：`GoReleaser`**
  - **入选理由**：应对单机交叉编译 AI 内核困难的痛点。你只需要写一个 `.goreleaser.yaml`，推送至 GitHub 后，系统全自动打下 Mac、Linux、Win 所有组合芯片的压缩包（甚至是直接打出 Docker 镜像以及向系统包管理器拉取 Formula / PKGBUILD 补丁文件），让这个极客形态的 CLI 从诞生到更新具备全球化分发能力。

---

## 六、 传输防线：彻底阻绝中间人 (MITM) 劫持方案
针对网络黑客、恶意内网抓包软件（诸如 Charles / Fiddler）等针对 HTTPS 进行“信任根伪造”中间人攻击（Man-in-the-Middle），导致核心网络传输中敏感信息泄露的严重安全隐患，我们必须在代码底端确立至少涵盖以下手段的双层加固体系：

### 10. 零服务端改造的防剥离架构
由于我们作为纯端侧的 CLI 工具，绝对**受限于既有服务端的认证规约（不可要求后端配合增加全新的验证字段如 Nonce）**，因此对抗嗅探劫持的动作必须 100% 收敛在本地：

    1. **可配置的商业 CA 白名单 (Configurable CA Whitelist - 默认不启用)**：为了兼顾普通开发者的极简开箱体验与某些极高安全风控企业（金融/军工）的死锁诉求，我们不再把 CA 名字池作为全局强制的写死屏障。而是将其作为一个高级 CLI 配置项下发（例如支持用户执行 `cli config set trusted_cas=DigiCert,GlobalSign`）。**在默认状态下此功能不启用**，底层仅依赖操作系统本身的原生可信根库来证明基本安全；一旦高级用户通过设置显式激活了该名单，底层拦截器便会在握手首跳时立刻启动严格的名字池比对，精准绞杀诸如 Fiddler 等伪造的 "DO_NOT_TRUST_XXX" 私生假证书！
    2. **受限泛域名与域根强制嵌套 (Restricted Wildcard & Root Binding)**：即使那张证书因为出内鬼导致由真正的商业大厂（比如 DigiCert）签发，但底层网卡对于握手验证毕竟异常宽容。代码强制切割服务端传回的证书实体 `DNSNames` (SANs) 进行二次验证：
       - **拦截宽泛通配符**：一旦证书中出现的星号 `*` 企图泛指顶级域名（例如 `*.com` 或 `*.cn`），直接拒绝并物理熔断连接。
       - **锁死 `chanjet.com` 官方域根**：证书中授权的合法域名，必须严丝合缝地以 `.chanjet.com` 或精确的 `chanjet.com` 收尾（且允许 `*.chanjet.com` 的企业型通配符）。防止黑客通过这几大合法商业 CA 去正常花钱买一张乱七八糟的野路子仿冒域名证书（如 `chanjet.api-fakeweb.com`）来骗过校验网。
  - **全局防御效能**：
    1. **一劳永逸且免动态构建**：因为 `DigiCert`、`GlobalSign` 等几家垄断大厂的名字百年不遇会改，只要将这个可信发行池写进 Go 的全局数组里兜底。未来哪怕服务端换了供应商甚至换公钥，只要他们还是在名单里任何一家正规 CA 商那买的证书，CLI 的源码乃至发布流水线都**完全不用修改**！实现了最完美的 0 运维 100% 续航。
    2. **降维杀灭假客户端劫持**：通过“你是这世界上这几家大名鼎鼎的正规发证局派发的吗？” + “你这张正规发证局颁出的证书真的是为 `chanjet.com` 这个名门正派申请的吗？” —— 这两组拷问的交叉印证，在不牺牲哪怕 1% 的运维精力的前提下，把绝大多数企业内网安全设备以及个人开发者桌面的破壳网关拦截得连门都进不去！

- **防线二：防线物理收敛的短周期凭证隔离 (Short-lived Token)**
  - **技术解法**：根据开放平台最新确认的规范，**既然服务端本身不支持 mTLS 双向认证，同时又对 `appSecret` 取消了公共参数强制传输的要求**，我们端侧的安全哲学就变得极其单纯。`appSecret` 本体仅在获取或刷新 `openToken` 的极低频环节于本地密存参与验算；随后所有的高频 `api` 数据请求交互全部强制仅携带含有明确生命时效的临牌 `openToken` 上路。
  - **防御效能**：依托**防线一**极其刚硬的 TLS 公钥验签（Certificate Pinning），中间人黑客本来就失去了劫持伪造的舞台。假使在极端变异环境下（如运行时发往网卡的真实包被 Dump），由于网络信封外壳里只有时效极短的 `openToken`（而能重置一切生命周期的主根密钥 `appSecret` 始终安静地躺在本地保险箱中绝不见光），攻击者顺手牵羊拿到的仅仅是一张转头就会作废的通行门票，从物理与时间窗口双维度直接瘫痪了严重越权的危机。
