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

当前 `Cargo.toml` 的版本号为 `0.1.2`。安装包通过本仓库的 Releases 发布，最新版本标记为 Latest；上游 GitHub Releases 的发行包**不代表包含本 README 描述的 SSH 增强功能**。

`v0.0.9` 已合并 ELYTerminal 0.2.0 的上游提交 [`6936b6f`](https://github.com/ELY-Open-Source/ELYTerminal/commit/6936b6f3edb5f55891dd62898290994f52e24dc0)，并修复 SSH 部分凭据缺失时的同步和本机删除记录被恢复的问题。集成范围见 [上游同步记录](docs/upstream-integration.md)，测试步骤见 [测试指南](docs/testing.md)。

`v0.1.0` 进一步合并至 ELYTerminal 0.2.2 的 [`ba664ef`](https://github.com/ELY-Open-Source/ELYTerminal/commit/ba664ef)，增加远程 Files/Git 每秒刷新、远程目录选择新建会话与可配置 SSH 保活。

`v0.1.1` 融合 ELYTerminal 0.2.3 的 [`fdca49a`](https://github.com/ELY-Open-Source/ELYTerminal/commit/fdca49a)，包含背景图布局、链接悬停下划线、右键复制／粘贴及 Windows 输入修复，并修复极端背景图尺寸或位置设置导致的计算溢出。SSH、SFTP、远程 Git 和托管 RSA PEM 私钥已通过实际连接测试。

`v0.1.2` 参照 Termora 重做 SSH 同步：稳定连接身份避免改组后重复，删除记录跨机传播，Gitee 固定更新同一个私密片段。同步策略支持手动或修改后随机延迟 10–29 秒，并显示最后成功时间；修复共享凭据版本、历史连接认证及同步期间本地修改的冲突。所有设备需升级后再共用新格式云端库，迁移后勿用旧版打开同一本地数据目录。详见 [同步说明](docs/ssh-sync.md)。

## 当前功能

### 本地终端与界面

- 通过 `alacritty_terminal` 提供终端网格、解析、滚动历史和 PTY 事件循环。
- 支持真彩色、宽字符、组合字符、鼠标报告、输入法、括号粘贴、OSC 8 链接和历史搜索。
- 支持 xterm、kitty 键盘协议，以及 Windows 的 win32-input-mode。
- 本地 shell 集成支持 zsh、bash、fish 和 PowerShell，报告当前目录、标题及命令状态。
- 本地终端默认从当前用户主目录启动，也可设置从指定文件夹或当前本地会话目录新建会话。
- 支持自定义启动配置及检测本机 shell，左侧加号旁的菜单可选择启动；支持命令行目录参数。
- 支持链接悬停下划线，以及可选的右键复制选区／无选区时粘贴；Windows 默认启用右键粘贴。
- **每次启动时左右侧栏均默认收起**，点击标题栏两端的图标或使用快捷键再展开，不恢复上次的展开状态。
- 左侧可切换会话与 SSH 主机；右侧通过图标切换监控、Files、Git 和传输任务，侧栏宽度支持拖动调整。
- 保留上游的 Ely 桌面角色、会话状态标记、通知、主题、透明度与背景模糊。
- 支持 Catppuccin Mocha 等内置主题，监控面板跟随应用主题，Mocha 的主要高亮使用粉色。
- 设置中的上游终端界面支持英文、简体中文切换；MioTerm 新增的部分 SSH、监控和同步界面仍为英文。
- 支持新建、编辑自定义主题、选择终端背景图，并记住窗口的显示器和位置。
- 背景透明度可设为 0–100%；背景图可覆盖终端或整个窗口，并设置大小、位置与平铺。
- 支持拖动会话排序和会话右键复制、关闭；复制 SSH 会话会重连原主机，并从已报告的远程目录启动。
- 可在设置中启用本地 Files 右键菜单，提供打开、复制路径和在此处新建本地会话。
- Files 展示时每秒刷新文件与 Git 标记；可将本地文件树条目拖入终端输入路径，右键 Files 图标可选择目录，按当前本地或 SSH 会话新建对应会话。
- 可设置界面字体与字号、终端字重、独立中日韩字体、抗锯齿模式和平滑光标。
- 支持 Kitty 图片协议与动画、OSC 5522 剪贴板协议；程序读取剪贴板前默认请求许可。
- 历史记录空闲后压缩存储，采用上游的 ASCII 解析和文本绘制优化。
- Windows 使用随包提供的 Microsoft OpenConsole / ConPTY，支持 WSL、Git Bash 目录集成及 Windows 11 Mica 背景。
- Windows 支持 cmd 提示符与目录集成，并可在设置中添加或移除“在 MioTerm 中打开”的资源管理器菜单。

### SSH 连接管理

- 使用系统 OpenSSH 的 `ssh`，不是在应用中另行实现 SSH 协议。
- 支持保存、编辑、分组、搜索主机，快速连接及最近连接记录。
- 新建连接的默认地址为 `127.0.0.1`，默认端口为 `22`；已有主机地址不会被覆盖。
- 支持 OpenSSH 配置、密码、指定私钥和 SSH Agent 四种认证模式。
- 每台主机可开关 SSH 保活、设置 1–3600 秒间隔，默认启用、30 秒，连续 3 次无响应后断开；终端、SFTP、Git 和监控统一使用，下次连接生效。此项作为本机网络偏好保存，不参与云同步。
- 主机编辑器可开启断线自动重连，默认关闭；已登录连接出现 SSH 传输错误后最多重试五次。手动或自动重连复用原会话位置和远程目录，正常退出不自动重连，不恢复远程程序或重放命令。
- 私钥模式可选择 PEM 或 OpenSSH 文件；保存或快速连接时导入应用数据目录的 `keys/`，之后移动或删除原文件不影响连接和同步。旧连接编辑后重新保存即可导入。
- 密码和私钥口令可选择保存在应用数据目录的加密凭据文件中，不以明文写入 `settings.json` 或 `connections.json`。
- 未知主机指纹仍需用户确认，不会自动信任；已保存的密码不会被用于回答 OTP/MFA 验证。

| 客户端系统 | 凭据存储 | 后台连接方式 |
|---|---|---|
| macOS | 应用加密文件 | 复用已认证会话的私有控制套接字，或使用明确保存的凭据 |
| Linux | 应用加密文件 | 复用已认证会话的私有控制套接字，或使用明确保存的凭据 |
| Windows | 应用加密文件及当前用户 ACL | 使用保存的凭据、Agent 或无需交互的私钥；不使用 Unix 控制套接字 |

Windows 中仅在终端交互输入、没有保存的密码，无法直接复用于后台 Files、Git 和监控连接。日常凭据操作不依赖系统钥匙串；旧版系统凭据需要在 Sync 页面显式导入。加密文件损坏或本机密钥丢失时会报错，不会静默重置。

**远程面板需要通过应用的 Hosts 或 Quick SSH connection 打开连接。** 在普通本地终端中手动输入 `ssh user@host` 仍可登录，但不会自动将该本地会话转换为应用管理的远程会话。

### 本地与远程文件

- 本地会话的 Files 显示当前 shell 目录及 Git 状态标记。
- SSH 会话的 Files 同时显示本地和远程两个区域，本地在上、远程在下。
- 两个区域可见时每秒刷新列表及 Git 标记，保留选择和地址栏草稿；隐藏、传输期间暂停轮询，慢请求不重叠。
- 两个区域可独立切换目录，提供路径输入、主目录、上级目录和刷新。
- 使用系统 `sftp` 上传、下载文件及递归文件夹，支持多选与取消。
- SSH 的 Files 面板在本地和远程目录之间展开传输任务，按文件排队，查看状态、耗时、路径及失败原因；支持取消、从头重试、清除历史。切换会话和收起侧栏后任务继续，退出应用后停止，任务记录不跨重启保留。
- 可从系统文件管理器拖入文件或文件夹到下方远程区域，确认后上传到当前显示的远程目录。
- 写入前需要确认，已有文件可能被覆盖，同名文件夹可能合并。
- 远程 shell 每次提示符报告当前目录，`cd` 后 Files 和 Git 随之更新。
- 从 Files 导航远程目录也会更新 Git 检查位置，但不会向终端输入 `cd`；下次 shell 提示符仍以终端目录为准。

当前不提供断点续传、原子传输或字节级进度；失败或取消可能留下部分文件。不支持选择符号链接、特殊文件传输，也不接受含控制字符的批量传输路径。

新增功能的操作与测试步骤见 [传输任务与断线重连](docs/transfers-reconnect.md)。

### 本地与远程 Git

- Git 面板可见时每秒检查当前本地或远程仓库，更新状态、分支与历史；刷新保留提交草稿，等待面板 Git 操作完成。

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

- **Gitee**：配置具有 `gists` 权限的 Token 和保险库名称；不需要搭建服务器或手动填写片段 ID。另一台机器使用同一账号、保险库名称和独立加密口令恢复。Token 可选择保存在应用的加密凭据文件，不写入设置 JSON。
- **WebDAV**：继续使用已存在的 HTTPS WebDAV 文件夹及用户名，原有设置无需迁移。

- **Save**：保存同步配置及选择记住的凭据；口令保存在应用的加密凭据文件，不以明文写入 JSON。保存设置本身不触发同步。
- **Sync now**：下载并解密已有归档，合并本地保存的主机，加密后上传，再导入合并结果。
- **Restore**：下载并合并到本地，不修改服务器上的归档。
- 同步主机配置、认证模式和**显式选择的私钥文件本体**，不是只同步另一台机器无法使用的绝对路径。
- 支持一并保存相邻的 OpenSSH `-cert.pub` 证书。
- 私钥内容编码到归档 JSON 后整体加密上传，不单独上传私钥文件。在另一台机器恢复到应用数据目录的 `keys/`，并自动更新主机的本地路径。
- Unix 下管理目录权限为 `0700`、私钥文件为 `0600`；Windows 下使用仅当前用户可访问的受保护 ACL。

完整归档使用 age 口令加密后才上传。WebDAV 密码和独立加密口令可选择记住，不以明文保存到设置；加密口令至少 6 个字符，丢失后无法恢复归档。Gitee Token 只用于访问服务，更换 Token 后需重新填写同一个加密口令，才能保存新 Token 对应的本地口令记录。

**Sync SSH passwords and key passphrases** 默认关闭。开启后，显式保存的 SSH 密码和私钥口令随连接版本一起加密同步。更新后的密码可替换旧版本，但同版本冲突或同步期间发生的本地密码变更会停止同步；不把秘密写入连接 JSON。未开启时不导出或恢复凭据，也不删除云端以前保存的凭据。

**Remember encryption passphrase** 可以记住至少六个字符的加密口令，之后留空即可复用。**Sync policy** 与 Termora 一样提供 **Manual**（默认）和 **On change**：选择后者并保存，新增、修改、删除连接或导入已保存凭据后，随机等待 10–29 秒再同步；继续修改会重新计时。手动与自动任务不会并发执行，均先拉取合并再上传；导入结果不重复触发同步，同步中的新编辑保留到下一轮。启动、保存设置和仅发生在远端的变化不触发同步，不设每日轮询或失败定时重试；失败后修改数据或点 **Sync now** 可再次尝试。设置页显示最后成功同步时间。另一台机器仍需首次配置 Token 和同一个加密口令，并主动同步或恢复以接收远端变化；外部密钥文件修改不轮询检测。

同步采用与 Termora 类似的固定连接 ID、记录版本和删除标记。修改名称、分组、地址或密钥更新原连接，不再产生旧的未分组副本；删除后，另一台设备下次同步或恢复时也会移除它。离线旧记录不会复活，手动重新添加会生成新 ID。旧版明确可识别的未分组重复项会在迁移时清理，不同密钥或不同配置保留。

不同步终端内容、应用设置、最近连接、`known_hosts` 或同步服务的 Token。没有选择同步私钥口令时，恢复后的加密私钥仍需要原口令。请先升级所有设备；旧版本无法读取新记录格式。

本地统一使用应用数据目录中的 `credentials/credentials.age`，没有新增 SQLite 依赖，日常保存、连接和同步不依赖系统钥匙串。旁边的 `local.key` 是自动生成的本机解密密钥；Unix 限制目录为 `0700`、文件为 `0600`，Windows 限制为当前用户访问。**拿到整个目录和密钥的人仍能解密，不能把这种本机加密当作主密码保险库。** 云端归档另用你设置的同步口令加密，不上传 `local.key`。

默认数据目录延续旧版的 `ELYTerminal` 名称：macOS 为 `~/Library/Application Support/ELYTerminal`，Windows 为 `%APPDATA%\ELYTerminal`，Linux 为 `$XDG_CONFIG_HOME/ELYTerminal`（未设置时是 `~/.config/ELYTerminal`）。可通过绝对路径环境变量 `MIOTERM_CONFIG_DIR` 指定其他应用数据目录；不要放到源码仓库或公开共享目录中。升级后在 Sync 页面点击 **Import system credentials**，可将旧版已保存的 SSH 凭据及选择记住的 Gitee Token 导入；原系统条目保留，冲突时不覆盖。

连接上的“记住密码”标记不等于密码本体已经迁入。缺失时会列出具体连接并继续同步其他可用数据；可在主机编辑器重新保存，或在云端已有密码时使用 Restore 恢复。凭据文件损坏等真实错误仍会停止同步。

macOS → Windows：先在源端开启凭据同步并执行同步，再在 Windows 配置同一账号、保险库和同步口令，开启凭据同步后点击 Restore。私钥按内容恢复到目标应用目录的 `keys/` 并更新连接路径，密码在目标应用目录重新加密保存，不复制 macOS 钥匙串。

Gitee 首次创建一个私密 Gist，之后固定 PATCH 更新，自动保存片段 ID；旧快照首次同步时汇总迁移，之后不再反复合并。单份加密归档上限 512 KiB，大型保险库建议使用 WebDAV。当前归档会移除已删除连接的密码及无引用密钥；仅保留小型 ID 删除标记，防止离线设备重新带回旧连接。标记随不同连接的删除次数增长，最多 10,000 条，不会随每次编辑或同步增加。

Gitee 上传前会复查、上传后会读回校验，但其 API 没有保证原子条件更新，两台设备恰好同时写入仍有覆盖窗口；要求严格并发保护时使用支持强 ETag 的 WebDAV。旧 Gist 和服务端版本历史不会由应用自动删除。

仅通过 OpenSSH 配置隐式使用的密钥、默认密钥、Agent 和硬件密钥不会自动导出；需要在主机的私钥模式中明确选择文件。WebDAV 归档的更新仍要求服务器提供强 ETag 并支持条件写入。

安全边界、合并规则和恢复细节见 [SSH 加密同步](docs/ssh-sync.md)。

## 运行要求

### 客户端

- 原项目具有 macOS、Windows 和 Linux 的平台实现。
- SSH 功能需要系统 `ssh`、`sftp` 位于可执行路径中；Git 功能需要 `git`。
- Linux 图形界面需要 X11 或 Wayland 及可用的图形环境；仅从旧版系统存储导入凭据时需要 Secret Service。
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
| Ely | `ely-gpui-component`，固定 Git revision `494c17d` |
| 终端引擎 | `alacritty_terminal` `0.26.0`，使用上游固定 revision 的引擎与 `vte` 补丁 |
| SSH / SFTP / Git | 系统命令行客户端 |
| 同步与凭据 | `age`、`ureq` / rustls；`keyring` 仅用于显式导入旧版系统凭据 |

在 macOS 使用运行时 Metal shader 编译启动：

```sh
cargo run --locked --release --features gpui_platform/runtime_shaders
```

Windows、Linux：

```sh
cargo run --locked --release
```

本次上游集成使用仓库固定的 Rust `1.95.0` 完成 macOS 构建、Clippy 和回归测试。

Linux 编译依赖，Debian / Ubuntu 示例：

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev libdbus-1-dev
```

打包脚本基于原项目调整，应用显示名与发行包名使用 MioTerm，Rust crate 和已有配置目录仍保留原名称：

- macOS：`scripts/mac-app.sh`。
- Windows：`scripts/windows-zip.ps1`。
- Linux：`scripts/linux-packages.sh`，需要 Docker，在 Ubuntu 22.04 构建 tar.gz、DEB、RPM、AppImage；也可单独运行 `scripts/linux-tar.sh`。

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

设置修改即时保存；无效的设置文件会阻止启动并给出错误，不会用默认值覆盖原文件。SSH 密码、私钥口令不存放在上述 JSON 文件中。`keys/` 中是供系统 OpenSSH 使用的私钥副本，保留原密钥自身的加密格式，依靠文件权限保护；并非额外使用本地凭据库加密。

更新检查默认读取本仓库 MioTerm 的 Releases，保留启动检查、每日检查、手动检查与打开下载页的原有流程，不会回退到上游发行包。可在 Settings > Terminal 中关闭自动检查；仓库没有已发布的 Release 或不可访问时会显示检查失败。

日志写入 stderr，设置 `RUST_LOG=elyterminal=debug` 可增加诊断信息。Windows release 构建不打开控制台，可使用 `elyterminal.exe 2> log.txt` 保留日志。分享日志前请检查并移除主机地址、用户名和路径等信息。

## 开发与验证

`scripts/check.sh` 包含格式、Clippy、测试和手写文件不超过 500 行的检查；目录结构与工程约定见 [AGENTS.md](AGENTS.md)。

新增测试覆盖 SSH 参数与路径、凭据处理、远程目录报告、文件传输、Git 后端、监控数据及 WebDAV 加密恢复。`tests/ssh_auth.rs` 默认使用隔离应用目录验证加密凭据和 askpass，不读取真实用户凭据。真实 SSH 集成测试位于 `tests/remote_ssh.rs`，需要隔离的本地 SSH 服务并显式启用；`tests/ssh_auth.rs` 的真实密钥登录测试需设置对应 fixture 环境变量。运行前应阅读对应测试和功能文档，不要将测试指向生产主机。

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

工作流使用 GitHub 托管的 macOS、Windows 和 Ubuntu runner，不需要原作者的自托管机器；macOS 会安装 shell 测试需要的 fish，Linux 保留原有的容器测试环境。同一次运行中包含三端任务，全部检查和打包成功后，使用已推送的 tag 创建普通 Release 并标记为 Latest，附上 macOS arm64、Windows x86_64、Linux x86_64 安装包与 SHA-256 校验文件；不使用原作者的签名身份或私钥，也不会覆盖上游的旧版本标签。

发布新版本时，先更新 `Cargo.toml` 和 `Cargo.lock` 中的项目版本，提交并推送到 `origin/main`，再推送对应的新 tag。例如版本为 `0.1.2` 时：

```sh
git tag -a v0.1.2 -m "MioTerm 0.1.2"
git push origin v0.1.2
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
- Windows 随包携带 Microsoft ConPTY，MIT 许可保留在 `vendor/conpty`；Linux 包保留 FreeType 与 libX11 解码表的许可。

终端引擎、基础界面和原有资源的贡献属于各自作者；本修改版仅在其基础上继续开发。
