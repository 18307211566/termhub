# termhub 设计文档

- **日期**：2026-05-08
- **状态**：Draft（待用户复核）
- **作者**：基于 README.md 的脑暴产出
- **代号**：termhub

---

## 1. 概述

### 1.1 产品定位

**termhub** 是一个运行在 Windows 上的本地原生应用，托盘常驻。它扮演两个角色：

1. **上联客户端**：主动打开一个或多个 SSH / 串口 / Telnet / Raw TCP / 本地 shell 会话；
2. **下联 SSH server**：把上述会话以 SSH 方式重新暴露出去，允许局域网内多台设备同时接入同一个会话，所有人**完全合流共享**——看到一样的输出，谁都能输入。

参考项目：[Upterm](https://github.com/owenthereal/upterm)（命令行版本），但 termhub 提供原生 GUI 配置面板，支持串口与 Telnet 等嵌入式调试常用上联，并默认面向局域网协作场景。

### 1.2 解决的核心问题

嵌入式开发与运维场景里，调试信息分散在串口 / SSH / Telnet 多种通道，多个工程师常常需要**同时看一根串口**或**多人协作调一台路由器**：

- 一台 Windows 机器只能开一个 COM 口持有者，第二个人就连不上；
- SSH 多终端登录到同一台路由器时不共享屏幕，看不到别人在做什么；
- 现成的 tmux/screen 共享方案在 Windows 上原生支持差，且只能共享 shell，不能共享串口。

termhub 把这件事做成"本机一开，全员可看"。

### 1.3 用户场景

- **场景 A（嵌入式调试现场）**：A 工程师把开发板的 COM3 接到自己的电脑、跑 termhub；B 同事用手机 ssh 进来围观日志，C 同事的笔记本 ssh 进来打两条命令复现问题。
- **场景 B（多设备共享 SSH）**：A 用 termhub 打开到客户路由器的 SSH 上联；B、C 通过 ssh 进 termhub 的下联，三人合流操作同一台路由器，省去了反复传"刚才那条命令是什么"。
- **场景 C（本地 shell 分享）**：A 在 termhub 里把本机 PowerShell 当作上联开放出去，B 接入时进入与 A 完全相同的 shell，便于结对调试。

---

## 2. 功能需求

### 2.1 v1 必做（In Scope）

- 五种上联类型：
  - Serial（含波特率/数据位/停止位/校验/流控可配置）
  - SSH 客户端（密码认证）
  - Telnet 客户端
  - Raw TCP socket
  - Local Shell（PowerShell / cmd / WSL / 自定义命令）
- **多会话**：同时挂多个上联，下联用户用 `ssh <session_name>@host -p 2222` 选频道
- **完全合流共享**：所有下联看一致输出，任何下联都能写入；不做主从/抢占
- **不回放历史**：新接入只看后续输出
- **自动重连**：按上联类型差异化策略（见 §4），可逐会话覆盖默认值
- **下联 SSH server**：内嵌 `russh`，密码认证，每会话独立用户名（=会话名）+ 密码
- **托盘常驻**：关主窗口缩到托盘，服务持续运行；托盘菜单可"打开主窗口/完全退出"
- **配置 GUI**：一屏全览，左栏会话列表 + 右栏详情/接入用户列表
- **配置持久化**：TOML 落盘，密码字段经 Windows DPAPI 加密
- **风险提示 banner**：默认监听 `0.0.0.0:2222`，GUI 顶部提示并提供"仅本机"开关
- **Kick / 流量计数**：GUI 中实时显示每个接入用户与字节流量，可单独踢人

### 2.2 v1 不做（Out of Scope，明确声明）

- 公钥认证 / SSH 键盘交互认证
- 多用户身份 / ACL / 审计日志
- 历史回放 / 输出录制 / 日志归档（仅运行期诊断日志）
- 主从 / 抢占式输入控制（仅合流）
- 共享模式切换 GUI（v1 仅一种语义）
- 公网穿透 / NAT 穿越（建议用户自行用 frp / cloudflared）
- mac / Linux 打包（架构预留，但 v1 仅出 Windows 安装包）
- GUI 内嵌终端渲染（GUI 不做终端，仅做控制面板）
- 多 tab / 主题定制 / 高级 UX 个性化

### 2.3 v2 候选（不在本 spec 范围）

公钥认证、共享模式切换（合流/主从/抢占）、历史回放与录制、内置反向隧道、跨平台二进制、移动端伴侣 App、上联自动发现（USB 枚举）。

---

## 3. 整体架构

### 3.1 进程模型

Tauri 单进程双侧：

- **后端（Rust）**：常驻 `tokio` 运行时，承载所有 IO、上联驱动、Hub、下联 SSH server、托盘
- **前端（WebView）**：Tauri 内嵌 WebView2，UI 用 React + TypeScript + Tailwind/shadcn-ui
- **IPC**：前后端通过 Tauri `invoke`（命令）+ `event`（事件推送）通信
- 关闭主窗口仅隐藏 WebView，Rust 进程继续；"完全退出"由托盘菜单触发

### 3.2 后端模块

```text
┌──────────────────── Tauri Backend (Rust) ──────────────────────────┐
│                                                                    │
│  ┌───────────┐    ┌────────────────┐     ┌──────────────────────┐  │
│  │ Config    │    │ Upstream       │     │ Hub (per-session)    │  │
│  │ Store     │◄──►│ Driver Trait   │◄───►│  - broadcast<Bytes>  │  │
│  │ (TOML)    │    │  ├ Serial      │     │    (上联→所有下联)   │  │
│  └───────────┘    │  ├ Ssh         │     │  - mpsc<Bytes>       │  │
│                   │  ├ Telnet      │     │    (所有下联→上联)   │  │
│                   │  ├ RawTcp      │     │  - 订阅者列表        │  │
│                   │  └ LocalShell  │     └──────────┬───────────┘  │
│                   └────────────────┘                │              │
│  ┌───────────┐                          ┌──────────────────────┐   │
│  │ SessionMgr│◄─────────────────────────┤ SSH Server (russh)   │   │
│  │  Map<Name,│                          │ 监听 :2222           │   │
│  │   Hub>    │                          │ 用户名=会话名/密码鉴权│   │
│  └─────▲─────┘                          └──────────────────────┘   │
│        │ IPC                                                        │
│  ┌─────┴──────────────────────────────────────────────────────┐    │
│  │ Tauri Command Handlers + Tray + Window Mgmt                │    │
│  └────────────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────────┘
```

### 3.3 关键设计选择

| 选择 | 决定 | 理由 |
|---|---|---|
| 每会话 = 一个 Hub | broadcast + mpsc 双 channel | 完全合流语义可由两条 channel 直接表达 |
| Upstream 是 trait | 五种类型同一接口 | 加新类型只动 driver crate |
| 下联用户名 = 会话名 | 不做用户管理 | 调试场景对"用户身份"无需求；密码=会话密码即可 |
| 不回放 = 不维护 backlog | broadcast 仅作传输 | 与 §2.1 已确认行为一致 |
| 上联断开默认重连 | LocalShell 例外 | 重连本地 shell 违反"用户主动 exit"直觉 |
| 配置加密 | DPAPI | Windows 原生，不引第三方依赖 |

---

## 4. 会话生命周期与自动重连

### 4.1 状态机

```text
                 stop / delete
                ◄──────────────────┐
                │                  │
   [Idle] ─create─► [Starting] ──► [Running] ─upstream_lost─► [Reconnecting]
      ▲                │                                         │  │
      │                │err                                      │  │success
      │                ▼                                         ▼  ▼
      │            [Failed]                              [Reconnecting] / [Running]
      │                ▲                                         │
      │                │                                         │exhausted / user_stop
      └─────────── delete ──────────────────────────────────────►┘
```

状态枚举：`Idle / Starting / Running / Reconnecting / Failed / Stopped`，通过 Tauri `event` `session:status_changed` 推前端。

> **注**：当某会话 `auto_reconnect = false`（如 Local Shell 默认情况）时，`Running` 收到 `upstream_lost` 不进 `Reconnecting`，而是直接进入 `Stopped`。状态机的"是否重连"分支由会话配置决定。

### 4.2 重连策略

| Driver | 默认重连 | 触发条件 | 退避序列 |
|---|---|---|---|
| Serial | ✅ | 设备拔出 / IO 错误 | 1s → 2s → 4s → 8s → 30s 上限，无限次 |
| SSH | ✅ | TCP/SSH 断 | 同上 |
| Telnet | ✅ | TCP 断 | 同上 |
| Raw TCP | ✅ | TCP 断 | 同上 |
| Local Shell | ❌ | 进程退出 → Stopped | （可在 GUI 单独打开） |

每会话 GUI 上有"自动重连"开关，可逐会话覆盖默认。

### 4.3 失败终止条件

- **网络/IO 错误**（连接被重置、设备临时不可用）→ Reconnecting
- **配置错误**（找不到 COM、SSH auth failed、命令不存在）→ 直接 Failed，不重试

### 4.4 抖动去抖

第一次断线先等 1 秒重试；若 1 秒内连续两次失败再进入退避序列，避免 USB 抖动刷屏。

### 4.5 下联体验

进入 Reconnecting 时 Hub 不关 broadcast，仅插入 ANSI 着色的系统行：

```
*** termhub: upstream lost, reconnecting...
*** termhub: reconnected
```

下联 SSH 会话**保持不断**，重连成功后输出无缝继续。

### 4.6 GUI 操作

每个会话卡有 `Start / Stop / Restart / Delete`：

- `Stop`：中止重连循环并关闭上联
- `Delete`：先 Stop 然后从配置删除（向所有下联发 `*** termhub: session terminated` 后关 SSH 通道）

---

## 5. 数据通路与 SSH Server

### 5.1 上联 → 下联（输出扇出）

上联 driver 读到字节立即 `broadcast::Sender::send`；不做行/包缓冲，追求低延迟。每个下联各自一个 `broadcast::Receiver`。

**滞后处理**：单个下联消费不过来（broadcast 满 1024 chunk lag）时，丢弃该下联滞后数据并发 `*** termhub: client lagged, output truncated`，**绝不阻塞别的下联**。

系统级提示行（断线/重连/会话开停）走同一条 broadcast，前缀 `\x1b[33m*** termhub:` 用 ANSI 黄色标记。

### 5.2 下联 → 上联（输入汇聚）

多下联输入字节按到达顺序进 `mpsc::Sender`，原样写到上联。**输入交叉是合流模式的物理事实**，不做去重/排队（违反低延迟透传哲学）。GUI 显示"最近输入来自 client X"作提示，便于追溯。

### 5.3 SSH Server（基于 russh）

| 项 | 默认 | 备注 |
|---|---|---|
| 监听 | `0.0.0.0:2222` | 端口可改；不绑 22 避免与 OpenSSH 冲突 |
| Host key | `Ed25519`，自动生成 | 存 `%APPDATA%\termhub\host_key`，GUI 可显示指纹 |
| Auth | `password` only | v1 范围 |
| 用户名 | `^[a-zA-Z0-9._-]+$`，不区分大小写 | 等同会话名 |
| 密码 | 每会话独立 | DPAPI 加密落盘 |
| 同会话最大并发 | 16，可调 | 防恶意打满 |
| Keepalive | 30s 无响应判离 | TCP keepalive + SSH ping |

### 5.4 PTY 与终端尺寸

**上联是 PTY 类**（SSH 上联 / Local Shell）：

- 上联 PTY size 由**第一个接入的下联请求决定**
- 后续下联仅在本端 SSH channel 记录其请求 size，**不修改上游**
- GUI 显示当前上联 size + 各下联 size（便于排查渲染错位）
- 提供"GUI 强制设定 PTY size"覆盖项（如固定 80×24），独立于任何下联

**上联是流类**（Serial / Telnet / Raw TCP）：

- 不存在 PTY 概念，下联 size 请求完全忽略
- 字节流原样透传

### 5.5 行结束与编码

- **默认完全不改字节流**——加密通道里多嘴最容易出诡异 bug
- 串口配置项单独提供：
  - "客户端 `\r` 改写为 `\r\n` / `\n` / `\r`" 开关
  - "上联 `\r` 改写为 `\r\n`" 开关
  - 编码标签（仅作前端日志预览用，不影响透传字节流）

### 5.6 安全模型边界

- ✅ SSH 通道加密、host key 校验、密码鉴权
- ❌ 无 ACL / 多角色 / 审计
- ❌ 默认监听 `0.0.0.0`，**局域网内任何人知道密码即可接入**——GUI 顶部黄色 banner 警示并提供一键切回环
- ❌ 无公网穿透；公网访问请用户自行 frp / cloudflared

---

## 6. 前端 UI 设计

### 6.1 整体布局（一屏全览）

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ termhub                                              [⚙ 设置]  [_]  [□]  [×] │
├──────────────────────────────────────────────────────────────────────────────┤
│ ⚠ 监听 0.0.0.0:2222  局域网内任何人知道密码即可接入  [改为仅本机]  [复制指纹]│
├──────────┬───────────────────────────────────────────────────────────────────┤
│ 会话     │ 选中会话: com3                                                    │
│          │ 上联：Serial  COM3  115200 8N1 No-flow                            │
│ ● com3   │ 状态：● Running     运行 12m 03s     重连次数: 0                  │
│ ● router │ 用户名/密码：com3 / pass1234   [👁]  [📋]                          │
│ ◎ tcp1   │ 自动重连：● 开启     PTY 覆盖：80×24（流类无 PTY，禁用）          │
│ ● shell  │ 监听 ssh com3@<本机>:2222                                         │
│          │ ▶ Start   ⏸ Stop   ↻ Restart   ✎ 编辑   🗑 删除                   │
│          │ 接入用户表: # / 远程地址 / 接入时间 / 入字节 / 出字节 / Kick      │
│  + 新建  │                                                                   │
└──────────┴───────────────────────────────────────────────────────────────────┘
```

### 6.2 区块清单

| 区块 | 作用 |
|---|---|
| 顶部 banner | 风险提示 + 监听切换 + host key 指纹 |
| 左栏会话列表 | 状态点（绿/黄/红/灰）+ 名字 + 接入数 |
| 右栏详情 | 上联参数 / 状态 / 用户名密码 / 操作按钮 / 接入用户表 |
| 新建按钮 | 弹出向导，按上联类型动态切换表单 |
| ⚙ 设置 | 监听端口/地址、host key 重生成、自启动开关、日志路径 |

### 6.3 新建会话向导

- Step 1：上联类型下拉（Serial / SSH / Telnet / Raw TCP / Local Shell）
- Step 2：动态参数表单（按类型替换）
  - Serial：COM 口（实时扫描🔄）/ 波特率 / 数据位 / 停止位 / 校验 / 流控 / 行结束选项
  - SSH：host / port / username / password
  - Telnet：host / port
  - Raw TCP：host / port
  - Local Shell：command（默认 `powershell.exe`，可改 `cmd.exe` / `wsl.exe` / 自定义）
- Step 3：会话名（默认按上联类型自动生成）/ 密码（🎲随机生成按钮）/ 自动重连开关 / PTY 覆盖（仅 PTY 类启用）

### 6.4 实时数据更新

- 状态/客户端变化 → Tauri event 推送
- 字节计数 → 1 Hz 节流，前端拉式 `invoke('get_metrics', sessionId)`
- 串口/网卡列表 → 按需扫描

### 6.5 键盘快捷键（v1 基础）

`Ctrl+N` 新建 / `Ctrl+,` 设置 / `Del` 删除选中 / `Ctrl+R` 重启选中

### 6.6 主题

跟随系统深色 / 浅色。

---

## 7. 工程实现

### 7.1 仓库结构（Cargo workspace + Tauri）

```text
termhub/
├─ Cargo.toml
├─ crates/
│  ├─ termhub-core/      # Hub / SessionMgr / Driver trait / 配置序列化
│  ├─ termhub-drivers/   # Serial / SSH / Telnet / RawTcp / LocalShell
│  ├─ termhub-sshd/      # 下联 SSH server
│  └─ termhub-app/       # Tauri 主二进制
│     ├─ src/            # commands / event / tray
│     └─ ui/             # React + TS 前端工程
└─ docs/superpowers/specs/...
```

### 7.2 关键依赖

| 用途 | crate |
|---|---|
| 异步运行时 | `tokio` |
| SSH server + client | `russh` + `russh-keys` |
| 串口 | `serialport` + `tokio-serial` |
| 本地 PTY | `portable-pty`（Windows ConPTY） |
| Telnet | `tokio` + 手写选项协商 |
| 配置 | `serde` + `toml` + `serde_with` |
| Windows DPAPI | `windows-sys` |
| 日志 | `tracing` + `tracing-appender` |
| Tauri | `tauri 2.x` + `tauri-plugin-autostart` + `tauri-plugin-tray` |
| 前端 | React 18 + TS + Tailwind + shadcn-ui + Vitest |

### 7.3 配置文件 schema

`%APPDATA%\termhub\config.toml`：

```toml
[server]
listen = "0.0.0.0:2222"
host_key_path = "host_key"
max_clients_per_session = 16

[ui]
start_minimized = false
auto_start = false

[[sessions]]
name = "com3"
password = "<dpapi-encrypted>"
auto_reconnect = true
pty_override = { cols = 80, rows = 24 }
upstream = { type = "serial", port = "COM3", baud = 115200, data_bits = 8, parity = "none", stop_bits = 1, flow = "none", input_eol = "as_is", output_eol = "as_is" }

[[sessions]]
name = "router"
password = "<dpapi-encrypted>"
auto_reconnect = true
upstream = { type = "ssh", host = "192.168.1.1", port = 22, user = "admin", password = "<dpapi-encrypted>" }
```

`upstream` 用 `#[serde(tag = "type")]` 的 internally tagged enum，5 种类型一一对应。

### 7.4 状态机实现要点

- 每会话由一个 `tokio::task` 拉起：循环 `connect → run → wait_lost → backoff → reconnect`
- 状态变化由 `tokio::sync::watch::Sender<SessionStatus>` 推 GUI 与重连循环
- `Stop` 通过 `tokio_util::sync::CancellationToken` 优雅关闭

### 7.5 错误与 panic 处理

- driver task 用 `JoinHandle` 监管：panic 进入 `Failed` 并写日志 backtrace
- 主进程 `panic = "unwind"`，自定义 `set_hook` 上报日志 + 托盘弹窗，进程不退
- 日志：`%APPDATA%\termhub\logs\termhub-YYYYMMDD.log`，按天滚动，保留 7 天

### 7.6 测试策略

| 层 | 工具 | 覆盖 |
|---|---|---|
| 单元 | `cargo test` | Hub 的 broadcast/mpsc、状态机转换、配置序列化、退避算法 |
| 集成 | `cargo test --test ...` | 起 SSH server + 自连 SSH client + 假 driver（loopback），验端到端字节通路 |
| 串口 | Linux 用 `socat` 虚拟 pty 对；Windows 用 `com0com` | CI 在 Linux 跑 socat |
| 前端 | Vitest + React Testing Library | 表单校验、状态渲染 |
| 烟测 | 5 个 client 同连发 1 GB，验不丢字节 | release 前必跑 |

---

## 8. 风险与边界

### 8.1 已知风险

- **合流模式输入交叉**：物理事实，不解决；GUI 显示"最近输入者"作提示
- **PTY size 共享冲突**：取首个下联请求 + 可强制覆盖；多下联 size 不一致时渲染可能错位
- **默认监听 `0.0.0.0`**：局域网知道密码即可入；GUI banner 警示
- **DPAPI 限制**：DPAPI 加密绑定 Windows 用户账户，跨用户/跨机器无法解密；这是预期行为（敏感字段不应跨账户漂移）

### 8.2 不变式（Invariants）

- 会话名在配置中**全局唯一**
- Hub 与会话**一一对应**，删除会话必先关 Hub
- 上联 driver task 退出 ≡ Hub 进入终止状态（Failed / Stopped）
- 下联 SSH 通道生命周期**短于**所属 Hub 的生命周期

---

## 9. v1 验收标准

1. 五种上联各能创建并稳定运行 ≥ 30 分钟
2. 单会话支持 ≥ 8 个下联同时合流，1 GB 数据透传不丢字节
3. 上联断开（拔串口 / 关 SSH 服务端）后自动重连成功，下联会话保持不断
4. 配置文件重启后恢复，密码字段在磁盘上为加密形式
5. GUI 风险 banner、监听切换、host key 指纹复制均可工作
6. 关主窗口后服务持续运行；托盘菜单可恢复窗口与完全退出
7. Kick 单个下联生效；下联超时（30s 无心跳）被自动剔除
8. 状态变化（Running / Reconnecting / Failed）实时反映到 GUI

---

## 10. 后置候选（v2 清单）

- 公钥认证 / SSH server `authorized_keys`
- 共享模式切换（合流 / 主从 / 抢占）
- 历史回放与会话录制
- 反向隧道 / 公网中继
- mac / Linux 二进制打包
- 移动端伴侣 App
- 上联自动发现（USB 热插拔扫描）
