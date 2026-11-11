# 本次 ELY 0.2.4 融合测试方案

适用于 MioTerm v0.1.3。上游基线为 `9504b53`，2026-11-11 的融合提交为
`b5819c7`，本次发布将 MioTerm 自身版本从 `0.1.2` 更新为 `0.1.3`。

## 测试顺序

建议先完成前五项，约 15 至 25 分钟；其余按需要验证。

| 顺序 | 范围 | 重点 |
| --- | --- | --- |
| 1 | 启动与基础界面 | 用户目录、侧栏默认关闭、Mocha 配色、小人物 |
| 2 | 快捷键与清屏 | 自定义、冲突提示、重置、本地和 SSH 清屏 |
| 3 | 文件夹新建会话 | 本地/远程自动区分，Files/Git 跟随当前目录 |
| 4 | 拖拽与传输 | 路径转义、焦点、双向拖拽、自动展开任务 |
| 5 | SSH 重连 | 原位重连、保留目录、正常退出不自动重试 |
| 6 | 背景设置 | 图片独立透明度、渐变、遮罩、主题可读性 |
| 7 | 本地 CLI | 指定会话读写、远程目录、断线后拒绝输入 |
| 8 | 同步回归 | 分组、删除、缺少凭据、私钥跨设备恢复 |

SSH 测试应从应用的主机列表或快速连接建立连接。在普通本地终端里手动执行
`ssh user@host`，不会启用应用的远程 Files、Git、监控和连接管理。
测试远程 Files/传输需要服务端 Python 3 和 SFTP，远程 Git 还需要 `git`。

## 运行目录版

已经构建的最新二进制：

```sh
/Users/ctexthuang/Desktop/code/ELYTerminal/target/release/elyterminal
```

同一配置目录只运行一个应用实例，重复启动前先退出旧窗口。
需要重新编译时，在项目根目录执行：

```sh
cargo run --offline --locked --release --features gpui_platform/runtime_shaders
```

`runtime_shaders` 是本机 macOS 构建所需的功能开关。
若要测试空配置，可在退出普通实例后，使用独立配置目录启动：

```sh
MIO_TEST_CONFIG=$(mktemp -d /tmp/mioterm-024-config.XXXXXX)
env MIOTERM_CONFIG_DIR="$MIO_TEST_CONFIG" target/release/elyterminal
```

空配置不会带入已有主机或同步设置；它和当前正常配置的测试结果要分开记录。

## 1. 启动与界面

1. 启动后执行 `pwd`。未设置固定起始目录时，应为当前用户主目录，而非项目目录。
2. 左右侧栏均应关闭；分别点击左右图标后才展开，再点击可收起。
3. 当前主题为 Catppuccin Mocha。检查主机、Files、Git、监控的高亮和选中状态
   遵循主题，粉色高亮与其他面板一致。
4. 检查设置中启用的小人物仍显示；左栏切换到会话页时，原有 3×3 状态点保留。
5. 在本地和通过应用连接的远程 zsh/bash 中分别执行：

```sh
printf 'terminal=%s version=%s\n' "$TERM_PROGRAM" "$TERM_PROGRAM_VERSION"
```

两端预期均为 `terminal=mioterm version=0.1.3`。

## 2. 快捷键与清屏

1. 打开设置 → 终端 → 快捷键（Settings → Terminal → Shortcuts）。
2. 搜索“清屏 / Clear”，录制一个未占用的组合，例如 `Cmd+Shift+K`。
3. 尝试把它设成查找使用的 `Cmd+F`：应提示冲突，不能覆盖已有操作。
4. 设回空闲组合。在本地终端执行下面的命令，再使用自定义清屏键：

```sh
for n in 1 2 3 4 5; do printf 'clear-check-%s\n' "$n"; done
```

5. 预期之前的输出与滚动历史被清除，当前提示符保留；输入 `pwd` 仍正常执行。
6. 在 SSH 终端重复上述步骤，效果一致，连接不应被关闭。
7. 改一个“新建会话 / New session”快捷键，检查新键有效、旧键不再触发该操作，
   然后用重置箭头恢复默认。清屏默认是 macOS `Cmd+K`、Windows/Linux `Ctrl+Shift+K`。
8. 可选：在 `top` 等使用备用屏幕的程序中清屏，程序画面不应被清空。

Windows 的清屏保留 ConPTY 当前屏幕，仅清除滚动历史；与 macOS/Linux 的表现不同。

## 3. 目录、Files 与 Git

在本地终端准备文件；命令会创建一个新的独立测试目录，并打印路径：

```sh
MIO_LOCAL_CHECK=$(mktemp -d "$HOME/mioterm-local-check.XXXXXX")
mkdir -p "$MIO_LOCAL_CHECK/space dir" "$MIO_LOCAL_CHECK/downloads"
printf 'upload-check\n' > "$MIO_LOCAL_CHECK/upload.txt"
printf '%s\n' "$MIO_LOCAL_CHECK"
```

在应用连接的远程终端准备另一套目录：

```sh
MIO_REMOTE_CHECK=$(mktemp -d "$HOME/mioterm-remote-check.XXXXXX")
mkdir -p "$MIO_REMOTE_CHECK/space dir" "$MIO_REMOTE_CHECK/uploads"
printf 'remote-check\n' > "$MIO_REMOTE_CHECK/remote space'quote.txt"
cd "$MIO_REMOTE_CHECK"
printf '%s\n' "$MIO_REMOTE_CHECK"
```

1. 激活本地会话，点击 `+` 旁下拉箭头 → 选择文件夹新建会话。选择本地测试目录，
   新会话的 `pwd` 和 Files 根目录应一致。
2. 激活 SSH 会话，使用同一个菜单：应显示远程目录选择器。选择远程的 `space dir`，
   新会话应连接同一主机并进入该目录，不能变成本地会话。
3. 在两个会话中分别 `cd` 到其他测试目录，检查 Files 和 Git 跟随当前会话。
   切换会话后，不应残留另一台主机或本地目录的信息。
4. 在 SSH 会话激活时，从配置菜单选一个本地 shell，预期仍创建本地会话。
5. 在远程测试目录执行下面的 Git 样例，检查 Git 页显示新文件与改动，Files 出现标记：

```sh
git init
printf 'first\n' > git-check.txt
git add git-check.txt
git -c user.name=MioTermTest -c user.email=mioterm-test@example.invalid commit -m 'test fixture'
printf 'second\n' >> git-check.txt
```

可继续测试 Git 面板的暂存、取消暂存、差异和历史。只对这些测试文件执行丢弃。

## 4. 拖拽与 SFTP 任务

SSH Files 上方为本地目录，下方为远程目录，中间为可折叠的传输任务区。
将两端分别导航至刚才创建的测试目录。

1. 把远程的 `remote space'quote.txt` 拖到 SSH 终端。应插入正确转义的远程路径，
   随后直接输入字符应落在终端。可先输入 `ls -l ` 再拖拽并回车，验证路径确实可用。
2. 本地窗格右键 `upload.txt` → 上传，确认目标后执行；中间任务区应自动展开，
   显示 SFTP 通道、目标主机和任务状态，远程文件内容应为 `upload-check`。
3. 远程窗格右键测试文件 → 下载，检查本地目标文件与远程内容相同。
4. 直接把文件从本地窗格拖到远程窗格，再反向拖回本地。两种方向都先确认再传输。
   多选后右键或拖动，应保留选中的多个文件。
5. 从 Finder 拖入一个测试文件到远程文件区域，检查外部拖拽上传仍然有效。
6. 手动收起任务区，再新增传输任务，任务区应再次自动展开。
7. 传输时切换会话或收起右栏，任务仍使用入队时的主机和目标目录。
8. 用较大的测试文件验证取消、从头重试、清除已结束记录；清除记录不删除实际文件。

拖入文件窗格时，目标始终是对面窗格当前打开的目录，落在某一行上也相同。
要传入子目录，先进入该子目录。同名文件确认后会覆盖，目录会合并。
文件内容可在两端用 `cat`、`cksum` 或 SHA-256 检查。

任务目前按完成项数显示进度，没有字节百分比、速度或断点续传；重试从头开始。
退出应用不保留队列。详细用例见 [传输与重连测试](transfers-reconnect.md)。

## 5. 重连与状态动画

1. 给 SSH 会话重命名，进入一个测试目录，执行 `exit`。
2. 会话应留在原位置并显示断开；正常退出不会自动重连。
3. 点击重连图标或会话右键菜单的重连，名称、会话位置和远程目录应保留，
   Files/Git/监控重新跟随新连接。
4. 若需测自动重连，先在主机设置启用，重新建立连接，再对专用测试连接中断网络。
   SSH 检测断线后最多重试五次，间隔依次为 2、5、10、20、30 秒。
5. 等待时点停止图标，预期停止该会话的后续重试；初次登录失败、本地会话不会自动重试。

默认保活为 30 秒、连续三次无响应后断开，断网不一定立即显示断开。
重连建立新会话，不恢复远程进程，也不重放断线前的命令。

在本地或远程 zsh/bash 中查看约 8 秒的状态点动画：

```sh
printf '\033]9;4;3\007'; sleep 8; printf '\033]9;4;0\007'
```

若程序已经使用 OSC 7501，OSC 9;4 会按协议被忽略，可新开普通 shell 会话测试。

## 6. 背景与主题

1. 设置 → 外观中选择背景图片，分别调整窗口透明度和图片透明度，两者应独立生效。
2. 打开渐变，调整起始色、结束色和角度；检查图片和终端文字仍正常显示。
3. 关闭图片重复，打开遮罩，色标可填 `transparent, black`，调整角度、尺寸和位置。
4. 切换本地/SSH、Files/Git/监控，缩放窗口，检查内容可读、无遮挡、侧栏高亮仍跟随主题。
5. 关闭并重开设置，检查配置保留；原有同步页面仍存在。

遮罩仅用于不重复的单张图片；启用重复平铺时不会应用遮罩。

## 7. 可选：本地 CLI 控制

在设置 → 终端打开“命令行控制 / Command line control”，默认关闭。
在另一个本地终端中执行下列命令；`PID` 替换为第一条输出中目标会话的进程号：

```sh
/Users/ctexthuang/Desktop/code/ELYTerminal/target/release/elyterminal ctl sessions
/Users/ctexthuang/Desktop/code/ELYTerminal/target/release/elyterminal ctl read PID 20
/Users/ctexthuang/Desktop/code/ELYTerminal/target/release/elyterminal ctl send PID 'pwd'
/Users/ctexthuang/Desktop/code/ELYTerminal/target/release/elyterminal ctl key PID ctrl-c
```

`sessions` 的六列依次为：PID、当前激活标记、标题、目录、连接、连接状态。
本地连接为 `local`，SSH 连接形如 `ssh://user@host:22`，目录为远程 POSIX 路径。

1. 分别选本地和 SSH 的 PID 读写，预期只有指定会话收到内容。`send` 会附带回车执行。
2. SSH 中 `cd` 后重新列出会话，目录应更新。
3. SSH `exit` 后，`read` 可读保留输出，`send` 和 `key` 应明确提示已断开。
4. 重连后 PID 会改变，重新列出后再操作。
5. 关闭命令行控制后，新请求应失败，对应控制文件被移除。

隔离配置运行时，每条 CLI 命令也要传入相同的 `MIOTERM_CONFIG_DIR`。
控制令牌只用于本机，不进入 Gitee/WebDAV 同步归档。
Windows 发布版是窗口程序，测试 CLI 输出时需要管道或重定向。

## 8. 同步回归

本次融合未改变同步协议。使用专用测试仓库名或 WebDAV 测试路径与测试主机，
在两台更新到相同源码版本的设备上验证：

1. 新增测试连接并同步，另一台恢复后只出现一条；修改分组后同步，旧的未分组副本不应复活。
2. 删除测试连接并同步，另一台执行同步或恢复后也应删除；重启和再次同步不应复活。
3. 添加一条没有已保存密码的测试连接，其他连接仍能同步，界面明确提示缺失项。
4. 若启用凭据同步，用专用测试私钥验证另一设备可以恢复并连接，不依赖原设备的密钥路径。
5. 选“修改后同步”，保存连接改动后等待 10 至 29 秒；应先拉取合并再上传，
   界面显示结果和完成时间。单纯启动软件、保存同步设置不触发自动同步。

仅在另一台改动、本机没有改动时，本机需要“立即同步”或“恢复”接收变化；
当前没有每日轮询。完整规则见 [同步说明](ssh-sync.md)。

## 已有验证与边界

本次融合已在 macOS、Rust 1.95.0、runtime shaders 构建上完成：

- 698 项核心测试通过，1 项云端测试忽略，4 项环境相关测试跳过。
- 字体、加密凭据库、实际 askpass、加密 PEM 导入与源文件移除测试通过。
- 3 项实际回环 SSH 测试通过，覆盖远程 Git/SFTP、路径类型变化、远程目录与终端版本。
- 原生窗口验证了自定义快捷键、本地/SSH CLI、SSH 清屏、带空格和引号的路径拖拽及焦点。
- 本地到远程的界面拖拽上传、确认、任务区自动展开与文件内容一致性通过。
- 全 targets Clippy 拒绝警告、格式、空白和 500 行限制检查通过；release 构建通过。

Windows/Linux 原生运行和 Windows NSIS 安装包需要在对应系统继续实测。
Windows 输入空格黑块问题此次仅引入上游诊断日志，尚未认定修复。
自动检查未访问生产 SSH 主机或真实云端归档，双设备同步仍需上述手工回归。

开发者复查命令：

```sh
cargo fmt --check
cargo clippy --offline --locked --all-targets --features gpui_platform/runtime_shaders -- -D warnings
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib --bins --tests -- \
  --skip fish_loads_its_script_and_restores_its_data_dirs \
  --skip powershell_reports_its_folder_byte_for_byte_and_keeps_the_status \
  --skip under_wsl_each_shell_reports_its_folder_as_windows_names_it \
  --skip only_an_interactive_shell_leading_its_group_draws_its_prompt
```

四项跳过针对当前 macOS 的 shell/环境限制，完整环境应补测。
实际 SSH 集成测试的环境配置见 [完整测试指南](testing.md)。
