use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VpnConfig {
    pub name: String,
    pub process_names: Vec<String>,
    pub default_http_port: u16,
    pub default_socks_port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetectedPort {
    pub port: u16,
    pub port_type: String, // "http" or "socks"
    pub process_name: String,
    pub pid: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetectionResult {
    pub success: bool,
    pub message: String,
    pub ports: Vec<DetectedPort>,
}

// 预设的 VPN 配置
pub fn get_vpn_configs() -> Vec<VpnConfig> {
    vec![
        VpnConfig {
            name: "Clash".to_string(),
            process_names: vec![
                "clash".to_string(),
                "clash-windows".to_string(),
                "Clash for Windows".to_string(),
                "cfw".to_string(),
                "clash-verge".to_string(),
                "ClashX".to_string(),
            ],
            default_http_port: 7890,
            default_socks_port: 7891,
        },
        VpnConfig {
            name: "V2Ray".to_string(),
            process_names: vec![
                "v2ray".to_string(),
                "v2rayN".to_string(),
                "v2ray-core".to_string(),
            ],
            default_http_port: 10808,
            default_socks_port: 10809,
        },
        VpnConfig {
            name: "Veee".to_string(),
            process_names: vec!["veee".to_string(), "Veee".to_string()],
            default_http_port: 15236,
            default_socks_port: 15235,
        },
        VpnConfig {
            name: "Shadowsocks".to_string(),
            process_names: vec![
                "ss-local".to_string(),
                "shadowsocks".to_string(),
                "Shadowsocks".to_string(),
                "sslocal".to_string(),
            ],
            default_http_port: 1080,
            default_socks_port: 1080,
        },
        VpnConfig {
            name: "Surge".to_string(),
            process_names: vec!["Surge".to_string(), "surge-cli".to_string()],
            default_http_port: 6152,
            default_socks_port: 6153,
        },
    ]
}

/// 根据 VPN 名称检测端口
pub fn detect_port_by_vpn_name(vpn_name: &str) -> DetectionResult {
    let configs = get_vpn_configs();

    // 查找匹配的 VPN 配置
    let config = configs
        .iter()
        .find(|c| c.name.to_lowercase() == vpn_name.to_lowercase());

    match config {
        Some(cfg) => detect_port_by_process_names(&cfg.process_names, cfg),
        None => {
            // 如果不在预设列表中，尝试直接用名字作为进程名搜索
            detect_port_by_custom_name(vpn_name)
        }
    }
}

/// 根据进程名列表检测端口
fn detect_port_by_process_names(process_names: &[String], config: &VpnConfig) -> DetectionResult {
    let mut all_ports = Vec::new();

    for process_name in process_names {
        if let Some(ports) = find_ports_by_process_name(process_name) {
            all_ports.extend(ports);
        }
    }

    if all_ports.is_empty() {
        // 进程未运行，返回默认端口
        DetectionResult {
            success: true,
            message: format!("未检测到 {} 运行，使用默认端口", config.name),
            ports: vec![
                DetectedPort {
                    port: config.default_http_port,
                    port_type: "http".to_string(),
                    process_name: config.name.clone(),
                    pid: 0,
                },
                DetectedPort {
                    port: config.default_socks_port,
                    port_type: "socks".to_string(),
                    process_name: config.name.clone(),
                    pid: 0,
                },
            ],
        }
    } else {
        // 对端口进行分类
        let classified_ports = classify_ports(all_ports, config);
        DetectionResult {
            success: true,
            message: format!("检测到 {} 正在运行", config.name),
            ports: classified_ports,
        }
    }
}

/// 根据自定义名称检测端口
fn detect_port_by_custom_name(name: &str) -> DetectionResult {
    if let Some(ports) = find_ports_by_process_name(name) {
        if !ports.is_empty() {
            return DetectionResult {
                success: true,
                message: format!("检测到 {} 正在运行", name),
                ports,
            };
        }
    }

    DetectionResult {
        success: false,
        message: format!("未找到名为 {} 的进程", name),
        ports: vec![],
    }
}

/// 根据进程名查找监听的端口
#[cfg(target_os = "windows")]
fn find_ports_by_process_name(process_name: &str) -> Option<Vec<DetectedPort>> {
    // Windows: 使用 tasklist 和 netstat
    let tasklist_output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .ok()?;

    let tasklist_str = String::from_utf8_lossy(&tasklist_output.stdout);
    let mut pids: Vec<u32> = Vec::new();

    // 解析 tasklist 输出，查找匹配的进程
    for line in tasklist_str.lines() {
        let lower_line = line.to_lowercase();
        if lower_line.contains(&process_name.to_lowercase()) {
            // CSV 格式: "进程名","PID","会话名","会话#","内存使用"
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
    }

    if pids.is_empty() {
        return None;
    }

    // 使用 netstat 查找这些 PID 监听的端口
    let netstat_output = Command::new("netstat").args(["-ano"]).output().ok()?;

    let netstat_str = String::from_utf8_lossy(&netstat_output.stdout);
    let mut ports = Vec::new();

    for line in netstat_str.lines() {
        if !line.contains("LISTENING") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        // 检查 PID 是否匹配
        if let Ok(pid) = parts[parts.len() - 1].parse::<u32>() {
            if pids.contains(&pid) {
                // 解析本地地址和端口
                let local_addr = parts[1];
                if let Some(port_str) = local_addr.rsplit(':').next() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        // 只关注常见的代理端口范围
                        if port > 1000 && port < 65535 {
                            ports.push(DetectedPort {
                                port,
                                port_type: "unknown".to_string(),
                                process_name: process_name.to_string(),
                                pid,
                            });
                        }
                    }
                }
            }
        }
    }

    Some(ports)
}

#[cfg(target_os = "macos")]
fn find_ports_by_process_name(process_name: &str) -> Option<Vec<DetectedPort>> {
    // macOS: 使用 lsof
    let output = Command::new("lsof")
        .args(["-i", "-P", "-n"])
        .output()
        .ok()?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut ports = Vec::new();

    for line in output_str.lines() {
        let lower_line = line.to_lowercase();
        if !lower_line.contains(&process_name.to_lowercase()) {
            continue;
        }
        if !line.contains("LISTEN") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }

        // lsof 输出格式: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
        let pid = parts[1].parse::<u32>().unwrap_or(0);
        let name_part = parts[8]; // 类似 *:7890 或 127.0.0.1:7890

        if let Some(port_str) = name_part.rsplit(':').next() {
            if let Ok(port) = port_str.parse::<u16>() {
                if port > 1000 && port < 65535 {
                    ports.push(DetectedPort {
                        port,
                        port_type: "unknown".to_string(),
                        process_name: process_name.to_string(),
                        pid,
                    });
                }
            }
        }
    }

    Some(ports)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn find_ports_by_process_name(_process_name: &str) -> Option<Vec<DetectedPort>> {
    // Linux 或其他系统暂不支持
    None
}

/// 对检测到的端口进行分类（HTTP/SOCKS）
fn classify_ports(mut ports: Vec<DetectedPort>, config: &VpnConfig) -> Vec<DetectedPort> {
    // 去重
    ports.sort_by_key(|p| p.port);
    ports.dedup_by_key(|p| p.port);

    // 根据默认端口和常见规则分类
    for port in &mut ports {
        if port.port == config.default_http_port {
            port.port_type = "http".to_string();
        } else if port.port == config.default_socks_port {
            port.port_type = "socks".to_string();
        } else {
            // 常见的 HTTP 代理端口
            let http_ports = [7890, 8080, 8118, 3128, 10808, 15236, 6152];
            // 常见的 SOCKS 代理端口
            let socks_ports = [7891, 1080, 10809, 15235, 6153];

            if http_ports.contains(&port.port) {
                port.port_type = "http".to_string();
            } else if socks_ports.contains(&port.port) {
                port.port_type = "socks".to_string();
            }
        }
    }

    ports
}
