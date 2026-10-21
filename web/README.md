# vibeview

vibetty 终端会话的 **MQTT Web 查看端**——一个移动优先的"无限画布"终端查看器。浏览器通过
MQTT-over-WebSocket 连上 broker,自动发现所有在线的 vibetty 实例,以多终端窗口画布的方式
实时查看与操作,和 [vibekeys](../vibekeys_firmware) 设备看到的是同一批会话。

```
                     MQTT broker (rumqttd / mosquitto / EMQX)
                        │  TCP 1883            │  WS :9001 (/mqtt)
        vibetty ◄───────┘                      └──────► vibeview (浏览器)
       (发布终端)                                     多窗口画布 · 键盘输入
          ▲
          └──── vibekeys 设备也能订同一批会话
```

## 功能

- **自动发现**:订阅 retained presence(`+/+/+/vibetty`),实时列出在线实例
  (标题、agent 状态 working/waiting、LWT 下线即时移除、`ts` 超时兜底)
- **真终端仿真**:text 模式 `screen_text` 流(0x00 全屏基线 / 0x01 PTY 增量)喂给
  xterm.js,完整 ANSI 颜色与转义;重连自动重订阅 + 换新基线
- **无限画布**:多终端窗口自由拖动/缩放,双指捏合与滚轮缩放、空白拖动平移
  (手势层移植自 [sshx](https://github.com/ekzhang/sshx),MIT)
- **省流量**:只有聚焦窗口向服务端请求推流(`sync close=false`),其余窗口
  `close=true` 暂停;切走即静音,切回即恢复
- **输入**:xterm 键盘直输(含 IME 中文、粘贴)+ 移动端快捷键条
  (方向键/Esc/Tab/^C/^D/^Z/⏎/⌫ + 服务端 scrollback 滚动)
- **只读模式**:窗口一键切换 👁,只看不发
- **独立部署**:纯静态 SPA(`dist/`),任意静态主机可跑;broker 地址支持
  `?broker=` 参数与 localStorage 记忆

## 开发

```bash
npm install
npm run dev        # 开发服务器
npm run build      # 产物 → dist/
npm run preview    # 本地预览产物
npm run check      # svelte-check
```

## 连接

页面顶部状态栏点开填 broker WebSocket 地址,形如:

```
ws://<host>:9001/mqtt     # 明文(http 页面)
wss://<host>:9002/mqtt    # TLS(https 页面必须,否则 mixed content 被浏览器拦截)
```

- 路径须以 `/mqtt` 结尾
- vibetty 内置 rumqttd 的 WS 端口由 `~/.vibetty/config.toml` 的
  `[mqtt] builtin_ws_port` 决定(默认 0 = 不开)
- 账号密码可选(与 broker 配置一致);`user` 段 = broker 用户名(未填则 `root`)

也可用 URL 参数直接带地址:`https://<host>/vibeview/?broker=wss://broker:9002/mqtt`

## 部署

`npm run build` 后把 `dist/` 丢给任意静态服务器即可(nginx、GitHub Pages、
`python -m http.server`……)。注意:

1. **https 页面只能连 wss://**——broker 需要配 TLS;或把页面部署在 http 下内网使用
2. 静态资源与 broker 不同源时,CORS 由 WebSocket 天然豁免,无需额外配置
3. 协议契约以 [vibetty `src/mqtt.rs`](../vibetty/src/mqtt.rs) 为唯一真相源

## 技术栈

Vite + Svelte 5 + TypeScript + Tailwind 4 · mqtt.js(WebSocket)·
@xterm/xterm(webgl/fit addon)· @use-gesture/vanilla + @tldraw/vec(画布手势)

MIT License。画布手势与自动排列移植自 sshx(MIT,© Eric Zhang)。
