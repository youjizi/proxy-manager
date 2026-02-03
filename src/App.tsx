import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

interface VpnConfig {
  name: string;
  process_names: string[];
  default_http_port: number;
  default_socks_port: number;
}

interface DetectedPort {
  port: number;
  port_type: string;
  process_name: string;
  pid: number;
}

interface DetectionResult {
  success: boolean;
  message: string;
  ports: DetectedPort[];
}

interface SoftwareConfig {
  name: string;
  config_type: string;
  enabled: boolean;
  installed: boolean;
  config_path: string | null;
  is_custom?: boolean;
}

interface ProxyProfile {
  name: string;
  host: string;
  port: number;
}

interface SoftwareProxyMapping {
  software_name: string;
  profile_name: string;
}

interface CustomSoftware {
  name: string;
  config_type: string;
  config_path: string;
}

interface UserConfig {
  profiles: ProxyProfile[];
  mappings: SoftwareProxyMapping[];
  custom_software: CustomSoftware[];
}

interface ClosePreference {
  remember: boolean;
  action: string;
}

function App() {
  const [vpnList, setVpnList] = useState<VpnConfig[]>([]);
  const [selectedVpn, setSelectedVpn] = useState<string>("");
  const [customVpn, setCustomVpn] = useState<string>("");
  const [detectionResult, setDetectionResult] = useState<DetectionResult | null>(null);
  const [isDetecting, setIsDetecting] = useState(false);

  const [softwareList, setSoftwareList] = useState<SoftwareConfig[]>([]);
  const [selectedSoftware, setSelectedSoftware] = useState<Set<string>>(new Set());
  const [expandedSoftware, setExpandedSoftware] = useState<string | null>(null);

  const [isProxyEnabled, setIsProxyEnabled] = useState(false);
  const [operationResults, setOperationResults] = useState<string[]>([]);
  const [isOperating, setIsOperating] = useState(false);

  // 代理配置组相关状态
  const [userConfig, setUserConfig] = useState<UserConfig>({ profiles: [], mappings: [], custom_software: [] });
  const [softwareMappings, setSoftwareMappings] = useState<Map<string, string>>(new Map());
  const [showProfileModal, setShowProfileModal] = useState(false);
  const [editingProfile, setEditingProfile] = useState<ProxyProfile | null>(null);
  const [newProfile, setNewProfile] = useState<ProxyProfile>({ name: "", host: "127.0.0.1", port: 7890 });

  // 自定义软件相关状态
  const [showSoftwareModal, setShowSoftwareModal] = useState(false);
  const [newSoftware, setNewSoftware] = useState<CustomSoftware>({ name: "", config_type: "json", config_path: "" });

  // 关闭确认对话框相关状态
  const [showCloseModal, setShowCloseModal] = useState(false);
  const [closeAction, setCloseAction] = useState<string>("minimize");
  const [rememberClose, setRememberClose] = useState(false);

  useEffect(() => {
    loadVpnList();
    loadSoftwareList();
    loadUserConfig();

    // 监听窗口关闭事件
    const unlisten = listen("close-requested", async () => {
      // 检查是否已记住选择
      try {
        const pref = await invoke<ClosePreference>("get_close_preference");
        if (pref.remember) {
          // 已记住选择，直接执行
          if (pref.action === "exit") {
            await invoke("exit_app");
          } else {
            await invoke("hide_window");
          }
        } else {
          // 未记住选择，显示对话框
          setShowCloseModal(true);
        }
      } catch (e) {
        console.error("Failed to get close preference:", e);
        setShowCloseModal(true);
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  async function loadVpnList() {
    try {
      const list = await invoke<VpnConfig[]>("get_vpn_list");
      setVpnList(list);
    } catch (e) {
      console.error("Failed to load VPN list:", e);
    }
  }

  async function loadSoftwareList() {
    try {
      const list = await invoke<SoftwareConfig[]>("get_software_list");
      setSoftwareList(list);
      const installed = new Set(list.filter((s) => s.installed).map((s) => s.name));
      setSelectedSoftware(installed);
    } catch (e) {
      console.error("Failed to load software list:", e);
    }
  }

  async function loadUserConfig() {
    try {
      const config = await invoke<UserConfig>("get_user_config");
      setUserConfig(config);

      // 初始化软件映射
      const mappings = new Map<string, string>();
      config.mappings.forEach((m) => {
        mappings.set(m.software_name, m.profile_name);
      });
      setSoftwareMappings(mappings);
    } catch (e) {
      console.error("Failed to load user config:", e);
    }
  }

  async function detectPort() {
    const vpnName = selectedVpn === "custom" ? customVpn : selectedVpn;
    if (!vpnName) return;

    setIsDetecting(true);
    setDetectionResult(null);

    try {
      const result = await invoke<DetectionResult>("detect_port", { vpnName });
      setDetectionResult(result);
    } catch (e) {
      setDetectionResult({
        success: false,
        message: `检测失败: ${e}`,
        ports: [],
      });
    } finally {
      setIsDetecting(false);
    }
  }

  async function toggleProxy() {
    if (selectedSoftware.size === 0) {
      setOperationResults(["请至少选择一个软件"]);
      return;
    }

    setIsOperating(true);
    setOperationResults([]);

    try {
      const softwareArray = Array.from(selectedSoftware);

      if (isProxyEnabled) {
        const results = await invoke<string[]>("disable_proxy", {
          softwareList: softwareArray,
        });
        setOperationResults(results);
        setIsProxyEnabled(false);
      } else {
        // 使用配置组方式开启代理
        const mappingsToApply: SoftwareProxyMapping[] = softwareArray.map((name) => ({
          software_name: name,
          profile_name: softwareMappings.get(name) || userConfig.profiles[0]?.name || "",
        }));

        const results = await invoke<string[]>("enable_proxy_with_profiles", {
          softwareMappings: mappingsToApply,
        });
        setOperationResults(results);
        setIsProxyEnabled(true);
      }
    } catch (e) {
      setOperationResults([`操作失败: ${e}`]);
    } finally {
      setIsOperating(false);
    }
  }

  function toggleSoftwareSelection(name: string) {
    const newSet = new Set(selectedSoftware);
    if (newSet.has(name)) {
      newSet.delete(name);
    } else {
      newSet.add(name);
    }
    setSelectedSoftware(newSet);
  }

  async function resetToOriginal() {
    if (selectedSoftware.size === 0) {
      setOperationResults(["请至少选择一个软件"]);
      return;
    }

    setIsOperating(true);
    setOperationResults([]);

    try {
      const softwareArray = Array.from(selectedSoftware);
      const results = await invoke<string[]>("reset_proxy", {
        softwareList: softwareArray,
      });
      setOperationResults(results);
      setIsProxyEnabled(false);
    } catch (e) {
      setOperationResults([`重置失败: ${e}`]);
    } finally {
      setIsOperating(false);
    }
  }

  function selectAll() {
    const installed = new Set(softwareList.filter((s) => s.installed).map((s) => s.name));
    setSelectedSoftware(installed);
  }

  function selectNone() {
    setSelectedSoftware(new Set());
  }

  // 代理配置组管理函数
  async function saveProfile() {
    try {
      const profile = editingProfile || newProfile;
      if (!profile.name.trim()) {
        setOperationResults(["配置组名称不能为空"]);
        return;
      }

      if (editingProfile) {
        // 更新现有配置
        const updatedProfiles = userConfig.profiles.map((p) =>
          p.name === editingProfile.name ? profile : p
        );
        const newConfig = { ...userConfig, profiles: updatedProfiles };
        await invoke("save_user_config", { config: newConfig });
        setUserConfig(newConfig);
      } else {
        // 添加新配置
        const config = await invoke<UserConfig>("add_proxy_profile", { profile });
        setUserConfig(config);
      }

      setShowProfileModal(false);
      setEditingProfile(null);
      setNewProfile({ name: "", host: "127.0.0.1", port: 7890 });
    } catch (e) {
      setOperationResults([`保存失败: ${e}`]);
    }
  }

  async function deleteProfile(profileName: string) {
    try {
      const config = await invoke<UserConfig>("delete_proxy_profile", { profileName });
      setUserConfig(config);

      // 清除使用该配置的映射
      const newMappings = new Map(softwareMappings);
      for (const [software, profile] of newMappings) {
        if (profile === profileName) {
          newMappings.delete(software);
        }
      }
      setSoftwareMappings(newMappings);
    } catch (e) {
      setOperationResults([`删除失败: ${e}`]);
    }
  }

  async function updateSoftwareMapping(softwareName: string, profileName: string) {
    try {
      const config = await invoke<UserConfig>("update_software_mapping", {
        softwareName,
        profileName,
      });
      setUserConfig(config);

      const newMappings = new Map(softwareMappings);
      newMappings.set(softwareName, profileName);
      setSoftwareMappings(newMappings);
    } catch (e) {
      setOperationResults([`更新映射失败: ${e}`]);
    }
  }

  function openAddProfileModal() {
    setEditingProfile(null);
    setNewProfile({ name: "", host: "127.0.0.1", port: 7890 });
    setShowProfileModal(true);
  }

  function openEditProfileModal(profile: ProxyProfile) {
    setEditingProfile({ ...profile });
    setShowProfileModal(true);
  }

  // 自定义软件管理函数
  function openAddSoftwareModal() {
    setNewSoftware({ name: "", config_type: "json", config_path: "" });
    setShowSoftwareModal(true);
  }

  async function saveCustomSoftware() {
    try {
      if (!newSoftware.name.trim()) {
        setOperationResults(["软件名称不能为空"]);
        return;
      }
      if (!newSoftware.config_path.trim()) {
        setOperationResults(["配置文件路径不能为空"]);
        return;
      }

      const config = await invoke<UserConfig>("add_custom_software", { software: newSoftware });
      setUserConfig(config);
      setShowSoftwareModal(false);
      setNewSoftware({ name: "", config_type: "json", config_path: "" });
      // 重新加载软件列表
      await loadSoftwareList();
    } catch (e) {
      setOperationResults([`添加失败: ${e}`]);
    }
  }

  async function deleteCustomSoftware(softwareName: string) {
    try {
      const config = await invoke<UserConfig>("delete_custom_software", { softwareName });
      setUserConfig(config);
      // 从选中列表中移除
      const newSelected = new Set(selectedSoftware);
      newSelected.delete(softwareName);
      setSelectedSoftware(newSelected);
      // 重新加载软件列表
      await loadSoftwareList();
    } catch (e) {
      setOperationResults([`删除失败: ${e}`]);
    }
  }

  async function handleExit() {
    await invoke("exit_app");
  }

  // 处理关闭确认
  async function handleCloseConfirm() {
    try {
      // 如果勾选了记住选择，保存偏好
      if (rememberClose) {
        await invoke("save_close_preference", {
          preference: { remember: true, action: closeAction }
        });
      }

      // 执行选择的操作
      if (closeAction === "exit") {
        await invoke("exit_app");
      } else {
        await invoke("hide_window");
      }
    } catch (e) {
      console.error("Failed to handle close:", e);
    }
    setShowCloseModal(false);
  }

  // 取消关闭
  function handleCloseCancel() {
    setShowCloseModal(false);
  }

  return (
    <main className="app-container">
      {/* 标题栏 */}
      <header className="app-header">
        <h1>Proxy Manager</h1>
      </header>

      <div className="app-content">
        {/* 左侧：VPN 检测和代理配置组 */}
        <section className="card">
          <div className="card-header">
            <span className="card-icon blue">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10"/>
                <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
              </svg>
            </span>
            <h2>VPN 检测</h2>
          </div>

          <div className="form-group">
            <select
              value={selectedVpn}
              onChange={(e) => setSelectedVpn(e.target.value)}
              className="select-input"
            >
              <option value="">选择 VPN 软件</option>
              {vpnList.map((vpn) => (
                <option key={vpn.name} value={vpn.name}>{vpn.name}</option>
              ))}
              <option value="custom">其他...</option>
            </select>
          </div>

          {selectedVpn === "custom" && (
            <div className="form-group">
              <input
                type="text"
                value={customVpn}
                onChange={(e) => setCustomVpn(e.target.value)}
                placeholder="输入 VPN 名称"
                className="text-input"
              />
            </div>
          )}

          <button
            onClick={detectPort}
            disabled={isDetecting || (!selectedVpn || (selectedVpn === "custom" && !customVpn))}
            className="btn btn-secondary"
          >
            {isDetecting ? "检测中..." : "检测端口"}
          </button>

          {detectionResult && (
            <div className={`result-box ${detectionResult.success ? "success" : "error"}`}>
              <p>{detectionResult.message}</p>
              {detectionResult.ports.length > 0 && (
                <div className="port-list">
                  {detectionResult.ports.map((port, idx) => (
                    <span key={idx} className="port-tag">
                      {port.port} ({port.port_type})
                    </span>
                  ))}
                </div>
              )}
            </div>
          )}

          <div className="divider" />

          {/* 代理配置组 */}
          <div className="card-header">
            <span className="card-icon purple">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="3"/>
                <path d="M12 1v6m0 6v10M4.22 4.22l4.24 4.24m7.08 7.08l4.24 4.24M1 12h6m6 0h10M4.22 19.78l4.24-4.24m7.08-7.08l4.24-4.24"/>
              </svg>
            </span>
            <h2>代理配置组</h2>
            <div className="header-actions">
              <button onClick={openAddProfileModal} className="link-btn">+ 添加</button>
            </div>
          </div>

          <div className="profile-list">
            {userConfig.profiles.map((profile) => (
              <div key={profile.name} className="profile-item">
                <div className="profile-info">
                  <span className="profile-name">{profile.name}</span>
                  <span className="profile-address">{profile.host}:{profile.port}</span>
                </div>
                <div className="profile-actions">
                  <button
                    onClick={() => openEditProfileModal(profile)}
                    className="icon-btn"
                    title="编辑"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
                    </svg>
                  </button>
                  <button
                    onClick={() => deleteProfile(profile.name)}
                    className="icon-btn danger"
                    title="删除"
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                    </svg>
                  </button>
                </div>
              </div>
            ))}
            {userConfig.profiles.length === 0 && (
              <div className="empty-state">暂无配置组，点击添加</div>
            )}
          </div>
        </section>

        {/* 右侧：软件列表 */}
        <section className="card">
          <div className="card-header">
            <span className="card-icon green">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="3" y="3" width="18" height="18" rx="2"/>
                <path d="M9 9h6M9 13h6M9 17h4"/>
              </svg>
            </span>
            <h2>软件配置</h2>
            <div className="header-actions">
              <button onClick={openAddSoftwareModal} className="link-btn">+ 添加</button>
              <span className="separator">|</span>
              <button onClick={selectAll} className="link-btn">全选</button>
              <span className="separator">|</span>
              <button onClick={selectNone} className="link-btn">清空</button>
            </div>
          </div>

          <div className="software-list">
            {softwareList.map((software) => (
              <div key={software.name} className={`software-item ${!software.installed ? "disabled" : ""}`}>
                <div className="software-row">
                  <label className="checkbox-label">
                    <input
                      type="checkbox"
                      checked={selectedSoftware.has(software.name)}
                      onChange={() => toggleSoftwareSelection(software.name)}
                      disabled={!software.installed}
                    />
                    <span className="checkbox-custom" />
                    <span className="software-name">{software.name}</span>
                  </label>

                  <div className="software-actions">
                    {software.installed && userConfig.profiles.length > 0 && (
                      <select
                        value={softwareMappings.get(software.name) || ""}
                        onChange={(e) => updateSoftwareMapping(software.name, e.target.value)}
                        className="profile-select"
                      >
                        <option value="">选择配置</option>
                        {userConfig.profiles.map((profile) => (
                          <option key={profile.name} value={profile.name}>
                            {profile.name}
                          </option>
                        ))}
                      </select>
                    )}
                    {software.config_path && (
                      <button
                        onClick={() => setExpandedSoftware(
                          expandedSoftware === software.name ? null : software.name
                        )}
                        className="icon-btn"
                        title="查看配置路径"
                      >
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <path d={expandedSoftware === software.name ? "M18 15l-6-6-6 6" : "M6 9l6 6 6-6"} />
                        </svg>
                      </button>
                    )}
                    {software.is_custom && (
                      <button
                        onClick={() => deleteCustomSoftware(software.name)}
                        className="icon-btn danger"
                        title="删除自定义软件"
                      >
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                        </svg>
                      </button>
                    )}
                    <span className={`status-badge ${software.installed ? "installed" : ""} ${software.is_custom ? "custom" : ""}`}>
                      {software.is_custom ? "自定义" : software.installed ? "已安装" : "未检测"}
                    </span>
                  </div>
                </div>

                {expandedSoftware === software.name && software.config_path && (
                  <div className="config-path">
                    <code>{software.config_path}</code>
                  </div>
                )}
              </div>
            ))}
          </div>
        </section>
      </div>

      {/* 底部操作栏 */}
      <footer className="app-footer">
        <div className="footer-left">
          <span className="footer-info">
            备份位置: %LOCALAPPDATA%\proxy-manager\backups
          </span>
          <div className="reset-wrapper">
            <button
              onClick={resetToOriginal}
              disabled={isOperating || selectedSoftware.size === 0}
              className="btn-reset"
              title="重置到初始状态"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
                <path d="M3 3v5h5"/>
              </svg>
              重置
            </button>
            <div className="reset-tooltip">
              将选中软件的配置文件恢复到首次使用本工具前的原始状态。
              此操作会覆盖当前配置，请谨慎操作。
            </div>
          </div>
        </div>
        <div className="footer-actions">
          <button
            onClick={toggleProxy}
            disabled={isOperating || selectedSoftware.size === 0}
            className={`btn ${isProxyEnabled ? "btn-danger" : "btn-primary"}`}
          >
            {isOperating ? "处理中..." : isProxyEnabled ? "关闭代理" : "开启代理"}
          </button>
        </div>
      </footer>

      {/* 操作结果弹窗 */}
      {operationResults.length > 0 && (
        <div className="toast">
          <div className="toast-header">
            <span>操作结果</span>
            <button onClick={() => setOperationResults([])} className="toast-close">×</button>
          </div>
          <div className="toast-body">
            {operationResults.map((result, idx) => (
              <div key={idx} className={`toast-item ${result.startsWith("✓") ? "success" : "error"}`}>
                {result}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 配置组编辑弹窗 */}
      {showProfileModal && (
        <div className="modal-overlay" onClick={() => setShowProfileModal(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>{editingProfile ? "编辑配置组" : "添加配置组"}</h3>
              <button onClick={() => setShowProfileModal(false)} className="modal-close">×</button>
            </div>
            <div className="modal-body">
              <div className="form-group">
                <label>名称</label>
                <input
                  type="text"
                  value={editingProfile ? editingProfile.name : newProfile.name}
                  onChange={(e) => {
                    if (editingProfile) {
                      setEditingProfile({ ...editingProfile, name: e.target.value });
                    } else {
                      setNewProfile({ ...newProfile, name: e.target.value });
                    }
                  }}
                  placeholder="例如: Clash"
                  className="text-input"
                />
              </div>
              <div className="form-row">
                <div className="form-group flex-1">
                  <label>主机地址</label>
                  <input
                    type="text"
                    value={editingProfile ? editingProfile.host : newProfile.host}
                    onChange={(e) => {
                      if (editingProfile) {
                        setEditingProfile({ ...editingProfile, host: e.target.value });
                      } else {
                        setNewProfile({ ...newProfile, host: e.target.value });
                      }
                    }}
                    placeholder="127.0.0.1"
                    className="text-input"
                  />
                </div>
                <div className="form-group" style={{ width: 100 }}>
                  <label>端口</label>
                  <input
                    type="number"
                    value={editingProfile ? editingProfile.port : newProfile.port}
                    onChange={(e) => {
                      const port = parseInt(e.target.value) || 0;
                      if (editingProfile) {
                        setEditingProfile({ ...editingProfile, port });
                      } else {
                        setNewProfile({ ...newProfile, port });
                      }
                    }}
                    className="text-input"
                  />
                </div>
              </div>
            </div>
            <div className="modal-footer">
              <button onClick={() => setShowProfileModal(false)} className="btn btn-secondary">
                取消
              </button>
              <button onClick={saveProfile} className="btn btn-primary">
                保存
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 自定义软件添加弹窗 */}
      {showSoftwareModal && (
        <div className="modal-overlay" onClick={() => setShowSoftwareModal(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>添加自定义软件</h3>
              <button onClick={() => setShowSoftwareModal(false)} className="modal-close">×</button>
            </div>
            <div className="modal-body">
              <div className="form-group">
                <label>软件名称</label>
                <input
                  type="text"
                  value={newSoftware.name}
                  onChange={(e) => setNewSoftware({ ...newSoftware, name: e.target.value })}
                  placeholder="例如: MyApp"
                  className="text-input"
                />
              </div>
              <div className="form-group">
                <label>配置文件类型</label>
                <select
                  value={newSoftware.config_type}
                  onChange={(e) => setNewSoftware({ ...newSoftware, config_type: e.target.value })}
                  className="select-input"
                >
                  <option value="json">JSON</option>
                  <option value="ini">INI</option>
                  <option value="env">ENV</option>
                </select>
              </div>
              <div className="form-group">
                <label>配置文件路径</label>
                <input
                  type="text"
                  value={newSoftware.config_path}
                  onChange={(e) => setNewSoftware({ ...newSoftware, config_path: e.target.value })}
                  placeholder="例如: C:\Users\xxx\.myapprc"
                  className="text-input"
                />
                <span className="form-hint">支持环境变量如 %USERPROFILE%</span>
              </div>
            </div>
            <div className="modal-footer">
              <button onClick={() => setShowSoftwareModal(false)} className="btn btn-secondary">
                取消
              </button>
              <button onClick={saveCustomSoftware} className="btn btn-primary">
                添加
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 关闭确认对话框 */}
      {showCloseModal && (
        <div className="modal-overlay">
          <div className="modal close-modal">
            <div className="modal-header">
              <h3>关闭窗口</h3>
            </div>
            <div className="modal-body">
              <p className="close-modal-desc">请选择关闭窗口时的操作：</p>
              <div className="close-options">
                <label className="radio-label">
                  <input
                    type="radio"
                    name="closeAction"
                    value="minimize"
                    checked={closeAction === "minimize"}
                    onChange={(e) => setCloseAction(e.target.value)}
                  />
                  <span className="radio-custom" />
                  <span className="radio-text">最小化到系统托盘</span>
                </label>
                <label className="radio-label">
                  <input
                    type="radio"
                    name="closeAction"
                    value="exit"
                    checked={closeAction === "exit"}
                    onChange={(e) => setCloseAction(e.target.value)}
                  />
                  <span className="radio-custom" />
                  <span className="radio-text">退出程序</span>
                </label>
              </div>
              <label className="checkbox-label remember-checkbox">
                <input
                  type="checkbox"
                  checked={rememberClose}
                  onChange={(e) => setRememberClose(e.target.checked)}
                />
                <span className="checkbox-custom" />
                <span>记住我的选择</span>
              </label>
            </div>
            <div className="modal-footer">
              <button onClick={handleCloseCancel} className="btn btn-secondary">
                取消
              </button>
              <button onClick={handleCloseConfirm} className="btn btn-primary">
                确定
              </button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
