# AGENTS.md

vibegate —— herdr 终端会话的 Web 网关:把 herdr 管理的终端会话以画布窗口的形式暴露给浏览器(sshx 的替代品),支持手机/平板触控操作。

## 架构

**Rust (axum) 网关 + Svelte 5 画布前端**,单二进制发布。

```
src/                     Rust 网关
  main.rs    clap CLI(list/run[-d]/stop/status/tui/update)、路由装配、守护进程化
  herdr.rs   herdr Unix socket 客户端(NDJSON,一次一连接):
             attach、events.subscribe、pane.read/send_keys
  ws.rs      /ws 控制 WS + /term/{tid} 终端流(PTY 转发)
  pty.rs     portable-pty 会话管理
  term.rs    终端会话状态(带宽优化:仅聚焦窗口推送)
  files.rs   /api/files、/api/file(GET/PUT 可断点续传,.vbp sidecar)、
             /api/upload/status、/api/status、/api/shutdown(仅回环)
  tui.rs     `vibegate tui` 全屏仪表盘(ratatui-kit 组件,探针查 /api/status)
  update.rs  从 GitHub Releases 自更新
web/                     Svelte 5 + xterm.js 前端(rust-embed 打进 release 二进制)
  src/lib/      canvas.ts(窗口/焦点/抽屉状态)、session.ts(xterm 会话+剪贴板)、
                touchZoom.ts、connection.ts、mods.ts(粘滞修饰键)、i18n.ts(中英)、
                theme.ts/wezterm 主题、actions.ts(原生指针监听)
  src/components/  Canvas、TermWindow、SessionDrawer、FileDrawer、Keybar、
                ImageWindow、MdWindow、ConnectBar
```

部署形态:herdr 插件 pane 引导 `vibegate run`(herdr-plugin.toml + try-run.sh),浏览器连网关的 8480 端口。install.sh / `update` 子命令走 GitHub Releases(second-state/vibegate)。

## 构建与测试

```bash
cd web && npm run build        # 前端 → web/dist(release 由 rust-embed 打包)
cargo build                    # debug 从磁盘读 web/dist,改前端不用重编 Rust
cargo test                     # Rust 单元测试
cd web && npx vitest run       # 前端测试
```

发布用 build-release.sh;release.yml / prerelease.yml 交叉编译并传 GitHub Releases。

## 关键约定与坑(改动前必读)

- **Svelte 5 事件委托陷阱**:组件里的 `on:pointerdown|stopPropagation` 挡不住原生事件冒泡到 use-gesture。需要真正拦截时用原生监听器(web/src/lib/actions.ts 的 stopNative / nativePointerDown)。
- **element! 属性解析器限制**(ratatui-kit 0.12):props 表达式里不能有裸标识符和方法调用链,一律先算成局部变量再传;`Text` 不能直接做 Border 子节点,要包 View。
- **Linux Chromium 的 contextmenu 在右键按下瞬间触发**(早于知道是否在拖动)→ 画布直接禁用原生右键菜单。
- **非安全上下文(http:// LAN)**:navigator.clipboard 和 crypto.randomUUID 都不存在 → 剪贴板走 execCommand("copy") 兜底,随机数用 getRandomValues(web/src/lib/session.ts)。
- **rust-embed**:release 编译期打包 web/dist;debug 运行时读磁盘。
- **xterm 鼠标上报**:herdr 开鼠标上报后本地拖选失效,Shift+拖选强制本地选区(标准终端惯例)。
- 主题加新配色时同步更新 web/src/lib/theme.ts(含 wezterm 移植配色)。

## 工作规则

- 只做被明确要求的改动;回答问题/发现问题时不要顺手改文件。
- **commit ≠ push**:用户说 commit 就只 commit,push 必须等用户明说。
- commit 信息用英文祈使句,末尾加 `Co-Authored-By: Claude Code <noreply@anthropic.com>`。
