# 如何生成 ArxivCat APK

## 已完成

✅ Android 项目已创建在 `android-app/` 目录

## 下一步（3 步搞定）

### 1. 修改服务器地址

编辑 `android-app/app/src/main/java/com/arxivcat/MainActivity.java`

找到第 13 行：
```java
private static final String WEB_URL = "http://192.168.1.100:5000";
```

改成你的电脑 IP（运行 `ipconfig` 查看 IPv4 地址）

### 2. 用 Android Studio 打开

1. 打开 Android Studio
2. File → Open
3. 选择 `D:\PersonalProjects\ArxivCat\android-app`
4. 等待 Gradle 同步（首次需要下载依赖，可能需要几分钟）

### 3. 构建 APK

点击：**Build → Build Bundle(s) / APK(s) → Build APK(s)**

生成的 APK 在：
```
android-app/app/build/outputs/apk/debug/app-debug.apk
```

### 4. 安装到手机

1. 将 APK 传到手机（微信、QQ、USB 都行）
2. 手机上点击安装
3. 允许"未知来源"安装
4. 完成！

## 使用要求

- ✅ 手机和电脑在同一 WiFi
- ✅ Web 服务器正在运行（`.\run-web.ps1`）
- ✅ Android 7.0 或更高版本

## 如果没有 Android Studio

可以用命令行构建：

```bash
cd android-app
.\gradlew assembleDebug
```

APK 会生成在同样的位置。

## 图标

当前使用默认图标。如需自定义：
1. 准备 512x512 PNG 图标
2. Android Studio → 右键 `res` → New → Image Asset
3. 选择图标文件
4. 重新构建

## 注意

这个 APK 是**调试版本**，适合自己用。如果要发布到应用商店，需要：
1. 生成签名密钥
2. 构建发布版本
3. 签名 APK

详见 `android-app/README.md`
