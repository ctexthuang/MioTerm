# MioTerm 测试指令与验收清单

适用于当前工作目录版本：MioTerm 0.0.8 + ELYTerminal 0.2.0 上游合并及审查修复。
版本号尚未提升；本地测试包不等于 GitHub 上已发布的 v0.0.8。

## 1. 启动当前源码

在 macOS 的系统终端执行：

```sh
cd /Users/ctexthuang/Desktop/code/ELYTerminal
cargo run --locked --release --features gpui_platform/runtime_shaders
```

以应用包启动，保留桌面图标与应用身份：

```sh
cd /Users/ctexthuang/Desktop/code/ELYTerminal
env CARGO_NET_OFFLINE=true scripts/mac-app.sh --features gpui_platform/runtime_shaders
open -n "$PWD/target/release/MioTerm.app"
```

离线构建需要依赖已经缓存。`open -n` 打开新实例，不会替你关闭旧实例。
避免多个实例同时编辑配置，测试前自行确认旧窗口已完成任务。
默认使用已有应用数据，可能执行已启用的启动自动同步。

不接触现有应用配置的独立实例：

```sh
cd /Users/ctexthuang/Desktop/code/ELYTerminal
MIO_CONFIG=$(mktemp -d /tmp/mioterm-config.XXXXXX)
printf '测试配置目录：%s\n' "$MIO_CONFIG"
env MIOTERM_CONFIG_DIR="$MIO_CONFIG" target/release/MioTerm.app/Contents/MacOS/elyterminal
```

此方法隔离应用配置和凭据库，不隔离用户的 Shell、`~/.ssh` 或系统环境。
独立配置没有原来保存的主机。关闭实例后可保留目录供复测，不要上传其内容。

## 2. 启动与界面

- 启动后左右栏均关闭；点击对应图标才展开，再点击可收起。
- 右侧监控、文件、Git 图标切换正确；高亮跟随主题。
- Catppuccin Mocha 的选中高亮是粉色；桌面人物正常显示。
- 设置中切换中英文即时生效，Sync 页面仍存在。
- 设置中改变字体、字号、背景、透明度或创建自定义主题，终端即时更新。
- 拖动窗口、改变大小和显示器位置；重启后窗口可见且位置合理。
- 拖动会话排序后，当前终端仍对应原会话；复制 SSH 会话仍连接原主机。

常用 macOS 按键：`Cmd-T` 新建，`Cmd-W` 关闭，`Cmd-,` 设置，
`Cmd-B` 左栏，`Cmd-Alt-B` 右栏，`Cmd-F` 查找，`Ctrl-Tab` 切换会话。
其他平台见 [快捷键](zh_cn/keys.md)。

## 3. 本地终端

以下命令在 MioTerm 新建的本地终端中运行。

### 默认目录、字符与样式

```sh
printf 'HOME=%s\nPWD=%s\n' "$HOME" "$PWD"
printf '中文宽字符 ABC 123\n组合字符：e\314\201\n'
printf '\033[31m红色\033[0m \033[32m绿色\033[0m \033[1m粗体\033[0m \033[4m下划线\033[0m\n'
printf '\033[38;2;245;194;231mMocha Pink\033[0m\n'
printf '\033]0;MioTerm Local Test\007'
```

预期：默认 Start in 为 Home 时，首个本地会话位于用户目录；字符不乱码，
光标与文字对齐，标题更新。自定义 Start in 设置应以其配置为准。

### 输入、选择、查找与滚动

```sh
awk 'BEGIN { for (i = 1; i <= 300; i++) printf "mioterm-find-%03d 中文测试\n", i }'
```

使用滚轮浏览历史；`Cmd-F` 搜索 `mioterm-find`，Enter 和 Shift-Enter 切换匹配。
拖动选择、复制、粘贴；粘贴中文、多行文本、带空格路径，确认不会漏字或重复输入。
用 `cat` 测试输入法组合与多行粘贴，完成后 `Ctrl-D` 退出。

### 图片

```sh
python3 - <<'PY'
import base64
import sys

pixels = bytes([245, 194, 231, 255]) * (16 * 16)
payload = base64.b64encode(pixels).decode("ascii")
sys.stdout.write(f"\033_Ga=T,q=2,f=32,s=16,v=16,i=901,c=8,r=4;{payload}\033\\\n")
sys.stdout.flush()
PY
```

预期：显示一块粉色图片，不把协议文本打印为普通字符，也不往 Shell 命令行
写入 `Gi=901;OK`。`q=2` 关闭协议回执，末尾换行避免 zsh 的 `%` 无换行标记。
此命令也可在托管 SSH 终端内运行，直接传输图片仍应正常显示。清理测试图片：

```sh
printf '\033_Ga=d,d=I,i=901,q=2\033\\'
```

## 4. 本地 File 与 Git

在同一个本地终端创建专用测试目录，不在真实项目里执行这些 Git 操作：

```sh
MIO_TEST_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/mioterm-test.XXXXXX")
mkdir -p "$MIO_TEST_ROOT/upload/sub folder" "$MIO_TEST_ROOT/download" "$MIO_TEST_ROOT/repo"
printf 'MioTerm upload test\n中文文件\n' > "$MIO_TEST_ROOT/upload/sub folder/中文 file.txt"
printf 'MioTerm single file\n' > "$MIO_TEST_ROOT/upload/single.txt"
printf '本地测试目录：%s\n' "$MIO_TEST_ROOT"
cd "$MIO_TEST_ROOT/repo"
git init
git -c user.name='MioTerm Test' -c user.email='test@example.invalid' commit --allow-empty -m 'test: initial'
git config user.name 'MioTerm Test'
git config user.email 'test@example.invalid'
printf 'first\n' > tracked.txt
git add tracked.txt
git commit -m 'test: tracked file'
printf 'second\n' >> tracked.txt
printf 'new\n' > untracked.txt
```

- 右侧 File 随 `cd` 定位到 `repo`；新文件和 Git 标记正确。
- Git 显示 `tracked.txt` 的修改与 `untracked.txt`，Diff 内容正确。
- 测试暂存、取消暂存、提交、历史、amend；提交仅作用于此测试仓库。
- 修改测试文件后测试 Discard：先取消确认，再确认，观察内容变化。
- `cd ..` 离开仓库后面板清空或显示非 Git 仓库，不残留可操作的旧状态。
- 切换两个不同目录的会话，File/Git 分别跟随自己的会话。

## 5. 托管 SSH、远程 File 与 Git

从左侧主机管理或 Quick SSH 连接测试服务器。不要在普通本地终端里输入
`ssh ...` 代替这个入口：后者不会创建远程面板所需的连接元数据。

服务器需要 Python 3、Git、可用的 SFTP 子系统，以及 bash、zsh 或 fish 登录 Shell。
在已经连接的远程终端执行：

```sh
hostname
whoami
pwd
command -v python3 git
MIO_REMOTE_ROOT=$(mktemp -d /tmp/mioterm-remote.XXXXXX)
mkdir -p "$MIO_REMOTE_ROOT/incoming" "$MIO_REMOTE_ROOT/repo"
printf '远程测试目录：%s\n' "$MIO_REMOTE_ROOT"
cd "$MIO_REMOTE_ROOT/repo"
git init
git config user.name 'MioTerm Test'
git config user.email 'test@example.invalid'
printf 'remote first\n' > remote.txt
git add remote.txt
git commit -m 'test: remote initial'
printf 'remote second\n' >> remote.txt
```

- 每次 `cd` 后远程 File/Git 都跟随服务器目录，不读取本机同名路径。
- 远程 Git 的 Diff、暂存、取消暂存、提交和历史正确。
- 在远程终端运行 `git status --short`、`git log -3 --oneline` 验证面板操作。
- File 中导航不应偷偷在终端输入 `cd`；下一个真实 Shell 提示符可重新跟随。
- 切换本地/远程会话，面板不会把某台服务器的结果显示在另一会话。
- 复制远程会话，主机、用户名和起始目录正确；断开后不可继续启动传输。
- 首次连接仍需核对主机指纹，错误指纹不能自动接受。

## 6. 上传、下载、拖放与取消

本地 File 地址栏填入第 4 节输出的 `.../upload`；远程地址栏填入
第 5 节输出的 `.../incoming`。跨会话不共享 Shell 变量，请使用输出的实际路径。

1. 选中 `single.txt`，点击上传并确认；远端应出现文件，传输状态明确。
2. 上传 `sub folder`，验证递归目录、空格和中文文件名。
3. 从 Finder 拖动本地测试文件或目录到远程 File 区域，先取消一次，再确认。
4. 本地面板切到 `.../download`，选择远程测试文件并下载。
5. 对同名测试文件重复上传，应出现覆盖提示；不要拿生产文件验证覆盖。
6. 用一个较大的测试文件验证取消；取消后可能留下部分文件，不承诺断点续传。

在本地原文件与下载文件分别计算 SHA-256，比对结果相同：

```sh
shasum -a 256 '/实际本地测试目录/upload/single.txt'
shasum -a 256 '/实际本地测试目录/download/single.txt'
```

Linux 远端可用：

```sh
sha256sum "$MIO_REMOTE_ROOT/incoming/single.txt"
```

## 7. 密码与 PEM 私钥

- 密码模式：勾选记住密码，保存后连接；关闭应用再打开，使用同一主机重连。
- 私钥模式：显式选择测试用 `.pem` 或 OpenSSH 私钥；有口令的密钥填写其口令。
- 保存后私钥导入应用管理目录；先验证连接成功，再移动测试用源文件并复测。
- 只移动专门生成的测试密钥，不移动、删除或覆盖你的生产密钥。
- 错误密码、错误私钥口令或不可用的 agent，应明确失败，不无声停住。
- 自动填充不应替代 MFA/OTP 输入，也不应跳过未知主机指纹确认。

需要新测试密钥时，在本地执行；先让测试服务器信任其公钥，再连接：

```sh
MIO_KEY_ROOT=$(mktemp -d /tmp/mioterm-key.XXXXXX)
ssh-keygen -t rsa -b 3072 -m PEM -f "$MIO_KEY_ROOT/test.pem" -C 'mioterm-fixture'
```

按交互提示设置口令，不把真实密码或私钥内容粘进命令、日志或聊天。

## 8. Gitee / WebDAV 同步与跨机恢复

此节会真实上传数据。使用专用测试账号或独立 vault/目录，先确认保存的主机
都是准备同步的内容。同步采用追加合并，但会尊重本机删除记录；不会向其他
设备传播删除，也不是覆盖式镜像。测试时只运行一个使用相同配置目录的实例。

1. 设置 > Sync，选 Gitee 或 WebDAV；填写服务凭据及独立测试 vault/目录。
2. 加密口令输入两遍，恰好 6 个字符应通过；正式使用推荐更长的独立口令。
3. 开启记住加密口令；需要跨机免重输 SSH 密码时，开启同步 SSH 凭据。
4. Save 后 Sync now；验证成功状态。Save 本身不等于上传。
5. 重开应用，记住的秘密字段可以留空，Sync now 仍应成功。
6. 开启本地修改后自动同步，Save；新建测试主机或改变测试分组，等待约 2 秒。
7. 仅打开终端、切换会话或更新最近记录，不应新增同步快照。
8. 用另一台设备或第 1 节的独立配置目录，填写同一服务、vault 和加密口令。
9. 开启凭据包含选项后 Restore；恢复主机、分组、实际私钥内容及选择同步的凭据。
10. 在新设备连接恢复的测试主机；不应依赖源机器上的绝对路径或钥匙串。
11. 错误口令必须明确失败；同一主机的不同凭据应报告冲突，不得静默覆盖。
12. 关闭自动同步后排队任务不再重试，已开始的网络请求可能仍完成。
13. 混合保存有密码、无已存密码、agent 和无口令私钥主机，均应同步连接信息；
    缺少已记住的密码时提示警告，但不阻塞其他主机。
14. 只对测试主机移走显式选择的密钥文件，该主机跳过本地导出并提示警告；
    本地记录不删除，其他主机仍可同步，云端已有密钥和密码不应丢失。
15. 如果云端有该主机的完整凭据，Sync now 也应恢复密码和丢失的密钥；
    损坏的密钥、凭据库或真正的凭据冲突仍应报错。
16. 删除已同步的测试主机，切换界面、Sync now、Restore，再重启应用；该主机
    不应重新出现，其密码和密钥也不应重新导入。其他主机仍正常同步。
17. 主动重新添加同一主机后可以再次同步；另一独立配置目录仍能恢复云端旧记录，
    本机删除不会清空备份或删除其他设备的主机。

Gitee 私密片段里只能看到密文；不应出现明文密码、私钥或本地解密 identity。
主机删除、远端单独发生的修改，不会靠持续轮询自动传播。
更换云端加密口令时使用新 vault/目录，避免旧快照无法解密。
完整语义和限制见 [同步说明](ssh-sync.md)。

## 9. 监控

- 本地/SSH 会话分别显示对应机器的统计；SSH 监控目前要求 Linux 服务器。
- 网络图持续观察 1 分钟，坐标、布局和图例不应每秒跳动或重叠。
- 测试暂停、恢复、刷新、网卡筛选；CPU/内存进程排序可切换。
- 切换会话后不应继续显示另一台机器的采样。
- 断开连接或权限不足时应显示错误，而不是伪造零值。

## 10. 开发者回归命令

以下在项目根目录执行。macOS 使用 release + runtime_shaders；其他平台通常
去掉 `--features gpui_platform/runtime_shaders` 即可，使用 Rust 1.95.0。

### 静态检查

```sh
cargo fmt --check
cargo clippy --offline --locked --release --features gpui_platform/runtime_shaders --all-targets -- -D warnings
git diff --check
sed -n '7,$p' scripts/check.sh | sh
```

最后一条只运行现有检查脚本的注释与文件行数规则；完整脚本是 `scripts/check.sh`，
但它不带本机所需的 runtime_shaders，也不跳过缺失的 Shell，不能把环境失败当成功。

### 本机完整核心回归

```sh
cargo test --offline --quiet --locked --release --features gpui_platform/runtime_shaders --lib --bins --tests -- \
  --skip fish_loads_its_script_and_restores_its_data_dirs \
  --skip powershell_reports_its_folder_byte_for_byte_and_keeps_the_status \
  --skip under_wsl_each_shell_reports_its_folder_as_windows_names_it \
  --skip only_an_interactive_shell_leading_its_group_draws_its_prompt
```

这 4 项依赖本机没有的 Shell 或特定运行环境；具备条件后单独补跑，
完整环境可去掉所有 `--skip`。PTY、回环端口和共享内存测试需要系统权限。
云端 Gitee 和两个显式 SSH 集成测试仍默认忽略。

### 按模块定位

```sh
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib term:: -- --nocapture
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib ssh:: -- --nocapture
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib git:: -- --nocapture
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib monitor:: -- --nocapture
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib vault:: -- --nocapture
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --test ssh_auth
python3 tests/monitor_probe.py
```

`term::` 整模块可能包含上述环境依赖测试，可追加相同的 `--skip`。
`ssh_auth` 默认隔离应用目录，验证凭据库、跨进程写入、askpass 与 PEM 导入；
未提供专用 SSH fixture 时，它不会自动验证真实服务器登录。

### 本次审查的安全与目录回归

```sh
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib remote_ -- --nocapture
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib shared_memory_is_read_then -- --nocapture
```

覆盖远程图像拒绝本机文件、临时文件和共享内存，分块传输中断后恢复，
直接传输图片仍可显示，以及托管 SSH 的 OSC 7 始终作为远程 POSIX 目录。
这些防护不自动识别普通本地终端里手动启动的 `ssh`。

### 专用回环 SSH 与真实云端测试

`tests/remote_ssh.rs` 需要独立的本机测试 SSH 服务和已认证 ControlMaster。
不要指向生产服务器或正在使用的用户会话：

```sh
export ELY_SSH_TEST_PORT='实际回环测试端口'
export ELY_SSH_TEST_SOCKET='/专用测试连接的/control.sock'
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --test remote_ssh -- --ignored --nocapture
```

服务与 socket 的建立不包含在该命令中；测试要求回环地址，并会创建临时测试仓库。
真实 PEM 登录测试另需 `ELY_SSH_TEST_IDENTITY` 和 `ELY_SSH_TEST_KNOWN_HOSTS`。

以下会读取应用保存的 Gitee token，创建并清理它自己的测试私密片段，只有在
明确接受真实云端写入后才手动运行。它不上传用户的主机和私钥：

```sh
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib ssh::sync::gitee::live:: -- --ignored --nocapture
```

### 截图与其他平台

macOS 自动截图示例，需要屏幕录制权限，且不等于完整交互验收：

```sh
cargo build --offline --locked --release --features gpui_platform/runtime_shaders --example shot
MIO_SHOT_CONFIG=$(mktemp -d /tmp/mioterm-shot.XXXXXX)
env MIOTERM_CONFIG_DIR="$MIO_SHOT_CONFIG" target/release/examples/shot --keys 'wait' /tmp/mioterm-smoke.png /bin/sh -c 'printf "MioTerm smoke test\n"; sleep 8'
```

Linux 容器回归需要 Docker：`scripts/linux.sh test --locked`。
Windows/Linux 原生输入、窗口、SFTP 和安装包需要在对应系统补验；
macOS 的 `--all-targets` 只检查本机目标下的所有 Cargo target，不代表三端编译通过。

## 11. 反馈问题

记录：应用路径、版本/提交、操作系统、复现步骤、期望与实际结果，以及是否使用
托管 SSH。截图应遮住 token、密码和私钥。不要上传应用凭据目录或整个配置文件。
开发日志可用 `RUST_LOG=elyterminal=debug` 启动，但可能包含路径、命令和主机信息，
分享前先脱敏；避免用带秘密的命令验证日志。
