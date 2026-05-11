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
│  └─ termhub-app/       # Tauri 主二进制 + React 前端
└─ docs/superpowers/     # 设计文档与开发计划
```

## 开发

需要 Rust 1.79+ 和 Node.js 18+。

```bash
# 前端依赖
cd crates/termhub-app/ui && npm ci && cd ../../..

# 开发模式（前端 hot reload + Tauri 窗口）
cargo run -p termhub-app

# 运行测试
cargo test --workspace

# 构建 MSI
cargo build -p termhub-app --release
```

## 许可证

MIT OR Apache-2.0
