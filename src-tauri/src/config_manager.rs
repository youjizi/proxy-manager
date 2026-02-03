use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SoftwareConfig {
    pub name: String,
    pub config_type: String, // "json", "ini", "xml"
    pub enabled: bool,
    pub installed: bool,
    pub config_path: Option<String>,
    #[serde(default)]
    pub is_custom: bool, // 是否为自定义软件
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProxySettings {
    pub http_proxy: String,
    pub https_proxy: String,
    pub no_proxy: String,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            http_proxy: "http://127.0.0.1:7890".to_string(),
            https_proxy: "http://127.0.0.1:7890".to_string(),
            no_proxy: "localhost,127.0.0.1,::1".to_string(),
        }
    }
}

/// 获取备份目录路径
/// 位置: %LOCALAPPDATA%\proxy-manager\backups\
fn get_backup_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("proxy-manager").join("backups"))
}

/// 获取软件配置的初始备份路径（首次备份，永不覆盖）
fn get_original_backup_path(software_name: &str) -> Option<PathBuf> {
    get_backup_dir().map(|dir| dir.join(format!("{}.original.backup", software_name)))
}

/// 获取软件配置的当前备份路径（每次切换 VPN 时更新）
fn get_current_backup_path(software_name: &str) -> Option<PathBuf> {
    get_backup_dir().map(|dir| dir.join(format!("{}.current.backup", software_name)))
}

/// 备份软件的原有配置
/// - original: 首次备份，永不覆盖（用于重置到初始状态）
/// - current: 每次开启代理前保存当前配置（用于切换 VPN）
fn backup_config(software_name: &str, config_path: &PathBuf) -> Result<(), String> {
    if !config_path.exists() {
        return Ok(()); // 配置文件不存在，无需备份
    }

    let backup_dir = get_backup_dir().ok_or("无法获取备份目录")?;
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

    let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;

    // 1. 初始备份：只在不存在时创建，永不覆盖
    let original_path = get_original_backup_path(software_name).ok_or("无法获取初始备份路径")?;
    if !original_path.exists() {
        fs::write(&original_path, &content).map_err(|e| e.to_string())?;
    }

    // 2. 当前备份：每次都更新，保存切换前的配置
    let current_path = get_current_backup_path(software_name).ok_or("无法获取当前备份路径")?;
    fs::write(&current_path, &content).map_err(|e| e.to_string())?;

    Ok(())
}

/// 从备份还原软件配置
/// reset_to_original: true = 重置到初始状态, false = 还原到上次配置
fn restore_config(software_name: &str, config_path: &PathBuf, reset_to_original: bool) -> Result<bool, String> {
    let backup_path = if reset_to_original {
        get_original_backup_path(software_name)
    } else {
        get_current_backup_path(software_name)
    }.ok_or("无法获取备份路径")?;

    if !backup_path.exists() {
        return Ok(false); // 没有备份，返回 false
    }

    let content = fs::read_to_string(&backup_path).map_err(|e| e.to_string())?;
    fs::write(config_path, content).map_err(|e| e.to_string())?;

    // 注意：不删除备份文件，保持持久化

    Ok(true)
}

/// 获取支持的软件列表并检测安装状态
pub fn get_software_list() -> Vec<SoftwareConfig> {
    let mut software_list = vec![
        SoftwareConfig {
            name: "Git".to_string(),
            config_type: "ini".to_string(),
            enabled: true,
            installed: false,
            config_path: None,
            is_custom: false,
        },
        SoftwareConfig {
            name: "npm".to_string(),
            config_type: "ini".to_string(),
            enabled: true,
            installed: false,
            config_path: None,
            is_custom: false,
        },
        SoftwareConfig {
            name: "Cursor".to_string(),
            config_type: "json".to_string(),
            enabled: true,
            installed: false,
            config_path: None,
            is_custom: false,
        },
        SoftwareConfig {
            name: "VSCode".to_string(),
            config_type: "json".to_string(),
            enabled: true,
            installed: false,
            config_path: None,
            is_custom: false,
        },
        SoftwareConfig {
            name: "IDEA".to_string(),
            config_type: "xml".to_string(),
            enabled: true,
            installed: false,
            config_path: None,
            is_custom: false,
        },
        #[cfg(target_os = "windows")]
        SoftwareConfig {
            name: "Windows Terminal".to_string(),
            config_type: "env".to_string(),
            enabled: true,
            installed: true, // 环境变量总是可用的
            config_path: Some("HKEY_CURRENT_USER\\Environment".to_string()),
            is_custom: false,
        },
    ];

    // 检测每个软件的安装状态
    for software in &mut software_list {
        if let Some(path) = get_config_path(&software.name) {
            software.config_path = Some(path.to_string_lossy().to_string());
            // 检查配置文件或其父目录是否存在
            let path_buf = PathBuf::from(&path);
            software.installed = path_buf.exists() || path_buf.parent().map(|p| p.exists()).unwrap_or(false);
        }
    }

    software_list
}

/// 获取软件配置文件路径
fn get_config_path(software_name: &str) -> Option<PathBuf> {
    let home_dir = dirs::home_dir()?;

    match software_name {
        "Git" => Some(home_dir.join(".gitconfig")),
        "npm" => Some(home_dir.join(".npmrc")),
        "Cursor" => {
            #[cfg(target_os = "windows")]
            {
                dirs::config_dir().map(|p| p.join("Cursor").join("User").join("settings.json"))
            }
            #[cfg(target_os = "macos")]
            {
                Some(home_dir.join("Library/Application Support/Cursor/User/settings.json"))
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            {
                dirs::config_dir().map(|p| p.join("Cursor").join("User").join("settings.json"))
            }
        }
        "VSCode" => {
            #[cfg(target_os = "windows")]
            {
                dirs::config_dir().map(|p| p.join("Code").join("User").join("settings.json"))
            }
            #[cfg(target_os = "macos")]
            {
                Some(home_dir.join("Library/Application Support/Code/User/settings.json"))
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            {
                dirs::config_dir().map(|p| p.join("Code").join("User").join("settings.json"))
            }
        }
        "IDEA" => {
            #[cfg(target_os = "windows")]
            {
                // 查找最新版本的 IDEA 配置目录
                if let Some(appdata) = dirs::config_dir() {
                    let jetbrains_dir = appdata.join("JetBrains");
                    if jetbrains_dir.exists() {
                        if let Ok(entries) = fs::read_dir(&jetbrains_dir) {
                            let mut idea_dirs: Vec<_> = entries
                                .filter_map(|e| e.ok())
                                .filter(|e| {
                                    e.file_name()
                                        .to_string_lossy()
                                        .starts_with("IntelliJIdea")
                                })
                                .collect();
                            idea_dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
                            if let Some(latest) = idea_dirs.first() {
                                return Some(
                                    latest.path().join("options").join("proxy.settings.xml"),
                                );
                            }
                        }
                    }
                }
                None
            }
            #[cfg(target_os = "macos")]
            {
                let app_support = home_dir.join("Library/Application Support/JetBrains");
                if app_support.exists() {
                    if let Ok(entries) = fs::read_dir(&app_support) {
                        let mut idea_dirs: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                e.file_name()
                                    .to_string_lossy()
                                    .starts_with("IntelliJIdea")
                            })
                            .collect();
                        idea_dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
                        if let Some(latest) = idea_dirs.first() {
                            return Some(latest.path().join("options").join("proxy.settings.xml"));
                        }
                    }
                }
                None
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            {
                None
            }
        }
        "Windows Terminal" => {
            // 环境变量不需要文件路径，返回 None
            None
        }
        _ => None,
    }
}

/// 开启代理
pub fn enable_proxy(
    software_list: &[String],
    proxy_settings: &ProxySettings,
) -> Result<Vec<String>, String> {
    let mut results = Vec::new();

    for software_name in software_list {
        match enable_proxy_for_software(software_name, proxy_settings) {
            Ok(msg) => results.push(format!("✓ {}: {}", software_name, msg)),
            Err(e) => results.push(format!("✗ {}: {}", software_name, e)),
        }
    }

    Ok(results)
}

/// 关闭代理
pub fn disable_proxy(software_list: &[String]) -> Result<Vec<String>, String> {
    let mut results = Vec::new();

    for software_name in software_list {
        match disable_proxy_for_software(software_name) {
            Ok(msg) => results.push(format!("✓ {}: {}", software_name, msg)),
            Err(e) => results.push(format!("✗ {}: {}", software_name, e)),
        }
    }

    Ok(results)
}

/// 重置到初始状态（还原首次备份的配置）
pub fn reset_to_original(software_list: &[String]) -> Result<Vec<String>, String> {
    let mut results = Vec::new();

    for software_name in software_list {
        match reset_software_to_original(software_name) {
            Ok(msg) => results.push(format!("✓ {}: {}", software_name, msg)),
            Err(e) => results.push(format!("✗ {}: {}", software_name, e)),
        }
    }

    Ok(results)
}

/// 重置单个软件到初始状态
fn reset_software_to_original(software_name: &str) -> Result<String, String> {
    // Windows Terminal 特殊处理
    if software_name == "Windows Terminal" {
        #[cfg(target_os = "windows")]
        {
            return reset_windows_env_to_original();
        }
        #[cfg(not(target_os = "windows"))]
        {
            return Err("Windows Terminal 仅支持 Windows 系统".to_string());
        }
    }

    let config_path =
        get_config_path(software_name).ok_or_else(|| "无法获取配置路径".to_string())?;

    // 从初始备份还原
    if restore_config(software_name, &config_path, true)? {
        return Ok("已重置到初始状态".to_string());
    }

    Ok("没有初始备份，无需重置".to_string())
}

/// 为单个软件开启代理
fn enable_proxy_for_software(
    software_name: &str,
    proxy_settings: &ProxySettings,
) -> Result<String, String> {
    // Windows Terminal 特殊处理（环境变量）
    if software_name == "Windows Terminal" {
        #[cfg(target_os = "windows")]
        {
            return enable_windows_env_proxy(proxy_settings);
        }
        #[cfg(not(target_os = "windows"))]
        {
            return Err("Windows Terminal 仅支持 Windows 系统".to_string());
        }
    }

    let config_path =
        get_config_path(software_name).ok_or_else(|| "无法获取配置路径".to_string())?;

    // 先备份原有配置
    backup_config(software_name, &config_path)?;

    match software_name {
        "Git" => enable_git_proxy(&config_path, proxy_settings),
        "npm" => enable_npm_proxy(&config_path, proxy_settings),
        "Cursor" | "VSCode" => enable_vscode_proxy(&config_path, proxy_settings),
        "IDEA" => enable_idea_proxy(&config_path, proxy_settings),
        _ => Err("不支持的软件".to_string()),
    }
}

/// 为单个软件关闭代理
fn disable_proxy_for_software(software_name: &str) -> Result<String, String> {
    // Windows Terminal 特殊处理（环境变量）
    if software_name == "Windows Terminal" {
        #[cfg(target_os = "windows")]
        {
            return disable_windows_env_proxy();
        }
        #[cfg(not(target_os = "windows"))]
        {
            return Err("Windows Terminal 仅支持 Windows 系统".to_string());
        }
    }

    let config_path =
        get_config_path(software_name).ok_or_else(|| "无法获取配置路径".to_string())?;

    // 尝试从当前备份还原（上次的配置）
    if restore_config(software_name, &config_path, false)? {
        return Ok("已还原上次配置".to_string());
    }

    // 没有备份，使用原来的方式关闭代理
    match software_name {
        "Git" => disable_git_proxy(&config_path),
        "npm" => disable_npm_proxy(&config_path),
        "Cursor" | "VSCode" => disable_vscode_proxy(&config_path),
        "IDEA" => disable_idea_proxy(&config_path),
        _ => Err("不支持的软件".to_string()),
    }
}

// ============ Git 代理配置 ============

fn enable_git_proxy(config_path: &PathBuf, proxy_settings: &ProxySettings) -> Result<String, String> {
    let mut content = if config_path.exists() {
        fs::read_to_string(config_path).unwrap_or_default()
    } else {
        String::new()
    };

    // 移除现有的代理配置
    content = remove_git_proxy_section(&content);

    // 添加新的代理配置
    let proxy_section = format!(
        "\n[http]\n\tproxy = {}\n[https]\n\tproxy = {}\n",
        proxy_settings.http_proxy, proxy_settings.https_proxy
    );
    content.push_str(&proxy_section);

    fs::write(config_path, content).map_err(|e| e.to_string())?;
    Ok("代理已开启".to_string())
}

fn disable_git_proxy(config_path: &PathBuf) -> Result<String, String> {
    if !config_path.exists() {
        return Ok("配置文件不存在，无需操作".to_string());
    }

    let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let new_content = remove_git_proxy_section(&content);
    fs::write(config_path, new_content).map_err(|e| e.to_string())?;
    Ok("代理已关闭".to_string())
}

fn remove_git_proxy_section(content: &str) -> String {
    let mut result = String::new();
    let mut skip_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section_name = trimmed[1..trimmed.len() - 1].to_lowercase();
            skip_section = section_name == "http" || section_name == "https";
            if !skip_section {
                result.push_str(line);
                result.push('\n');
            }
        } else if !skip_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    result.trim_end().to_string()
}

// ============ npm 代理配置 ============

fn enable_npm_proxy(config_path: &PathBuf, proxy_settings: &ProxySettings) -> Result<String, String> {
    let mut content = if config_path.exists() {
        fs::read_to_string(config_path).unwrap_or_default()
    } else {
        String::new()
    };

    // 移除现有的代理配置
    content = remove_npm_proxy_lines(&content);

    // 添加新的代理配置
    content.push_str(&format!("\nproxy={}\n", proxy_settings.http_proxy));
    content.push_str(&format!("https-proxy={}\n", proxy_settings.https_proxy));

    fs::write(config_path, content.trim()).map_err(|e| e.to_string())?;
    Ok("代理已开启".to_string())
}

fn disable_npm_proxy(config_path: &PathBuf) -> Result<String, String> {
    if !config_path.exists() {
        return Ok("配置文件不存在，无需操作".to_string());
    }

    let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let new_content = remove_npm_proxy_lines(&content);
    fs::write(config_path, new_content.trim()).map_err(|e| e.to_string())?;
    Ok("代理已关闭".to_string())
}

fn remove_npm_proxy_lines(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim().to_lowercase();
            !trimmed.starts_with("proxy=") && !trimmed.starts_with("https-proxy=")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ============ VSCode/Cursor 代理配置 ============

fn enable_vscode_proxy(
    config_path: &PathBuf,
    proxy_settings: &ProxySettings,
) -> Result<String, String> {
    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut json: serde_json::Value = if config_path.exists() {
        let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // 设置代理
    json["http.proxy"] = serde_json::Value::String(proxy_settings.http_proxy.clone());

    let content = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    fs::write(config_path, content).map_err(|e| e.to_string())?;
    Ok("代理已开启".to_string())
}

fn disable_vscode_proxy(config_path: &PathBuf) -> Result<String, String> {
    if !config_path.exists() {
        return Ok("配置文件不存在，无需操作".to_string());
    }

    let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let mut json: serde_json::Value =
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

    // 移除代理设置
    if let Some(obj) = json.as_object_mut() {
        obj.remove("http.proxy");
    }

    let content = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    fs::write(config_path, content).map_err(|e| e.to_string())?;
    Ok("代理已关闭".to_string())
}

// ============ IDEA 代理配置 ============

fn enable_idea_proxy(
    config_path: &PathBuf,
    proxy_settings: &ProxySettings,
) -> Result<String, String> {
    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // 解析代理地址
    let proxy_url = &proxy_settings.http_proxy;
    let (host, port) = parse_proxy_url(proxy_url)?;

    let xml_content = format!(
        r#"<application>
  <component name="HttpConfigurable">
    <option name="USE_HTTP_PROXY" value="true"/>
    <option name="PROXY_HOST" value="{}"/>
    <option name="PROXY_PORT" value="{}"/>
  </component>
</application>"#,
        host, port
    );

    fs::write(config_path, xml_content).map_err(|e| e.to_string())?;
    Ok("代理已开启（需重启 IDEA）".to_string())
}

fn disable_idea_proxy(config_path: &PathBuf) -> Result<String, String> {
    if config_path.exists() {
        fs::remove_file(config_path).map_err(|e| e.to_string())?;
    }
    Ok("代理已关闭（需重启 IDEA）".to_string())
}

/// 解析代理 URL，提取 host 和 port
fn parse_proxy_url(url: &str) -> Result<(String, u16), String> {
    let url = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let parts: Vec<&str> = url.split(':').collect();

    if parts.len() != 2 {
        return Err("无效的代理地址格式".to_string());
    }

    let host = parts[0].to_string();
    let port = parts[1]
        .parse::<u16>()
        .map_err(|_| "无效的端口号".to_string())?;

    Ok((host, port))
}

// ============ Windows 环境变量代理配置 ============

#[cfg(target_os = "windows")]
fn get_env_original_backup_path() -> Option<PathBuf> {
    get_backup_dir().map(|dir| dir.join("windows_env.original.backup.json"))
}

#[cfg(target_os = "windows")]
fn get_env_current_backup_path() -> Option<PathBuf> {
    get_backup_dir().map(|dir| dir.join("windows_env.current.backup.json"))
}

#[cfg(target_os = "windows")]
fn enable_windows_env_proxy(proxy_settings: &ProxySettings) -> Result<String, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| format!("无法打开注册表: {}", e))?;

    // 备份现有的环境变量
    let backup_dir = get_backup_dir().ok_or("无法获取备份目录")?;
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

    let mut backup_data = serde_json::Map::new();

    // 读取并备份现有值
    for var_name in &["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"] {
        if let Ok(value) = env.get_value::<String, _>(*var_name) {
            backup_data.insert(var_name.to_string(), serde_json::Value::String(value));
        }
    }

    let backup_json = serde_json::to_string_pretty(&backup_data).map_err(|e| e.to_string())?;

    // 1. 初始备份：只在不存在时创建
    let original_path = get_env_original_backup_path().ok_or("无法获取初始备份路径")?;
    if !original_path.exists() {
        fs::write(&original_path, &backup_json).map_err(|e| e.to_string())?;
    }

    // 2. 当前备份：每次都更新
    let current_path = get_env_current_backup_path().ok_or("无法获取当前备份路径")?;
    fs::write(&current_path, &backup_json).map_err(|e| e.to_string())?;

    // 设置新的环境变量
    env.set_value("HTTP_PROXY", &proxy_settings.http_proxy)
        .map_err(|e| format!("设置 HTTP_PROXY 失败: {}", e))?;
    env.set_value("HTTPS_PROXY", &proxy_settings.https_proxy)
        .map_err(|e| format!("设置 HTTPS_PROXY 失败: {}", e))?;
    env.set_value("NO_PROXY", &proxy_settings.no_proxy)
        .map_err(|e| format!("设置 NO_PROXY 失败: {}", e))?;

    // 广播环境变量更改消息
    broadcast_env_change();

    Ok("环境变量已设置（新终端窗口生效）".to_string())
}

#[cfg(target_os = "windows")]
fn restore_env_from_backup(backup_path: &PathBuf) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| format!("无法打开注册表: {}", e))?;

    // 先删除所有代理相关的环境变量
    for var_name in &["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"] {
        let _ = env.delete_value(*var_name);
    }

    if backup_path.exists() {
        let backup_content = fs::read_to_string(backup_path).map_err(|e| e.to_string())?;
        let backup_data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&backup_content).unwrap_or_default();

        // 还原备份的值
        for (key, value) in backup_data {
            if let Some(val_str) = value.as_str() {
                let _ = env.set_value(&key, &val_str.to_string());
            }
        }
    }

    broadcast_env_change();
    Ok(())
}

#[cfg(target_os = "windows")]
fn disable_windows_env_proxy() -> Result<String, String> {
    let current_path = get_env_current_backup_path().ok_or("无法获取当前备份路径")?;
    restore_env_from_backup(&current_path)?;
    Ok("已还原上次环境变量（新终端窗口生效）".to_string())
}

#[cfg(target_os = "windows")]
fn reset_windows_env_to_original() -> Result<String, String> {
    let original_path = get_env_original_backup_path().ok_or("无法获取初始备份路径")?;
    if !original_path.exists() {
        return Ok("没有初始备份，无需重置".to_string());
    }
    restore_env_from_backup(&original_path)?;
    Ok("已重置到初始环境变量（新终端窗口生效）".to_string())
}

/// 广播环境变量更改消息，通知系统环境变量已更新
#[cfg(target_os = "windows")]
fn broadcast_env_change() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    // WM_SETTINGCHANGE = 0x001A
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const HWND_BROADCAST: isize = 0xFFFF;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: *const u16,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }

    let env_str: Vec<u16> = OsStr::new("Environment")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env_str.as_ptr(),
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }
}
