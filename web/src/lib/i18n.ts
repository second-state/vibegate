/**
 * Minimal i18n: zh/en dictionaries + a `t` store of translate functions.
 * Language auto-detects from the browser, overrides persist to localStorage.
 * Placeholders: t("uploadFailed", { name, msg }) replaces "{name}" / "{msg}".
 */
import { derived, writable, type Writable } from "svelte/store";

export type Lang = "zh" | "en";
const KEY = "vibeview.lang";

function initial(): Lang {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved === "zh" || saved === "en") return saved;
  } catch {
    /* private mode */
  }
  return navigator.language?.toLowerCase().startsWith("zh") ? "zh" : "en";
}

export const lang: Writable<Lang> = writable(initial());
lang.subscribe((v) => {
  try {
    localStorage.setItem(KEY, v);
  } catch {
    /* ignore */
  }
});

const zh = {
  // ConnectBar
  menu: "菜单",
  files: "文件",
  sessions: "会话",
  stateIdle: "未连接",
  stateConnecting: "连接中…",
  stateConnected: "已连接",
  stateReconnecting: "重连中…",
  stateError: "错误",
  badgeTitle: "状态:{state} · 点击配置连接",
  gatewayUrl: "vibegate 网关地址",
  gatewayHint: "即 vibegate 服务端的 /ws 地址;https 页面请用 wss://。",
  mixedContent:
    "当前页面是 https,浏览器会拦截 ws:// 明文连接——请改用 wss://,或把本页部署在 http 下",
  connect: "连接",
  disconnect: "断开",
  appearance: "外观",
  terminalTheme: "终端主题",
  canvasBgLabel: "画布背景",
  terminalBgLabel: "终端背景",
  bgPlaceholder: "文件路径或图片 URL(留空 = 无)",
  bgOpacity: "背景透明度",
  keybarLabel: "辅助键",
  keybarFloat: "悬浮",
  keybarTop: "置顶",
  keybarHidden: "隐藏",
  language: "语言",
  appearanceNote: "相对路径走网关文件 API;图片窗口标题栏也有「画布」「终端」快捷设置按钮。",

  // TermWindow
  reconnect: "重新连接",
  closeWindow: "关闭窗口(不影响远端会话)",
  gridSize: "终端大小(列×行)",
  disconnectedHint: "连接已断开——点标题栏 ⟳ 重新连接",
  paneGoneHint: "远端面板已关闭,窗口即将移除…",

  // Keybar
  stickyModTitle: "粘滞 {mod}:下一次按键(按钮或软键盘输入)附带 {mod}",
  scrollUpTitle: "向上滚动服务端 scrollback",
  scrollDownTitle: "向下滚动服务端 scrollback",
  resetZoomTitle: "重置画布缩放到 1×(当前 {zoom}×)",

  // Canvas
  canvasEmpty: "画布是空的",
  canvasEmptyConnect: "点右上角「会话」打开一个终端窗口",
  canvasEmptyOffline: "先在顶部连接 vibegate 网关",
  canvasHint: "拖动空白平移 · 双指/滚轮缩放 · 双击窗口居中",

  // SessionDrawer
  wsPanes: "{ws} ws · {pane} pane",
  toggleCollapse: "展开/收起",
  newPaneHere: "在此 workspace 新建终端",
  onCanvas: "画布",
  closePane: "关闭此 pane",
  emptyWorkspace: "(空 workspace)",
  notConnected: "未连接网关——点顶部状态栏配置",
  others: "其他",
  newWorkspace: "新建 workspace",
  creating: "创建中…",
  newWorkspaceTitle: "新建 workspace(带一个终端)",

  // FileDrawer
  refresh: "刷新",
  goUp: "上一级",
  rootDir: "(根目录)",
  uploadLabel: "⇧ 上传",
  uploading: "上传中({n})",
  uploadTitle: "上传到当前目录",
  emptyDir: "空目录",
  loading: "加载中…",
  isFile: "这是一个文件",
  notDir: "不是目录",
  uploadFailed: "上传失败 {name}: {msg}",
  resumeIn: "上传中断,正在续传({n}/3)…",
  viewImage: "点击在画布中查看图片",
  viewMd: "点击在画布中查看 Markdown",
  download: "下载",
  retryUpload: "重传",
  resumeUpload: "从断点继续上传",
  dismiss: "忽略",
  netError: "网络错误",
  breakpointQueryError: "无法查询断点",

  // ImageWindow
  setCanvasBg: "设为画布背景",
  setTermBg: "设为终端背景",
  canvasBtn: "画布",
  termBtn: "终端",
  closeWin: "关闭窗口",
};

export type MsgKey = keyof typeof zh;

const en: Record<MsgKey, string> = {
  // ConnectBar
  menu: "Menu",
  files: "Files",
  sessions: "Sessions",
  stateIdle: "Offline",
  stateConnecting: "Connecting…",
  stateConnected: "Connected",
  stateReconnecting: "Reconnecting…",
  stateError: "Error",
  badgeTitle: "State: {state} · click to configure",
  gatewayUrl: "vibegate gateway URL",
  gatewayHint: "The /ws endpoint of your vibegate server; use wss:// on https pages.",
  mixedContent:
    "This page is https, the browser will block plaintext ws:// — switch to wss:// or serve this page over http.",
  connect: "Connect",
  disconnect: "Disconnect",
  appearance: "Appearance",
  terminalTheme: "Terminal theme",
  canvasBgLabel: "Canvas background",
  terminalBgLabel: "Terminal background",
  bgPlaceholder: "File path or image URL (empty = none)",
  bgOpacity: "Background opacity",
  keybarLabel: "Keybar",
  keybarFloat: "Float",
  keybarTop: "Top",
  keybarHidden: "Hidden",
  language: "Language",
  appearanceNote:
    "Relative paths go through the gateway file API; image windows also have canvas/terminal quick buttons.",

  // TermWindow
  reconnect: "Reconnect",
  closeWindow: "Close window (remote session keeps running)",
  gridSize: "Terminal size (cols×rows)",
  disconnectedHint: "Disconnected — click ⟳ in the title bar to reconnect",
  paneGoneHint: "Remote pane closed — removing this window…",

  // Keybar
  stickyModTitle: "Sticky {mod}: the next key (button or soft keyboard) carries {mod}",
  scrollUpTitle: "Scroll server scrollback up",
  scrollDownTitle: "Scroll server scrollback down",
  resetZoomTitle: "Reset canvas zoom to 1× (currently {zoom}×)",

  // Canvas
  canvasEmpty: "The canvas is empty",
  canvasEmptyConnect: 'Open a terminal window from "Sessions" (top right)',
  canvasEmptyOffline: "Connect to the vibegate gateway first (top bar)",
  canvasHint: "Drag to pan · pinch/scroll to zoom · double-click a window to center",

  // SessionDrawer
  wsPanes: "{ws} ws · {pane} pane",
  toggleCollapse: "Expand/collapse",
  newPaneHere: "New terminal in this workspace",
  onCanvas: "canvas",
  closePane: "Close this pane",
  emptyWorkspace: "(empty workspace)",
  notConnected: "Not connected — configure via the top bar",
  others: "Others",
  newWorkspace: "New workspace",
  creating: "Creating…",
  newWorkspaceTitle: "New workspace (with one terminal)",

  // FileDrawer
  refresh: "Refresh",
  goUp: "Up",
  rootDir: "(root)",
  uploadLabel: "⇧ Upload",
  uploading: "Uploading ({n})",
  uploadTitle: "Upload to the current directory",
  emptyDir: "Empty directory",
  loading: "Loading…",
  isFile: "This is a file",
  notDir: "Not a directory",
  uploadFailed: "Upload failed {name}: {msg}",
  resumeIn: "Upload interrupted, resuming ({n}/3)…",
  viewImage: "Click to view this image on the canvas",
  viewMd: "Click to view this markdown on the canvas",
  download: "Download",
  retryUpload: "Retry",
  resumeUpload: "Resume from breakpoint",
  dismiss: "Dismiss",
  netError: "Network error",
  breakpointQueryError: "Cannot query upload offset",

  // ImageWindow
  setCanvasBg: "Set as canvas background",
  setTermBg: "Set as terminal background",
  canvasBtn: "Canvas",
  termBtn: "Term",
  closeWin: "Close window",
};

function translate(l: Lang, k: MsgKey, p?: Record<string, unknown>): string {
  let s: string = (l === "en" ? en : zh)[k];
  if (p) {
    for (const [pk, pv] of Object.entries(p)) {
      s = s.replaceAll(`{${pk}}`, String(pv));
    }
  }
  return s;
}

/** Store of translate functions — `$t("key")` in components, `get(t)("key")`
 *  inside functions; both re-render / re-read on language switch. */
export const t = derived(lang, (l) => (k: MsgKey, p?: Record<string, unknown>) =>
  translate(l, k, p),
);
