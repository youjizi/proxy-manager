use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 代理配置组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
}

/// 软件与代理配置的映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareProxyMapping {
    pub software_name: String,
    pub profile_name: String,
}

/// 自定义软件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSoftware {
    pub name: String,
    pub config_type: String, // "json", "ini", "env"
    pub config_path: String,
}

/// 关闭行为偏好
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosePreference {
    pub remember: bool,           // 是否记住选择
    pub action: String,           // "minimize" 或 "exit"
}

impl Default for ClosePreference {
    fn default() -> Self {
        ClosePreference {
            remember: false,
            action: "minimize".to_string(),
        }
    }
}

/// 用户配置（包含所有代理配置组、软件映射和自定义软件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub profiles: Vec<ProxyProfile>,
    pub mappings: Vec<SoftwareProxyMapping>,
    #[serde(default)]
    pub custom_software: Vec<CustomSoftware>,
    #[serde(default)]
    pub close_preference: ClosePreference,
}

impl Default for UserConfig {
    fn default() -> Self {
        // 默认配置：预设一些常用的代理配置组
        UserConfig {
            profiles: vec![
                ProxyProfile {
                    name: "Clash".to_string(),
                    host: "127.0.0.1".to_string(),
                    port: 7890,
                },
                ProxyProfile {
                    name: "V2Ray".to_string(),
                    host: "127.0.0.1".to_string(),
                    port: 10808,
                },
                ProxyProfile {
                    name: "Veee".to_string(),
                    host: "127.0.0.1".to_string(),
                    port: 15236,
                },
            ],
            mappings: vec![],
            custom_software: vec![],
            close_preference: ClosePreference::default(),
        }
    }
}

/// 获取配置文件路径
fn get_config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".proxy-manager").join("user_config.json")
}

/// 加载用户配置
pub fn load_user_config() -> UserConfig {
    let config_path = get_config_path();

    if config_path.exists() {
        match fs::read_to_string(&config_path) {
            Ok(content) => {
                match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("解析配置文件失败: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("读取配置文件失败: {}", e);
            }
        }
    }

    // 返回默认配置
    UserConfig::default()
}

/// 保存用户配置
pub fn save_user_config(config: &UserConfig) -> Result<(), String> {
    let config_path = get_config_path();

    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
    }

    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;

    fs::write(&config_path, content)
        .map_err(|e| format!("写入配置文件失败: {}", e))?;

    Ok(())
}

/// 添加代理配置组
pub fn add_profile(profile: ProxyProfile) -> Result<UserConfig, String> {
    let mut config = load_user_config();

    // 检查是否已存在同名配置
    if config.profiles.iter().any(|p| p.name == profile.name) {
        return Err(format!("配置组 '{}' 已存在", profile.name));
    }

    config.profiles.push(profile);
    save_user_config(&config)?;

    Ok(config)
}

/// 删除代理配置组
pub fn delete_profile(profile_name: &str) -> Result<UserConfig, String> {
    let mut config = load_user_config();

    let original_len = config.profiles.len();
    config.profiles.retain(|p| p.name != profile_name);

    if config.profiles.len() == original_len {
        return Err(format!("配置组 '{}' 不存在", profile_name));
    }

    // 同时删除使用该配置组的映射
    config.mappings.retain(|m| m.profile_name != profile_name);

    save_user_config(&config)?;

    Ok(config)
}

/// 更新软件的代理配置映射
pub fn update_software_mapping(software_name: &str, profile_name: &str) -> Result<UserConfig, String> {
    let mut config = load_user_config();

    // 验证配置组是否存在
    if !config.profiles.iter().any(|p| p.name == profile_name) {
        return Err(format!("配置组 '{}' 不存在", profile_name));
    }

    // 查找并更新现有映射，或添加新映射
    if let Some(mapping) = config.mappings.iter_mut().find(|m| m.software_name == software_name) {
        mapping.profile_name = profile_name.to_string();
    } else {
        config.mappings.push(SoftwareProxyMapping {
            software_name: software_name.to_string(),
            profile_name: profile_name.to_string(),
        });
    }

    save_user_config(&config)?;

    Ok(config)
}

/// 更新代理配置组
pub fn update_profile(old_name: &str, profile: ProxyProfile) -> Result<UserConfig, String> {
    let mut config = load_user_config();

    // 查找并更新配置组
    if let Some(existing) = config.profiles.iter_mut().find(|p| p.name == old_name) {
        // 如果名称改变了，需要更新所有映射
        if old_name != profile.name {
            for mapping in &mut config.mappings {
                if mapping.profile_name == old_name {
                    mapping.profile_name = profile.name.clone();
                }
            }
        }

        existing.name = profile.name;
        existing.host = profile.host;
        existing.port = profile.port;
    } else {
        return Err(format!("配置组 '{}' 不存在", old_name));
    }

    save_user_config(&config)?;

    Ok(config)
}

/// 添加自定义软件
pub fn add_custom_software(software: CustomSoftware) -> Result<UserConfig, String> {
    let mut config = load_user_config();

    // 检查是否已存在同名软件
    if config.custom_software.iter().any(|s| s.name == software.name) {
        return Err(format!("软件 '{}' 已存在", software.name));
    }

    config.custom_software.push(software);
    save_user_config(&config)?;

    Ok(config)
}

/// 删除自定义软件
pub fn delete_custom_software(software_name: &str) -> Result<UserConfig, String> {
    let mut config = load_user_config();

    let original_len = config.custom_software.len();
    config.custom_software.retain(|s| s.name != software_name);

    if config.custom_software.len() == original_len {
        return Err(format!("软件 '{}' 不存在", software_name));
    }

    // 同时删除该软件的映射
    config.mappings.retain(|m| m.software_name != software_name);

    save_user_config(&config)?;

    Ok(config)
}
