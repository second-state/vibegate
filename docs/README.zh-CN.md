# vibegate

单二进制的 Web 网关,把 [herdr](https://github.com/herdrdev/herdr) 管理的终端会话
暴露成无限画布 UI——sshx 的移动端友好替代品。终端、图片、Markdown(含 mermaid
图表)都是可拖拽的画布窗口,手机和桌面都能平移、缩放、整理。

发布版二进制内嵌了全部网页资源:拷到哪都能跑,无需任何外部文件。

> English: [README.md](../README.md)

## 前置条件

- 一个在跑的 [herdr](https://github.com/herdrdev/herdr) 服务端(socket 默认
  `~/.config/herdr/herdr.sock`;识别 `HERDR_SOCKET_PATH` / `HERDR_CONFIG_ROOT`)
- 一个浏览器(桌面或手机)

## 快速开始

```bash
./build-release.sh            # 把 web/dist 内嵌进二进制
./target/release/vibegate run # 前台运行;Ctrl-C 停止
```

打开 `http://localhost:8480`。要给局域网(比如手机)用:

```bash
vibegate run --bind 0.0.0.0:8480
```

### 安装 / 更新

```bash
./install.sh                  # 从 GitHub Releases 装最新版到 ~/.cargo/bin
vibegate update               # 自更新
```

发布仓库默认是 `second-state/vibegate`,托管在别处的话改 `install.sh` 的
`REPO` 和 `src/update.rs` 里的 `REPO` 常量。

## 命令行

```
vibegate run [-d] [--bind ADDR] [--file-root DIR]... [--web-dir DIR]
vibegate stop   [--bind URL]
vibegate status [--bind URL]
vibegate update [TAG]
vibegate list
```

| 命令 | 作用 |
|---|---|
| `run` | 前台启动网关 |
| `run -d` | 以守护进程方式启动,立即返回 |
| `stop` | 优雅关停一个在跑的网关(仅限本机来源) |
| `status` | 显示网关是否在跑、什么版本 |
| `update [TAG]` | 从 GitHub Releases 自更新(默认最新稳定版) |
| `list` | 打印网关能看到的 herdr 面板(调试用) |

常用参数与默认值:

- `--bind` — 监听地址,默认 `127.0.0.1:8480`
- `--file-root DIR` — 文件 API 允许访问的目录;可重复,默认 `$HOME`
- `--herdr-socket PATH` — herdr socket 覆盖

环境变量:`VIBEGATE_BIND`、`VIBEGATE_HERDR_SOCKET`、`VIBEGATE_LOG_FILE`
(守护进程日志位置)、`RUST_LOG`(日志级别,默认 `info`)。

## 守护进程

```bash
vibegate run -d --bind 0.0.0.0:8480   # 后台启动,立即拿回终端
vibegate status                       # vibegate 0.1.0 — running
vibegate stop                         # 优雅关停
```

守护进程日志追加在 `~/.vibegate/logs/vibegate.log`(可用 `VIBEGATE_LOG_FILE`
覆盖;单文件 10MB 轮转,保留 5 天)。重复 `run -d` 会报已运行的版本,不会双开。
前台 `run` 保留给 systemd 这类 supervisor。

## 网页界面

- **连接** — 默认连页面同源;☰ 菜单里可改网关地址
- **会话**(右上)— herdr 工作区树;`+` 新建终端,点行打开到画布,`×` 关闭
  pane;agent 状态原样显示(`idle / working / blocked / done / unknown`)
- **画布** — 拖动平移,双指捏合(或 `ctrl`+滚轮)缩放,双击窗口居中
- **窗口标题栏** — 拖动移动,右下角缩放,`×` 关窗口(远端会话继续跑),
  `⟳` 重连断开的流
- **辅助键** — 悬浮在被激活终端下方:粘滞 `Ctrl / Alt / Shift`(下一次按键,
  按钮或软键盘都算,附带该修饰键)、方向键、滚动键、`1×` 缩放复位。位置和
  显隐在菜单里调(悬浮 / 置顶 / 隐藏)
- **✋ 画布模式** — 所有窗口点击穿透:拖动、捏合都变成操作画布。点按钮或
  **任意位置双指轻点**切换;激活时辅助键自动隐藏
- **文件** — 浏览设定的根目录;`⇧ 上传` 传文件(不限大小、支持断点续传,
  失败出现 `↻ 重传` 按钮);`⇩` 下载;点图片或 `.md` 打开成画布窗口
  (Markdown 渲染表格、代码和 mermaid 图表)
- **菜单** — 终端主题(Tokyo Night、Dracula、Nord、Gruvbox、One Dark、
  Sakura、Sequoia Moonlight)、画布/终端背景图与透明度、辅助键位置、
  中文 / English

## HTTP API

| 路径 | 方法 | 用途 |
|---|---|---|
| `/ws` | WS | 控制通道:pane/workspace 列表、创建/关闭 |
| `/term/{terminal_id}` | WS | 终端字节流(`?cols=&rows=`) |
| `/api/files?path=` | GET | 目录列表(限定在 file-root 内) |
| `/api/file?path=` | GET | 下载文件(流式) |
| `/api/file?path=` | PUT | 上传——不限大小,`X-Upload-Token` / `X-Upload-Total` 断点续传 |
| `/api/upload/status?path=&token=` | GET | 查询断点已收字节数 |
| `/api/status` | GET | `{"name":"vibegate","version":"…"}` 存活探测 |
| `/api/shutdown` | POST | 优雅退出——**仅限本机来源** |

手动关停请用 `vibegate stop`,不要直接打 `/api/shutdown`。
