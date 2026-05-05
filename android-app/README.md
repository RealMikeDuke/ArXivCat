# ArxivCat Android App

这是 ArxivCat 的 Android WebView 应用，将 Web 版本打包成 APK。

## 使用步骤

### 1. 修改服务器地址

编辑 `app/src/main/java/com/arxivcat/MainActivity.java`：

```java
// 修改这里为你的服务器地址
private static final String WEB_URL = "http://192.168.1.100:5000";
```

改成你的电脑 IP 地址（运行 `ipconfig` 查看）。

### 2. 用 Android Studio 打开项目

1. 打开 Android Studio
2. File → Open
3. 选择 `android-app` 文件夹
4. 等待 Gradle 同步完成

### 3. 构建 APK

**方式 1: 调试版本（最快）**
```
Build → Build Bundle(s) / APK(s) → Build APK(s)
```

生成的 APK 在：
```
app/build/outputs/apk/debug/app-debug.apk
```

**方式 2: 发布版本（需要签名）**
```
Build → Generate Signed Bundle / APK
```

### 4. 安装到手机

1. 将 APK 传到手机
2. 允许"未知来源"安装
3. 安装并打开

## 注意事项

- 手机和电脑需要在同一 WiFi
- 确保 Web 服务器正在运行（`.\run-web.ps1`）
- 如果要在外网使用，需要部署 Web 应用到云端

## 图标

当前使用默认图标。如需自定义：
1. 准备图标（512x512 PNG）
2. 使用 Android Studio 的 Image Asset 工具
3. 右键 `res` → New → Image Asset

## 技术说明

- 最低 Android 版本: 7.0 (API 24)
- 目标 Android 版本: 14 (API 34)
- 使用 WebView 加载 Web 应用
- 支持 JavaScript 和本地存储
