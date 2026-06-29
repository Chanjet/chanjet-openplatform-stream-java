# Java SDK 演示项目 (sdk-java-demo)

本演示项目展示了如何在 Spring Boot 应用中集成 `connector-sdk-java`，并利用 `MessageDispatcher` 优雅地处理畅捷通开放平台的各种业务推送消息。

## 🌟 核心演示内容

1.  **Spring 集成**：演示如何通过 `@Service` 和 `@PostConstruct` 声明式地启动 SDK 客户端。
2.  **系统消息处理**：快速订阅 `APP_TICKET`（票据）和 `TEMP_AUTH_CODE`（授权码）。
3.  **好系列业务适配**：展示 `onAppNotice` 无损监听机制，轻松获取销货单（GoodsIssue）等业务数据及企业 ID 上下文。
4.  **自定义扩展**：演示如何注册 T+ 生产加工单（`manufactureOrderMsg`）等非标准 POJO。

## 🛠️ 如何运行

### 1. 配置参数

Demo 项目现已支持通过 `.env` 文件快速注入环境变量。
您只需在 `sdk/java/demo` 目录下复制一份 `.env.example`，重命名为 `.env`，填入您的凭证即可：

```env
APP_KEY=your_app_key
APP_SECRET=your_app_secret_32_chars
# GATEWAY_URL=wss://open.chanjet.com/gateway # 可选，默认指向生产环境
```

*(底层原理：通过集成的 `spring-dotenv`，系统启动时会自动将 `.env` 变量映射到 `application.yml` 中的 `${APP_KEY}` 占位符。)*

### 2. 编译并启动
确保本地已安装 Java 21+ 和 Maven。

```bash
# 1. 首先安装 SDK 到本地仓库 (在项目根目录或 sdk/java 目录执行)
mvn clean install -DskipTests

# 2. 启动 Demo (在 sdk/java-demo 目录执行)
mvn spring-boot:run
```

## 📂 项目结构说明

- `model/`：存放自定义的业务 POJO（需继承 `BaseMessage`）。
- `DemoService.java`：**核心配置类**。在此处注册监听器并启动 `GatewayClient`。
- `DemoApplication.java`：Spring Boot 启动入口。

## 💡 开发建议

ISV 在实际开发时，可以直接参考 `DemoService.java` 中的注册逻辑。对于结构固定的畅捷通消息，建议优先使用 SDK 内置的 `onAppNotice`、`onAppTicket` 等快捷方法，以获得最佳的代码可读性和维护性。
