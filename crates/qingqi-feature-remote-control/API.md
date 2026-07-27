# 远程控制插件 API 文档

## 基本信息

- **服务地址**: `http://<IP>:3721`
- **数据格式**: JSON
- **字符编码**: UTF-8

## 统一响应格式

```json
{
  "ok": true,
  "data": { ... },
  "error": {
    "code": "ERROR_CODE",
    "message": "错误描述"
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| ok | boolean | 请求是否成功 |
| data | object | 成功时的响应数据 |
| error | object | 失败时的错误信息 |

---

## 认证接口

### POST `/api/v1/auth/pair` - 配对请求

使用 PIN 码配对设备，获取永久有效的 Token。

**请求体**:
```json
{
  "pin": "123456"
}
```

**响应**:
```json
{
  "ok": true,
  "data": {
    "token": "8ecebf4d-5a27-46d5-ae81-3ddee05b23e9",
    "expires_at": 9223372036854775807,
    "mac_address": "A8:5E:45:E7:DA:E4"
  }
}
```

### POST `/api/v1/auth/verify` - 验证令牌

**请求体**:
```json
{
  "token": "8ecebf4d-5a27-46d5-ae81-3ddee05b23e9"
}
```

**响应**:
```json
{
  "ok": true,
  "data": {}
}
```

### DELETE `/api/v1/auth` - 撤销令牌

**请求头**: `Authorization: Bearer <token>`

**响应**:
```json
{
  "ok": true,
  "data": {}
}
```

### GET `/api/v1/auth/devices` - 列出已配对设备

**响应**:
```json
{
  "ok": true,
  "data": {
    "devices": [
      {
        "device_name": "手机-123456",
        "created_at": 1719000000,
        "expires_at": 9223372036854775807,
        "permanent": true,
        "token": "8ecebf4d-..."
      }
    ]
  }
}
```

### DELETE `/api/v1/auth/devices/:device_name` - 撤销指定设备

**响应**:
```json
{
  "ok": true,
  "data": {}
}
```

---

## 系统控制接口

### GET `/api/v1/system/status` - 获取系统状态

**响应**:
```json
{
  "ok": true,
  "data": {
    "cpu_usage": 12.12,
    "memory_total": 17088012288,
    "memory_used": 7959609344,
    "memory_percent": 46.58,
    "uptime_seconds": 15260
  }
}
```

### GET `/api/v1/system/mac` - 获取 MAC 地址

**响应**:
```json
{
  "ok": true,
  "data": {
    "mac_address": "A8:5E:45:E7:DA:E4"
  }
}
```

### POST `/api/v1/system/shutdown` - 关机

**请求体**:
```json
{
  "force": false,
  "delay_secs": 0
}
```

### POST `/api/v1/system/sleep` - 睡眠/休眠

**请求体**:
```json
{
  "hibernate": false
}
```

### POST `/api/v1/system/restart` - 重启

**请求体**:
```json
{
  "force": false
}
```

### POST `/api/v1/system/logoff` - 注销

### POST `/api/v1/system/lock` - 锁屏

---

## 进程管理接口

### GET `/api/v1/processes` - 进程列表

**查询参数**:
| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| search | string | - | 搜索关键词 |
| page | number | 0 | 页码 |
| page_size | number | 50 | 每页数量 |

**响应**:
```json
{
  "ok": true,
  "data": {
    "processes": [
      {
        "pid": 11800,
        "name": "AggregatorHost.exe",
        "memory_bytes": 18882560,
        "cpu_usage": 0.0,
        "path": null
      }
    ],
    "total": 257,
    "page": 0,
    "page_size": 50
  }
}
```

### GET `/api/v1/processes/foreground` - 前台窗口

**响应**:
```json
{
  "ok": true,
  "data": {
    "pid": 9120,
    "title": "窗口标题",
    "path": "C:\\...\\app.exe",
    "process_name": "app.exe"
  }
}
```

### POST `/api/v1/processes/:pid/kill` - 结束进程

### POST `/api/v1/processes/:pid/suspend` - 挂起进程

### POST `/api/v1/processes/:pid/resume` - 恢复进程

---

## 应用管理接口

### POST `/api/v1/apps/launch` - 启动应用

**请求体**:
```json
{
  "path": "C:\\Windows\\System32\\notepad.exe",
  "args": []
}
```

### GET `/api/v1/apps/search` - 搜索已安装应用

**查询参数**: `query=notepad`

**响应**:
```json
{
  "ok": true,
  "data": {
    "apps": [
      {
        "name": "Notepad",
        "path": "C:\\Windows\\System32\\notepad.exe"
      }
    ]
  }
}
```

---

## 应用扫描接口

### GET `/api/v1/scanner/apps` - 获取所有扫描到的应用

**响应**:
```json
{
  "ok": true,
  "data": [
    {
      "id": "startmenu:C:\\...\\app.lnk",
      "name": "应用名称",
      "original_name": "应用名称",
      "exe_path": "C:\\...\\app.lnk",
      "icon_base64": null,
      "category": "Application",
      "source": "StartMenu"
    },
    {
      "id": "steam:730",
      "name": "Counter-Strike 2",
      "original_name": "Counter-Strike 2",
      "exe_path": "C:\\...\\cs2.exe",
      "icon_base64": null,
      "category": "Game",
      "source": "Steam"
    },
    {
      "id": "custom:abc123:C:\\Games\\game.exe",
      "name": "game",
      "original_name": "game",
      "exe_path": "C:\\Games\\game.exe",
      "icon_base64": null,
      "category": "Game",
      "source": "Custom"
    }
  ]
}
```

### POST `/api/v1/scanner/refresh` - 刷新扫描缓存

### POST `/api/v1/scanner/apps/rename` - 重命名应用

**请求体**:
```json
{
  "original_name": "原名称",
  "new_name": "新名称"
}
```

### POST `/api/v1/scanner/apps/:id/launch` - 启动应用

---

## Steam 接口

### GET `/api/v1/scanner/steam` - 获取 Steam 游戏列表

**响应**:
```json
{
  "ok": true,
  "data": {
    "steam_installed": true,
    "steam_path": "C:\\Program Files (x86)\\Steam",
    "libraries": [
      {
        "path": "C:\\Program Files (x86)\\Steam",
        "game_count": 15
      },
      {
        "path": "D:\\SteamLibrary",
        "game_count": 8
      }
    ],
    "games": [
      {
        "app_id": 730,
        "name": "Counter-Strike 2",
        "install_dir": "Counter-Strike Global Offensive",
        "install_path": "C:\\...\\Counter-Strike Global Offensive",
        "library_path": "C:\\Program Files (x86)\\Steam",
        "icon_path": "C:\\...\\appcache\\librarycache\\730_icon.jpg",
        "size_bytes": 36500000000,
        "last_played": 1719000000
      }
    ],
    "total": 23
  }
}
```

### POST `/api/v1/scanner/steam/refresh` - 刷新 Steam 扫描

**响应**:
```json
{
  "ok": true,
  "data": {
    "scanned": 23
  }
}
```

### POST `/api/v1/scanner/steam/:app_id/launch` - 启动 Steam 游戏

通过 `steam://rungameid/:app_id` 协议启动游戏。

**响应**:
```json
{
  "ok": true,
  "data": {}
}
```

---

## 自定义目录接口

### GET `/api/v1/scanner/custom-dirs` - 获取自定义目录列表

**响应**:
```json
{
  "ok": true,
  "data": {
    "dirs": [
      {
        "id": "custom_a1b2c3",
        "path": "F:\\MyGames",
        "name": "我的游戏库",
        "enabled": true,
        "max_depth": 3,
        "extensions": ["exe", "lnk"],
        "recursive": true,
        "created_at": 1719000000,
        "last_scanned": 1719100000
      }
    ]
  }
}
```

### POST `/api/v1/scanner/custom-dirs` - 添加自定义目录

**请求体**:
```json
{
  "path": "F:\\MyGames",
  "name": "我的游戏库",
  "enabled": true,
  "max_depth": 3,
  "extensions": ["exe", "lnk"],
  "recursive": true
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| path | string | 是 | 目录路径 |
| name | string | 否 | 显示名称（默认取目录名） |
| enabled | boolean | 否 | 是否启用（默认 true） |
| max_depth | number | 否 | 扫描深度，0=不限制（默认 0） |
| extensions | string[] | 否 | 文件扩展名（默认 ["exe","lnk"]） |
| recursive | boolean | 否 | 是否递归子目录（默认 true） |

**响应** (201):
```json
{
  "ok": true,
  "data": {
    "id": "custom_a1b2c3",
    "path": "F:\\MyGames",
    "name": "我的游戏库",
    "enabled": true,
    "max_depth": 3,
    "extensions": ["exe", "lnk"],
    "recursive": true,
    "created_at": 1719000000,
    "last_scanned": null
  }
}
```

### PUT `/api/v1/scanner/custom-dirs/:id` - 更新自定义目录

**请求体** (所有字段可选):
```json
{
  "name": "新名称",
  "enabled": false,
  "max_depth": 5
}
```

### DELETE `/api/v1/scanner/custom-dirs/:id` - 删除自定义目录

### POST `/api/v1/scanner/custom-dirs/validate` - 验证目录

**请求体**:
```json
{
  "path": "F:\\MyGames"
}
```

**响应**:
```json
{
  "ok": true,
  "data": {
    "valid": true,
    "exists": true,
    "is_dir": true,
    "readable": true,
    "exe_count": 12,
    "error": null
  }
}
```

---

## 任务管理接口

### GET `/api/v1/tasks` - 任务列表

**响应**: 同进程列表，最多 200 条

### POST `/api/v1/tasks/:pid/kill` - 结束任务

### POST `/api/v1/tasks/:pid/priority` - 设置进程优先级

**请求体**:
```json
{
  "priority": "high"
}
```

可选值：`low`, `normal`, `high`, `realtime`

### GET `/api/v1/tasks/stats` - 系统统计

**响应**: 同系统状态

---

## Web 界面接口

### GET `/` - 移动端 Web 界面

返回 HTML 页面。

### GET `/api/v1/web/apps` - 移动端应用列表

### GET `/api/v1/web/tasks` - 移动端任务列表

---

## WebSocket

### WS `/api/v1/events` - 实时事件流

**事件类型**:
| 类型 | 说明 |
|------|------|
| foreground_changed | 前台窗口变化 |
| process_started | 进程启动 |
| process_ended | 进程结束 |
| system_suspend | 系统挂起 |
| system_resume | 系统恢复 |

---

## 错误码

| 错误码 | 说明 |
|--------|------|
| INVALID_PIN | PIN 码无效或已过期 |
| INVALID_TOKEN | Token 无效或已过期 |
| MISSING_AUTH | 缺少认证头 |
| DEVICE_NOT_FOUND | 设备未找到 |
| APP_NOT_FOUND | 应用未找到 |
| GAME_NOT_FOUND | Steam 游戏未找到 |
| LAUNCH_FAILED | 启动失败 |
| ADD_FAILED | 添加目录失败 |
| UPDATE_FAILED | 更新目录失败 |
| DELETE_FAILED | 删除目录失败 |
| INVALID_INPUT | 输入参数无效 |
