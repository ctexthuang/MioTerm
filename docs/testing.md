# MioTerm 测试指令与验收清单

适用于当前工作目录版本：MioTerm 0.0.9 + ELYTerminal 0.2.2，合并提交 `e4fdaa2` 及面板刷新修改。
这是本地开发版本；应用版本号仍为 0.0.9，不能仅凭版本号区分它与已发布的 v0.0.9。
上游融合范围见 [融合说明](upstream-integration.md)。

## 0. 本轮先测这些，约 15 分钟

项目测试窗口使用 `target/release/elyterminal` 及独立配置：
`/private/tmp/mioterm-022-retest.PFEoUY/config`。
原有已安装应用与 SSH 会话保留；新窗口的保存主机列表为空是正常现象。
本轮测试未复制原有主机、密码或云端配置，自动同步关闭。
更新可执行文件不会更新已经运行的进程；测试新增刷新功能前应重开测试窗口。

| 顺序 | 怎么测 | 通过标准 |
| --- | --- | --- |
| 1 | 左右栏各展开、收起一次 | 启动时均关闭，图标切换正确，Mocha 高亮为粉色 |
| 2 | 终端执行 `pwd`，再输入中文 | 默认从用户目录启动，文字与光标对齐 |
| 3 | 设置 > 外观，调整界面字号、字重、中日韩字体 | 即时生效，文字不重叠、不缺字；详见第 2 节 |
| 4 | 设置 > 终端 > 配置文件，添加启动项 | 左栏顶部下拉菜单可启动，参数和目录正确 |
| 5 | 按第 4 节创建测试仓库，打开右侧 Files | 新增、删除文件约 1 秒后出现，Git 标记跟随 |
| 6 | 把带空格、单引号的文件拖到终端 | 输入经过转义的完整路径，不自动执行 |
| 7 | 本地／SSH 分别右键 Files 图标，选择目录新建会话 | 在对应机器的所选目录启动，原会话不变 |
| 8 | 两个不同目录的会话来回切换 | Files/Git 跟随当前会话，不串目录 |
| 9 | 在 zsh 中缩放窗口、输入长命令 | 不因缩放反复新增相同提示符，编辑不丢字 |
| 10 | 打开设置 > 同步，切换 Gitee/WebDAV | 页面与控件正常，未配置服务时先不点同步 |

先完成以上项目，再用专用服务器测试第 5 至 9 节。不要把生产文件用于覆盖、
Discard 或取消传输测试。独立配置仍使用本机 Shell 与 SSH 环境。

最近一轮已验证：581 项核心测试、5 项监控脚本测试、PEM 导入、加密凭据库、
实际 askpass helper、Clippy 和构建通过。原生截图与隔离回环 SSH 验证了文件
刷新、Git 标记、两端 Git 分支与历史自动更新、提交草稿保留及远程目录新会话。
在线 Gitee、生产 SSH 和 Windows/Linux 原生运行未执行；4 项 Shell 环境测试
被过滤，在线测试仍默认忽略。

## 1. 启动当前源码

先关闭此前的独立测试窗口；保留已安装应用及工作 SSH 会话，在 macOS 系统终端重开：

```sh
cd /Users/ctexthuang/Desktop/code/ELYTerminal
env MIOTERM_CONFIG_DIR=/private/tmp/mioterm-022-retest.PFEoUY/config \
  target/release/elyterminal
```

此目录位于临时目录，系统清理后不能继续复用。需要全新的独立配置时：

```sh
cd /Users/ctexthuang/Desktop/code/ELYTerminal
MIO_CONFIG=$(mktemp -d /tmp/mioterm-config.XXXXXX)
printf '测试配置目录：%s\n' "$MIO_CONFIG"
env MIOTERM_CONFIG_DIR="$MIO_CONFIG" target/release/elyterminal
```

源码变化后才需要重新构建：

```sh
cd /Users/ctexthuang/Desktop/code/ELYTerminal
cargo build --offline --locked --release --features gpui_platform/runtime_shaders --bin elyterminal
```

离线构建需要依赖已经缓存。直接运行二进制不会再生成多个同名 `.app` 图标。
不要让多个实例同时写同一配置目录。去掉 `MIOTERM_CONFIG_DIR` 会使用原有
应用数据，并可能触发已开启的启动自动同步；需要使用原有数据时先退出旧实例。
独立配置只隔离应用配置和凭据库，不隔离 Shell、`~/.ssh` 或系统环境。

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

### 本次新增的外观设置

按 `Cmd-,` 打开设置 > 外观，修改以下项目后回主窗口观察，再重开设置确认值保留。

| 项目 | 操作 | 预期 |
| --- | --- | --- |
| 界面字号 | 依次选 12、14、16 px | 侧栏、设置、工具提示调整，终端文字字号不随之改变 |
| 界面字体 | 选一种已安装字体，再恢复系统字体 | 中文与图标可读，无裁切或重叠 |
| 字重 | 在字体区选 400、500、700 | 按字体可用字面改变粗细，字符间距与光标位置稳定 |
| 中日韩字体 | macOS 可选 PingFang SC，再点清除 | 只改变 CJK 字体选择；拉丁文字与图标正常 |
| 平滑移动 | 在光标区开关，返回终端输入、按方向键 | 开启有平移动画，关闭直接跳格，无残影 |
| 抗锯齿 | Windows/Linux 对比灰度与次像素 | 次像素需窗口不透明；macOS 不显示此选项是正常行为 |

字体对比命令：

```sh
printf 'Latin ABC 123 | 中文 | 日本語 | 한글 | e\314\201\n'
printf '\033[1mBold 粗体\033[0m  Normal 普通\n'
```

### 启动配置与目录

1. 设置 > 终端 > 配置文件，点击添加，名称改为 `MioTerm Manual Test`。
2. 命令行填写下方内容，文件夹选择一个已存在的专用测试目录。
3. 展开左侧栏，点击顶部 `+` 旁的下拉箭头，选择该启动项。
4. 应出现 `PROFILE_OK` 和选定目录，随后进入可输入命令的 shell。
5. 复制这个本地会话，确认参数保留；删除测试启动项，菜单不再显示它。

```sh
/bin/sh -c 'printf "PROFILE_OK\n"; pwd; exec /bin/sh'
```

带参数的命令按原样运行，不注入 Shell 集成，Files 不保证跟随之后的 `cd`。
要测试自动目录跟随，请另建命令为 `/bin/zsh` 或 `/bin/bash`、不带参数的配置。
文件夹留空时遵循“起始位置”；无效命令或不存在的目录应显示拒绝保存的原因。
下拉菜单还应列出本机可用 shell；Git Bash/WSL 项目需要对应的 Windows 环境。

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

### zsh 缩放与光标

在默认 zsh 会话中输入一条较长但暂不回车的命令，左右移动光标并缩放窗口。
预期：内容保持完整，宽字符与光标对齐，不因缩放反复向历史添加相同提示符。
按 `Ctrl-C` 清除草稿，再运行 `false` 和 `true`，观察会话失败状态能随新命令清除。

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

### 自动刷新、拖动与选择目录

保持右侧 Files 可见，在终端继续执行：

```sh
printf 'auto refresh\n' > refresh-check.txt
mv refresh-check.txt refresh-renamed.txt
printf 'quoted path\n' > "it's a file.txt"
```

预期：约 1 秒后出现新文件，重命名后旧名称消失，Git 标记更新。
把 `it's a file.txt` 拖到终端空输入行，应输入转义后的完整路径并留在输入行，
不要直接回车执行它；按 `Ctrl-C` 清除草稿。再拖一个目录，确认路径同样正确。

设置 > 终端开启“文件菜单”，右键右上角 Files 图标，应出现
“选择文件夹新建会话…”；选另一测试目录，新会话进入该目录，原会话仍存在。
从托管 SSH 会话使用此入口时，应打开服务器目录选择器，按第 5 节验证。

命令行目录参数可在系统终端另行验证，运行前先退出使用同一测试配置的实例：

```sh
cd /Users/ctexthuang/Desktop/code/ELYTerminal
env MIOTERM_CONFIG_DIR=/private/tmp/mioterm-022-retest.PFEoUY/config \
  target/release/elyterminal --working-directory /private/tmp/mioterm-022-retest.PFEoUY/fixtures
```

新窗口 `pwd` 应为所选目录。`-d`、`--startingDirectory` 和直接跟目录参数也应可用；
传入不存在的目录应明确报错，不悄悄改到其他目录。

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

### 每秒刷新与远程目录新会话

1. 保持 Files 可见，在远程终端创建、重命名、删除测试文件；约 1 秒加请求耗时后更新。
2. 在上方本地窗格的目录中做同样操作，本地列表与 Git 标记也应更新。
3. 修改 `remote.txt`，检查下方远程窗格的 `M` 标记；子目录内的改动应给目录加点。
4. 保持 Git 页可见，用终端暂存、提交或新建文件；状态、分支与历史自动更新。
5. 连续暂存、取消暂存，输入提交草稿、打开 Diff；刷新不清空草稿，不用旧结果覆盖新状态。
6. 地址栏输入一半先不回车，等待几秒；刷新不改写输入，列表选中项也保留。
7. 开启“文件菜单”，右键 Files 图标，点击“选择远程目录新建 SSH 会话…”。
8. 确认显示当前主机；单击选目录、双击进入，上级／主目录按钮与路径输入都可导航。
9. 选带空格或单引号的目录并新建，执行 `hostname; pwd`，仍是原 SSH 主机和所选目录。
10. 取消应不建会话；原 SSH 断开或关闭后不可确认。切回本地后入口使用本地文件夹选择器。

隐藏右栏或切换到其他页时停止周期读取；慢连接不叠加同一窗格的请求。
上传确认或传输期间暂停文件轮询；切换主机后迟到的结果不能串入当前面板。
远程刷新需要托管 SSH 连接及服务器 Python 3、Git，不依赖重新出现 Shell 提示符。

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

### SSH 连接保活

1. 在添加／编辑主机中找到“SSH 连接保活”，默认开启，间隔 30 秒。
2. 改为 75 秒并直接点击保存，重开编辑确认；也测试数字输入框的上下步进。
3. 关闭保活、保存并重开，开关应关闭，75 秒的间隔保留；重新开启后仍用该值。
4. 新建 SSH 连接，终端、SFTP、Files、Git、监控使用一致参数；旧会话需重连才生效。
5. 云同步后本机保活选项不被覆盖；另一台首次恢复主机使用默认值。

保活使用 OpenSSH 的 ServerAliveInterval/ServerAliveCountMax，在无服务器流量时检查。
关闭将显式设置 ServerAliveInterval=0，不影响操作系统 TCP 保活；该功能不会自动重连，
也无法保证断网后继续连接或绕过服务器的主动退出策略。

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
