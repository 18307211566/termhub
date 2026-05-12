# termhub

局域网多设备终端汇聚工具。在 Windows 上打开 SSH / 串口 / Telnet / Raw TCP / 本地 Shell，通过内嵌 SSH server 让多人同时接入同一个会话——看到一样的输出，谁都能输入。

参考 [Upterm](https://github.com/owenthereal/upterm)，但 termhub 提供原生 GUI 配置面板，支持串口与 Telnet 等嵌入式调试常用上联，默认面向局域网协作场景。

## 快速上手

1. 下载最新 MSI 安装包，安装并启动 termhub
2. 在 GUI 中点击"新建会话"，选择上联类型并填写参数
3. 局域网内其他设备执行：

```bash
ssh <会话名>@<termhub主机IP> -p 2222
# 密码为创建会话时设定的密码
```

## 支持的上联类型

| 类型 | 说明 |
|---|---|
| Serial | 串口（COM），支持波特率/数据位/校验/停止位/流控/EOL 全参数 |
| SSH | SSH 客户端，密码认证 |
| Telnet | Telnet 客户端 |
| Raw TCP | 原始 TCP 连接 |
| Local Shell | 本地 PTY（PowerShell / cmd / WSL / 自定义命令） |
| Loopback | 内置环回（测试用） |

## 核心特性

- **完全合流共享**：所有下联看一致输出，任何下联都能写入
- **自动重连**：上联断开后自动退避重连（Serial/SSH/Telnet/RawTCP），下联会话保持不断
- **多会话**：同时挂多个上联，下联用户用 `ssh <session_name>@host -p 2222` 选频道
- **托盘常驻**：关主窗口缩到托盘，服务持续运行
- **配置持久化**：TOML 落盘，密码字段经 Windows DPAPI 加密
- **实时流量**：GUI 实时展示每个下联的字节流量
- **Kick / 流量计数**：可单独踢人，实时显示接入用户与字节量
- **安全提示**：默认监听 `0.0.0.0:2222` 时 GUI 顶部黄色警示，一键切回环回

## 默认监听 0.0.0.0:2222

termhub 默认监听所有网卡。**局域网内任何人知道密码即可接入。**

- GUI 顶部黄色 Banner 会提示风险
- 点击"改为仅本机"可切换到 `127.0.0.1:2222`（仅本机可连）
- 也可在"服务器"区块手动修改监听地址并热重载

## 项目结构

```
termhub/
├─ crates/
│  ├─ termhub-core/      # Hub / SessionMgr / Driver trait / 配置 / 状态机 / DPAPI
│  ├─ termhub-drivers/   # Serial / SSH / Telnet / RawTcp / LocalShell / Loopback
│  ├─ termhub-sshd/      # 下联 SSH server (russh)
│  ├─ termhub-app/       # Tauri 主二进制 + React 前端
│  └─ termhub-cli/       # CLI 工具（供 AI Agent 或脚本调用 HTTP API）
└─ docs/superpowers/     # 设计文档与开发计划
```

## 环境准备

| 依赖 | 最低版本 | 说明 |
|---|---|---|
| Rust | stable (1.79+) | 通过 [rustup](https://rustup.rs/) 安装，项目根目录 `rust-toolchain.toml` 会自动选择 stable 频道 |
| Node.js | 18+ | 用于构建 React 前端 |
| npm | 9+ | 随 Node.js 安装 |

首次安装 Rust 工具链后，确保已安装 Tauri CLI：

```bash
cargo install tauri-cli --version "^2"
```

## 开发

```bash
# 安装前端依赖（首次或 package.json 变更后）
cd crates/termhub-app/ui && npm install && cd ../../..

# 开发模式（前端 hot reload + Tauri 窗口）
cargo tauri dev

# 运行测试
cargo test --workspace
```

## 编译打包

### 构建 MSI 安装包

```bash
# 确保前端依赖已安装
cd crates/termhub-app/ui && npm install && cd ../../..

# 构建（自动编译 Rust + 打包前端 + 生成 MSI）
cargo tauri build
```

构建完成后，MSI 安装包位于：

```
target/release/bundle/msi/termhub_0.1.0_x64_en-US.msi
```

### 仅编译 Rust（不打包）

```bash
# Release 模式
cargo build --workspace --release

# Debug 模式
cargo build --workspace
```

# 调试

```bash
# 设置环境变量  
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path  

# 启动 termhub-app 
cargo run -p termhub-app 
```


编译产物：

| 二进制 | 路径 |
|---|---|
| termhub（桌面应用） | `target/release/termhub.exe` |
| termhub-cli（命令行工具） | `target/release/termhub-cli.exe` |

### 常见问题

- **`npm ci` 报错找不到 `package-lock.json`**：先运行 `npm install` 生成锁文件
- **编译时 `termhub.exe` 被占用**：关闭正在运行的 TermHub 桌面应用
- **`cargo: command not found`**：重新打开终端或运行 `source ~/.cargo/env`

## CLI 命令行工具

termhub-cli 通过 HTTP API 操作 TermHub，适合脚本和 AI Agent 调用。TermHub 桌面应用需先运行。

```bash
# 查看所有会话
termhub-cli session list

# 创建会话
termhub-cli session create <名称> <密码> --upstream loopback
termhub-cli session create <名称> <密码> --upstream serial --port COM3 --baud 115200
termhub-cli session create <名称> <密码> --upstream ssh --host 192.168.1.10 --user admin --password secret
termhub-cli session create <名称> <密码> --upstream local-shell --command powershell

# 停止 / 重启 / 删除会话
termhub-cli session stop <名称>
termhub-cli session restart <名称>
termhub-cli session delete <名称>

# 查看会话客户端
termhub-cli client list <会话名>

# 查看服务器信息
termhub-cli server info

# 列出串口
termhub-cli serial-ports
```

所有命令支持 `--json` 输出机器可读的 JSON 格式。

## 许可证

MIT OR Apache-2.0
