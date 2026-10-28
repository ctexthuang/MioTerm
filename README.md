# MioTerm

基于 Rust、GPUI 和 Ely 组件的原生终端，当前修改版在本地终端的基础上增加了 SSH 连接管理、远程文件传输、远程 Git、系统监控和 Gitee/WebDAV 加密同步。

当前主仓库：[ctexthuang / MioTerm](https://github.com/ctexthuang/MioTerm)。

## 项目来源与声明

**本项目基于原作者的 [ELYTerminal](https://github.com/ELY-Open-Source/ELYTerminal) 二次开发，并非从零独立开发，也不是上游官方发行版。**

- 原项目及维护组织：[ELY-Open-Source / ELYTerminal](https://github.com/ELY-Open-Source/ELYTerminal)。
- 原作者在本仓库继承的提交记录中署名为「雷电芽衣」；终端基座由原作者及 ELYTerminal contributors 开发。
- UI 组件基座：[Ely GPUI Components](https://github.com/ELY-Open-Source/Ely-GPUI-Components)。
- 本修改版主要扩展 SSH、SFTP、远程 Git、系统监控、WebDAV 同步及相关交互，不将这些新增功能归为上游已发布功能。
- 原项目的版权声明、MIT 许可证和第三方许可文件予以保留。感谢原作者和所有上游贡献者。

当前 `Cargo.toml` 的版本号为 `0.0.3`，修改版仍在开发中。测试包通过本仓库的 Releases 以 Preview 预发布形式提供；上游 GitHub Releases 的发行包**不代表包含本 README 描述的 SSH 增强功能**。

## 当前功能

### 本地终端与界面

- 通过 `alacritty_terminal` 提供终端网格、解析、滚动历史和 PTY 事件循环。
- 支持真彩色、宽字符、组合字符、鼠标报告、输入法、括号粘贴、OSC 8 链接和历史搜索。
- 支持 xterm、kitty 键盘协议，以及 Windows 的 win32-input-mode。
- 本地 shell 集成支持 zsh、bash、fish 和 PowerShell，报告当前目录、标题及命令状态。
- 启动时本地终端位于当前用户主目录；后续新建本地会话可按设置从主目录或当前本地会话目录启动。
- **每次启动时左右侧栏均默认收起**，点击标题栏两端的图标或使用快捷键再展开，不恢复上次的展开状态。
- 左侧可切换会话与 SSH 主机；右侧通过图标切换监控、Files 和 Git，侧栏宽度支持拖动调整。
- 保留上游的 Ely 桌面角色、会话状态标记、通知、主题、透明度与背景模糊。
- 支持 Catppuccin Mocha 等内置主题，监控面板跟随应用主题，Mocha 的主要高亮使用粉色。

### SSH 连接管理

- 使用系统 OpenSSH 的 `ssh`，不是在应用中另行实现 SSH 协议。
- 支持保存、编辑、分组、搜索主机，快速连接及最近连接记录。
- 新建连接的默认地址为 `127.0.0.1`，默认端口为 `22`；已有主机地址不会被覆盖。
- 支持 OpenSSH 配置、密码、指定私钥和 SSH Agent 四种认证模式。
- 密码和私钥口令可选择保存在当前系统的凭据存储中，不写入 `settings.json` 或 `connections.json`。
- 未知主机指纹仍需用户确认，不会自动信任；已保存的密码不会被用于回答 OTP/MFA 验证。

| 客户端系统 | 凭据存储 | 后台连接方式 |
|---|---|---|
| macOS | Keychain | 复用已认证会话的私有控制套接字，或使用明确保存的凭据 |
| Linux | Secret Service | 复用已认证会话的私有控制套接字，或使用明确保存的凭据 |
| Windows | Credential Manager | 使用保存的凭据、Agent 或无需交互的私钥；不使用 Unix 控制套接字 |

Windows 中仅在终端交互输入、没有保存的密码，无法直接复用于后台 Files、Git 和监控连接。系统凭据服务缺失或锁定时会报错，不会回退到明文文件保存。

**远程面板需要通过应用的 Hosts 或 Quick SSH connection 打开连接。** 在普通本地终端中手动输入 `ssh user@host` 仍可登录，但不会自动将该本地会话转换为应用管理的远程会话。

### 本地与远程文件

- 本地会话的 Files 显示当前 shell 目录及 Git 状态标记。
- SSH 会话的 Files 同时显示本地和远程两个区域，本地在上、远程在下。
- 两个区域可独立切换目录，提供路径输入、主目录、上级目录和刷新。
- 使用系统 `sftp` 上传、下载文件及递归文件夹，支持多选与取消。
- 写入前需要确认，已有文件可能被覆盖，同名文件夹可能合并。
- 远程 shell 每次提示符报告当前目录，`cd` 后 Files 和 Git 随之更新。
- 从 Files 导航远程目录也会更新 Git 检查位置，但不会向终端输入 `cd`；下次 shell 提示符仍以终端目录为准。

当前不提供断点续传、原子传输或字节级进度；失败或取消可能留下部分文件。不支持选择符号链接、特殊文件传输，也不接受含控制字符的批量传输路径。

### 本地与远程 Git

- 通过目标机器上的 `git` 命令执行，不使用 libgit2。
- 支持分支、文件状态、差异查看、提交历史、暂存、取消暂存、丢弃、提交和 amend。
- 丢弃改动需要确认；SSH 断开后不允许发起新的远程 Git 操作。
- 切换会话时跟随对应主机和目录，已排队的操作保留原始目标，不会转而操作另一个主机。

文件和 Git 的详细要求见 [远程 Files 与 Git](docs/ssh-remote.md)。

### 系统监控

- 随当前会话切换本地机器或远程 Linux 主机。
- 显示运行时间、负载、CPU、内存、交换空间和磁盘容量概览。
- 显示按 CPU 或内存排序的进程，以及上传、下载速率和网络历史曲线。
- 支持选择网络接口、暂停、恢复和刷新。
- 隐藏监控面板时停止采样；不再展示单独的 Filesystems 挂载列表。

远程监控目前针对 Linux，通过 SSH 执行 Python 3 采集脚本，不要求安装常驻代理；不宣称支持远程 macOS 或 Windows 的监控采集。

### 加密同步

在 **Settings > Sync** 中选择 Gitee 或 WebDAV：

- **Gitee**：配置具有 `gists` 权限的 Token 和保险库名称；不需要搭建服务器或手动填写片段 ID。另一台机器使用同一账号、保险库名称和独立加密口令恢复。Token 可选择保存在当前系统凭据库，不写入设置文件。
- **WebDAV**：继续使用已存在的 HTTPS WebDAV 文件夹及用户名，原有设置无需迁移。

- **Save**：仅保存同步配置，不保存或校验加密口令，也不上传归档。
- **Sync now**：下载并解密已有归档，合并本地保存的主机，加密后上传，再导入合并结果。
- **Restore**：下载并合并到本地，不修改服务器上的归档。
- 同步主机配置、认证模式和**显式选择的私钥文件本体**，不是只同步另一台机器无法使用的绝对路径。
- 支持一并保存相邻的 OpenSSH `-cert.pub` 证书。
- 在另一台机器恢复时，将私钥写入 `~/.ssh/elyterminal-sync/`，并将主机配置指向实际恢复的文件。
- Unix 下管理目录权限为 `0700`、私钥文件为 `0600`；Windows 下使用仅当前用户可访问的受保护 ACL。

完整归档使用 age 口令加密后才上传。WebDAV 密码和独立加密口令每次操作输入，不保存到设置；加密口令至少 6 个字符，丢失后无法恢复归档。Gitee Token 只用于访问服务，更换 Token 不影响归档解密。

**Sync SSH passwords and key passphrases** 默认关闭。开启后，显式保存在系统凭据库中的 SSH 密码和私钥口令也会进入加密归档，恢复到另一台机器的系统凭据库；不把它们写入连接文件。本地和远端凭据不一致时停止并报错，不静默覆盖。未开启时不导出或恢复凭据，也不删除云端以前保存的凭据。

当前同步为手动触发的增量合并，不传播删除。不同步终端内容、应用设置、最近连接、`known_hosts` 或同步服务的 Token。没有选择同步私钥口令时，恢复后的加密私钥仍需要原口令。

Gitee 每次同步创建一个新的私密加密快照，不修改、删除旧片段，避免并发覆盖；会自动合并尚未汇总的快照分支。每份加密快照上限 512 KiB，大型保险库建议使用 WebDAV。历史快照不会自动清理，凭据轮换后也可能保留旧密文；清理前应确认最新快照已包含全部数据。

仅通过 OpenSSH 配置隐式使用的密钥、默认密钥、Agent 和硬件密钥不会自动导出；需要在主机的私钥模式中明确选择文件。WebDAV 归档的更新仍要求服务器提供强 ETag 并支持条件写入。

安全边界、合并规则和恢复细节见 [SSH 加密同步](docs/ssh-sync.md)。

## 运行要求

### 客户端

- 原项目具有 macOS、Windows 和 Linux 的平台实现。
- SSH 功能需要系统 `ssh`、`sftp` 位于可执行路径中；Git 功能需要 `git`。
- Linux 保存凭据需要可用的 Secret Service；图形界面需要 X11 或 Wayland 及可用的图形环境。
- macOS 通知需要从 `.app` 启动，直接 `cargo run` 不提供相同的系统通知能力。

本修改版新增功能目前主要在 macOS 验证，已实际连接 Debian 检查远程目录与 Git 跟随；Windows 和 Linux 客户端的新增流程尚未完成完整端到端验证。

### SSH 服务器

| 功能 | 服务器要求 |
|---|---|
| 应用管理的 SSH 会话及目录跟随 | SSH 服务、Python 3，以及 zsh、bash 或 fish 登录 shell |
| 远程 Files | Python 3 |
| 文件上传、下载 | 已启用 SFTP 子系统 |
| 远程 Git | 已安装 `git` |
| 远程监控 | Linux、Python 3 及相应系统信息的读取权限 |

目录跟随通过会话私有临时目录中的 shell 集成实现，加载但不修改用户原有启动文件，正常退出时清理临时目录。不支持的登录 shell 会明确报错。

## 构建与运行

技术栈由当前仓库配置决定：

| 组件 | 当前配置 |
|---|---|
| Rust | edition 2024；`rust-toolchain.toml` 固定 `1.95.0` |
| GPUI | `gpui-pre` / `gpui-pre-platform`，固定 `0.3.8` |
| Ely | `ely-gpui-component`，固定 Git revision `7f7a715` |
| 终端引擎 | `alacritty_terminal`，固定 `0.26.0` |
| SSH / SFTP / Git | 系统命令行客户端 |
| 同步与凭据 | `age`、`ureq` / rustls、`keyring` |

在 macOS 使用运行时 Metal shader 编译启动：

```sh
cargo run --locked --release --features gpui_platform/runtime_shaders
```

Windows、Linux：

```sh
cargo run --locked --release
```

本次 macOS 开发验证使用已安装的 Rust `1.99.0`，未更改仓库的工具链固定值。使用该工具链时，可将上述命令的 `cargo` 替换为 `rustup run 1.99.0 cargo`。

Linux 编译依赖，Debian / Ubuntu 示例：

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev libdbus-1-dev
```

打包脚本基于原项目调整，应用显示名与发行包名使用 MioTerm，Rust crate 和已有配置目录仍保留原名称：

- macOS：`scripts/mac-app.sh`。
- Windows：`scripts/windows-zip.ps1`。
- Linux：`scripts/linux-tar.sh`，需要 Docker。

macOS 打包默认使用 ad-hoc 签名，不包含 Developer ID 签名或 Apple 公证；Windows 测试包也没有发布者签名，系统可能提示安全警告。修改版发行入口为 [MioTerm Releases](https://github.com/ctexthuang/MioTerm/releases)，仅在发布构建后提供下载。安装包附带项目及现有第三方许可文件。

## 快捷键

Settings > Terminal 中可查看当前平台的绑定。

| 操作 | macOS | Windows / Linux |
|---|---|---|
| 新建本地会话 | Cmd-T | Ctrl-Shift-T |
| 关闭会话 | Cmd-W | Ctrl-Shift-W |
| 会话 1 到 8 / 最后一个 | Cmd-1 至 Cmd-8 / Cmd-9 | Alt-1 至 Alt-8 / Alt-9 |
| 下一个 / 上一个会话 | Ctrl-Tab / Ctrl-Shift-Tab | Ctrl-Tab / Ctrl-Shift-Tab |
| 展开 / 收起左侧栏 | Cmd-B | Ctrl-Shift-B |
| 展开 / 收起右侧栏 | Cmd-Alt-B | Ctrl-Shift-Alt-B |
| 复制 / 粘贴 | Cmd-C / Cmd-V | Ctrl-Shift-C / Ctrl-Shift-V；Windows 也支持 Ctrl-V |
| 全选 | Cmd-A | Ctrl-Shift-A |
| 查找 | Cmd-F | Ctrl-Shift-F |
| 上一个 / 下一个提示符 | Cmd-Up / Cmd-Down | Ctrl-Shift-Up / Ctrl-Shift-Down |
| 放大 / 缩小 / 重置字号 | Cmd-= / Cmd-minus / Cmd-0 | Ctrl-= / Ctrl-minus / Ctrl-0 |
| 打开链接 | Cmd-点击 | Ctrl-点击 |
| 设置 | Cmd-, | Ctrl-, |
| 退出 | Cmd-Q | Ctrl-Shift-Q |

## 配置与日志

设置和保存的 SSH 连接分别位于以下目录中的 `settings.json` 与 `connections.json`：

- macOS：`~/Library/Application Support/ELYTerminal/`。
- Linux：`$XDG_CONFIG_HOME/ELYTerminal/`，未设置时为 `~/.config/ELYTerminal/`。
- Windows：`%APPDATA%\ELYTerminal\`。

设置修改即时保存；无效的设置文件会阻止启动并给出错误，不会用默认值覆盖原文件。SSH 密码、私钥口令不存放在上述 JSON 文件中；普通私钥文件仍由文件系统和 SSH 管理。

更新检查默认读取本仓库 MioTerm 的 Releases，保留启动检查、每日检查、手动检查与打开下载页的原有流程，不会回退到上游发行包。可在 Settings > Terminal 中关闭自动检查；仓库没有已发布的 Release 或不可访问时会显示检查失败。

日志写入 stderr，设置 `RUST_LOG=elyterminal=debug` 可增加诊断信息。Windows release 构建不打开控制台，可使用 `elyterminal.exe 2> log.txt` 保留日志。分享日志前请检查并移除主机地址、用户名和路径等信息。

## 开发与验证

`scripts/check.sh` 包含格式、Clippy、测试和手写文件不超过 500 行的检查；目录结构与工程约定见 [AGENTS.md](AGENTS.md)。

新增测试覆盖 SSH 参数与路径、凭据处理、远程目录报告、文件传输、Git 后端、监控数据及 WebDAV 加密恢复。真实 SSH 集成测试位于 `tests/remote_ssh.rs`，需要隔离的本地 SSH 服务并显式启用；`tests/ssh_auth.rs` 的系统凭据测试也需显式启用。运行前应阅读对应测试和功能文档，不要将测试指向生产主机。

### 仓库与上游同步

本地工作副本使用两个远程：

| 远程 | 地址 | 用途 |
|---|---|---|
| `origin` | `https://github.com/ctexthuang/MioTerm.git` | 默认拉取、推送及修改版发布 |
| `upstream` | `https://github.com/ELY-Open-Source/ELYTerminal.git` | 获取原作者的更新 |

本次配置将 `main` 的默认远程与推送目标指向 `origin`。新克隆的工作副本可执行 `git remote add upstream https://github.com/ELY-Open-Source/ELYTerminal.git` 添加上游。

同步原作者代码前，先提交或妥善保存本地改动，并确认上游仍使用 `main` 分支：

```sh
git fetch upstream
git log --oneline HEAD..upstream/main
git merge upstream/main
```

合并冲突需要逐项处理，保留 MioTerm 的 SSH 增强功能、仓库地址和更新来源。完成测试后，再执行 `git push origin main`。上游代码同步与应用发行更新是两条独立流程，不会在启动时自动合并源码或覆盖本地改动。

检查、测试、打包和发布统一在一个 `Release` 工作流中。普通分支提交、PR 和手动操作都不会触发；只有推送 `v` 开头的版本 tag 才会运行。tag 必须等于 `Cargo.toml` 版本号加 `v` 前缀，否则在开始三端构建前失败。

工作流使用 GitHub 托管的 macOS、Windows 和 Ubuntu runner，不需要原作者的自托管机器；macOS 会安装 shell 测试需要的 fish，Linux 保留原有的容器测试环境。同一次运行中包含三端任务，全部检查和打包成功后，使用已推送的 tag 创建 Preview 预发布，附上 macOS arm64、Windows x86_64、Linux x86_64 安装包与 SHA-256 校验文件；不使用原作者的签名身份或私钥，也不会覆盖上游的旧版本标签。

发布新版本时，先更新 `Cargo.toml` 和 `Cargo.lock` 中的项目版本，提交并推送到 `origin/main`，再推送对应的新 tag。例如下一个版本为 `0.0.3` 时：

```sh
git tag -a v0.0.3 -m "MioTerm 0.0.3"
git push origin v0.0.3
```

不要复用已有版本 tag；修复后重新发布应提升版本号。失败的构建可以在 Actions 中重新运行同一次工作流。旧的 CI 和 Release 历史记录仍会保留，但新的普通提交不再产生运行记录。

发布附件脚本 `scripts/release-asset.sh` 默认上传到 `ctexthuang/MioTerm`。日后配置自己的 Developer ID 和公证凭据后，`scripts/mac-app.sh` 仍支持正式签名与公证；默认测试构建不需要这些凭据。

## 许可证与致谢

本项目保留上游的 [MIT 许可证](LICENSE) 与 `Copyright (c) 2026 ELYTerminal contributors` 声明。

- 终端应用基座：[ELYTerminal](https://github.com/ELY-Open-Source/ELYTerminal)，原作者及其贡献者。
- UI 组件：[Ely GPUI Components](https://github.com/ELY-Open-Source/Ely-GPUI-Components)。
- 方框绘制与按键编码参考 Ghostty 的 MIT 代码，许可见 [LICENSE-GHOSTTY](LICENSE-GHOSTTY)。
- 内置主题来自 iTerm2-Color-Schemes，许可保留在 `assets/themes`。
- JetBrains Mono 及其 Nerd Font 版本采用 SIL Open Font License，许可保留在 `assets/fonts`。

终端引擎、基础界面和原有资源的贡献属于各自作者；本修改版仅在其基础上继续开发。
