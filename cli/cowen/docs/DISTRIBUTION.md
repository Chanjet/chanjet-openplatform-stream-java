# Cowen 跨平台客户端分发与安装规范 (Distribution & Installation Specification)

本规范整理并终结了 Cowen 命令行工具在 **macOS**, **Linux**, 与 **Windows** 三大操作系统上的打包格式、文件布局、分发方式以及安装启用逻辑的对齐设计。

---

## 📌 一、分发维度概览与差异对比

| 维度 | macOS (`macos-aarch64`) | Linux (`linux-x86_64` / `aarch64`) | Windows (`windows-x86_64`) |
| :--- | :--- | :--- | :--- |
| **打包命令** | `make package-macos-aarch64` | `make package-linux` | `make package-windows-x86_64-cross` (ZIP)<br>`make package-windows-x86_64-setup-cross` (Setup) |
| **产物格式** | 双击安装包：`.pkg` | 压缩归档包：`.tar.gz` | 自解压安装包：`.exe` (Setup)<br>压缩归档包：`.zip` (ZIP) |
| **默认安装路径** | 核心组件系统级：`/usr/local/bin/`<br>用户级目录：`~/.cowen/` | 用户级目录：`~/.cowen/bin/` | 用户级目录：`~/.cowen\bin\` |
| **配置目录** | 用户级：`~/.cowen/` | 用户级：`~/.cowen/` | 用户级：`~/.cowen\` |
| **插件加载路径** | 用户级插件目录：`~/.cowen/plugins/` | 用户级插件目录：`~/.cowen/plugins/` | 用户级插件目录：`~/.cowen\plugins\` |
| **系统插件路径** | 用户级系统插件目录：`~/.cowen/system_plugins/` | 用户级系统插件目录：`~/.cowen/system_plugins/` | 用户级系统插件目录：`~/.cowen\system_plugins\` |
| **守护进程机制** | `launchd` 用户 Agents 启动项 | `systemd` / 用户自启动配置 | Windows 注册表启动项 (Registry Run Key) |
| **组件可选性** | **支持**组件勾选 (Core 强制，AI / MCP 插件可选) | **不支持**可选 (全量打包，一键全量安装) | **不支持**可选 (根据构建类型全量嵌入安装) |
| **分发插件明细** | **系统插件**：<br>- `cowen_wasm_auth_selfbuilt.wasm`/`.bundle`<br>- `cowen_wasm_auth_storeapp.wasm`/`.bundle`<br>- `cowen_wasm_auth_custom.wasm`<br>**外部插件**：<br>- `libcowen_search_embedding` (AI)<br>- `libcowen_search_embedding.bundle`<br>- `cowen-mcp-plugin` (MCP)<br>- `cowen-mcp-plugin.bundle` | **系统插件**：<br>- `cowen_wasm_auth_selfbuilt.wasm`/`.bundle`<br>- `cowen_wasm_auth_storeapp.wasm`/`.bundle`<br>- `cowen_wasm_auth_custom.wasm`<br>**外部插件**：<br>- `libcowen_search_embedding` (AI)<br>- `libcowen_search_embedding.bundle`<br>- `cowen-mcp-plugin` (MCP)<br>- `cowen-mcp-plugin.bundle` | **系统插件**：<br>- `cowen_wasm_auth_selfbuilt.wasm`/`.bundle`<br>- `cowen_wasm_auth_storeapp.wasm`/`.bundle`<br>- `cowen_wasm_auth_custom.wasm`<br>**外部插件**：<br>- `libcowen_search_embedding.exe` (AI, 64位)<br>- `cowen_search_embedding.dll` (AI, 32位)<br>- `libcowen_search_embedding.bundle` / `cowen_search_embedding.bundle`<br>- `cowen-mcp-plugin.exe` (MCP)<br>- `cowen-mcp-plugin.bundle` |

---

## 🛠️ 二、各操作系统详细分发与安装逻辑

### 1. macOS 分发逻辑 (`.pkg` 形式)

macOS 分发采用了标准的组件化安装设计，主要利用系统的 `pkgbuild` 和 `productbuild` 构建工具链：

* **构建依赖**：`macos-aarch64` (编译二进制) ➡️ `build-system-plugins` (编译 WebAssembly 鉴权插件) ➡️ `impl-package-macos` (组件打包与合成)。
* **子组件拆分**：
  * **Core 核心组件** (`cowen-core.pkg`)：包含 `cowen` (CLI 客户端)、`cowen-daemon` (后台守护进程) 和 `cowen-uninstall` (卸载脚本)，并附带 WebAssembly 系统插件。
  * **AI 插件组件** (`cowen-plugin-ai.pkg`)：包含 `libcowen_search_embedding` 及其动态签名描述文件 `.bundle`。
  * **MCP 插件组件** (`cowen-plugin-mcp.pkg`)：包含 `cowen-mcp-plugin` 及其 `.bundle`。
* **合成与 GUI 自定义**：
  * 通过 `productbuild --synthesize` 生成组件定义 `Distribution.xml`。
  * 修改 `Distribution.xml` 启用自定义设置，将 `Core` 标记为强制且不可取消，将 `AI` 和 `MCP` 插件标记为可由用户自由选择是否安装（默认勾选）。
* **安装脚本行为**：
  * **Core 安装后脚本** ([postinstall](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/dist_assets/macos/scripts_core/postinstall))：探测图形界面当前登录的真实非 root 用户，将系统插件分发至 `~/.cowen/system_plugins/` 并修正所有权。随后停用旧版守护进程，并通过运行 `cowen daemon service install` 向 `launchd` 注册自启动项并将其拉起。
  * **Plugin 安装后脚本** ([postinstall](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/dist_assets/macos/scripts_plugin/postinstall))：在公共中转暂存区（`/usr/local/share/cowen/staging/`）动态移走成功安装的插件文件，将其移动至 `~/.cowen/plugins/`，恢复真实用户权限，并针对非 `.bundle` 文件调用 `cowen plugins enable <name>` 激活该插件。

---

### 2. Linux 分发逻辑 (`.tar.gz` 形式)

Linux 环境下为保障解压即用和免管理员 root 权限，采用了完全用户态的 Tarball 结构：

* **构建产物**：打包为 `cowen-v<VERSION>-linux-<ARCH>.tar.gz`。
* **物理结构**：
  * **根目录**：`cowen`、`cowen-daemon` 核心二进制、自解压安装脚本 `install.sh`、`README.txt`、`CHANGELOG.md` 及使用文档目录 `usage/`。
  * **`lib/` 目录**：存放外部插件二进制及其 `.bundle` 签名文件。
  * **`system_plugins/` 目录**：存放 WebAssembly 鉴权插件。
* **安装脚本 ([install.sh](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/dist_assets/linux/install.sh)) 行为**：
  1. 创建 `$HOME/.cowen/bin` 目录，将核心二进制 `cowen` 与 `cowen-daemon` 拷贝至此，并赋予可执行权限 (`chmod +x`)。
  2. 自动检查用户的 Shell 配置文件 (`.zshrc` / `.bashrc`)，在末尾动态追加 `PATH` 环境变量，将 `$HOME/.cowen/bin` 注册到系统执行路径。
  3. 通过 `cowen completion --install` 初始化 Shell 自动补全。
  4. 清理残留并执行 `cowen daemon service install` 注册基于 Linux 用户级别的自启动守护服务。
  5. **动态扫描插件目录**：拷贝 `lib/*` 到 `~/.cowen/plugins/`。然后扫描该目录，自动排除 `.bundle` 签名文件，将其他插件文件标记为可执行，并通过 `cowen plugins enable` 在后台进行服务注册和动态加载。
  6. 拷贝 `system_plugins/` 下的 WebAssembly 插件到 `~/.cowen/system_plugins/`。

---

### 3. Windows 分发逻辑 (`.exe` Setup / `.zip` 形式)

Windows 分发提供了两种主流模式以适应不同的部署环境：

#### 3.1 Setup 自解压安装包模式 (`.exe` - 官方分发首选)
由 [cowen_setup](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen_setup/) 箱体编译为一个静态无外部依赖的安装器：
* **静态资源嵌入**：在构建时（由 `build.rs` 引导），利用 `include_bytes!` 宏将核心的 `cowen.exe`、`cowen-daemon.exe`、Wasm 插件，以及（若构建环境存在）`libcowen_search_embedding.exe` 和 `cowen-mcp-plugin.exe` 直接以二进制字节数组形式硬编码嵌入到 `cowen_setup.exe` 中。
* **运行提取逻辑**：
  1. 用户双击运行后，将核心二进制释放至 `%USERPROFILE%\.cowen\bin\`，将 Wasm 系统插件释放至 `%USERPROFILE%\.cowen\system_plugins\`。
  2. 如果存在 AI 或 MCP 插件的嵌入字节，释放至 `%USERPROFILE%\.cowen\plugins\` 目录中。
  3. 执行自启动安装指令，并通过运行 `cowen plugins enable` 激活对应的外部插件。
  4. 修改 Windows 注册表的 User 级别 `Path` 环境变量，使 `cowen` 命令在全局 CMD/PowerShell 终端中即刻生效。

#### 3.2 绿色压缩包模式 (`.zip` - 适用于跨平台打包)
支持在 macOS/Linux 等交叉编译环境下快速成包：
* **归档结构**：直接打包 `cowen.exe`、`cowen-daemon.exe`、`system_plugins\`、使用文档以及外部扩展 `.exe` 和 `.bundle`（例如 `cowen-mcp-plugin.exe` 等）到根目录，并附带 `install.ps1` 部署脚本。
* **部署脚本 ([install.ps1](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/dist_assets/windows/install.ps1)) 行为**：
  1. 将核心 CLI 拷贝至 `%USERPROFILE%\.cowen\bin\` 并为当前用户追加注册 PATH 环境变量。
  2. 创建 `%USERPROFILE%\.cowen\plugins\`，**动态检测**包中附带的插件 `.exe`/`.dll` 并拷贝过去，依次调用 `cowen plugins enable` 完成自动装载。
  3. 将 Wasm 插件拷贝至系统插件路径下。
  4. 自动修改当前用户的 PowerShell Profile 文件（如 `Microsoft.PowerShell_profile.ps1`），注入 Tab 自动补全脚本。
  5. 重启守护进程，并在后台以静默窗口形式自动启动 `cowen-daemon`。

---

## 🔒 三、敏感信息过滤与安全防范

* 配置文件与分发脚本中禁止包含任何硬编码凭据（如 `<APP_TICKET>`、`<SECRET>` 等）。
* 插件分发与加载严格依赖安全签名验证。在 `make build-plugins` 阶段，会通过 `cowen-signer` 签名工具利用官方开发证书私钥为编译出的外部插件生成 `.bundle` 签名文件。
* 客户端启动时仅会加载数字签名校验通过（或者开发调试模式授权）的合规动态扩展插件，防止劫持。
