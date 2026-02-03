mod config_manager;
mod port_detector;
mod profile_manager;

use config_manager::{ProxySettings, SoftwareConfig};
use port_detector::{DetectionResult, VpnConfig};
use profile_manager::{
    ClosePreference, CustomSoftware, ProxyProfile, SoftwareProxyMapping, UserConfig,
};
use std::collections::HashMap;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

// ============ Tauri 命令 ============

/// 获取预设的 VPN 列表
#[tauri::command]
fn get_vpn_list() -> Vec<VpnConfig> {
    port_detector::get_vpn_configs()
}

/// 根据 VPN 名称检测端口
#[tauri::command]
fn detect_port(vpn_name: String) -> DetectionResult {
    port_detector::detect_port_by_vpn_name(&vpn_name)
}

/// 获取支持的软件列表（包含预设和自定义）
#[tauri::command]
fn get_software_list() -> Vec<SoftwareConfig> {
    let mut list = config_manager::get_software_list();

    // 添加自定义软件
    let user_config = profile_manager::load_user_config();
    for custom in user_config.custom_software {
        list.push(SoftwareConfig {
            name: custom.name,
            config_type: custom.config_type,
            enabled: true,
            installed: true, // 自定义软件默认标记为已安装
            config_path: Some(custom.config_path),
            is_custom: true,
        });
    }

    list
}

/// 获取用户配置（代理配置组 + 软件映射）
#[tauri::command]
fn get_user_config() -> UserConfig {
    profile_manager::load_user_config()
}

/// 保存用户配置
#[tauri::command]
fn save_user_config(config: UserConfig) -> Result<(), String> {
    profile_manager::save_user_config(&config)
}

/// 添加代理配置组
#[tauri::command]
fn add_proxy_profile(profile: ProxyProfile) -> Result<UserConfig, String> {
    profile_manager::add_profile(profile)
}

/// 删除代理配置组
#[tauri::command]
fn delete_proxy_profile(profile_name: String) -> Result<UserConfig, String> {
    profile_manager::delete_profile(&profile_name)
}

/// 更新软件的代理配置映射
#[tauri::command]
fn update_software_mapping(
    software_name: String,
    profile_name: String,
) -> Result<UserConfig, String> {
    profile_manager::update_software_mapping(&software_name, &profile_name)
}

/// 开启代理（使用配置组）
#[tauri::command]
fn enable_proxy_with_profiles(
    software_mappings: Vec<SoftwareProxyMapping>,
) -> Result<Vec<String>, String> {
    let config = profile_manager::load_user_config();
    let profiles: HashMap<String, ProxyProfile> = config
        .profiles
        .into_iter()
        .map(|p| (p.name.clone(), p))
        .collect();

    let mut results = Vec::new();

    for mapping in software_mappings {
        if let Some(profile) = profiles.get(&mapping.profile_name) {
            let proxy_settings = ProxySettings {
                http_proxy: format!("http://{}:{}", profile.host, profile.port),
                https_proxy: format!("http://{}:{}", profile.host, profile.port),
                no_proxy: "localhost,127.0.0.1,::1".to_string(),
            };

            match config_manager::enable_proxy(
                std::slice::from_ref(&mapping.software_name),
                &proxy_settings,
            ) {
                Ok(mut msgs) => results.append(&mut msgs),
                Err(e) => results.push(format!("✗ {}: {}", mapping.software_name, e)),
            }
        } else {
            results.push(format!(
                "✗ {}: 未找到配置 '{}'",
                mapping.software_name, mapping.profile_name
            ));
        }
    }

    Ok(results)
}

/// 开启代理（旧接口，保持兼容）
#[tauri::command]
fn enable_proxy(
    software_list: Vec<String>,
    proxy_host: String,
    proxy_port: u16,
) -> Result<Vec<String>, String> {
    let proxy_settings = ProxySettings {
        http_proxy: format!("http://{}:{}", proxy_host, proxy_port),
        https_proxy: format!("http://{}:{}", proxy_host, proxy_port),
        no_proxy: "localhost,127.0.0.1,::1".to_string(),
    };
    config_manager::enable_proxy(&software_list, &proxy_settings)
}

/// 关闭代理
#[tauri::command]
fn disable_proxy(software_list: Vec<String>) -> Result<Vec<String>, String> {
    config_manager::disable_proxy(&software_list)
}

/// 重置到初始状态（还原首次备份的配置）
#[tauri::command]
fn reset_proxy(software_list: Vec<String>) -> Result<Vec<String>, String> {
    config_manager::reset_to_original(&software_list)
}

/// 添加自定义软件
#[tauri::command]
fn add_custom_software(software: CustomSoftware) -> Result<UserConfig, String> {
    profile_manager::add_custom_software(software)
}

/// 删除自定义软件
#[tauri::command]
fn delete_custom_software(software_name: String) -> Result<UserConfig, String> {
    profile_manager::delete_custom_software(&software_name)
}

/// 退出应用程序
#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

/// 隐藏窗口到托盘
#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

/// 获取关闭行为偏好
#[tauri::command]
fn get_close_preference() -> ClosePreference {
    let config = profile_manager::load_user_config();
    config.close_preference
}

/// 保存关闭行为偏好
#[tauri::command]
fn save_close_preference(preference: ClosePreference) -> Result<(), String> {
    let mut config = profile_manager::load_user_config();
    config.close_preference = preference;
    profile_manager::save_user_config(&config)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Proxy Manager")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 阻止默认关闭行为，发送事件到前端让前端处理
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_vpn_list,
            detect_port,
            get_software_list,
            get_user_config,
            save_user_config,
            add_proxy_profile,
            delete_proxy_profile,
            update_software_mapping,
            enable_proxy,
            enable_proxy_with_profiles,
            disable_proxy,
            reset_proxy,
            add_custom_software,
            delete_custom_software,
            exit_app,
            hide_window,
            get_close_preference,
            save_close_preference
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
