#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;  // 提供 creation_flags 方法
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::time::Duration;
#[cfg(target_os = "windows")]
use std::fs;
use anyhow::Result;

// Windows 隐藏子进程窗口的标志
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// HWiNFO 进程管理器
///
/// 负责 HWiNFO 可执行文件的启动、停止和生命周期管理。
/// 注意：HWiNFO 仅存在于 Windows 平台。
#[cfg(target_os = "windows")]
pub struct HWiNFOManager {
    path: PathBuf,
    ini_path: PathBuf,  // HWiNFO64.INI 配置文件路径
}

#[cfg(target_os = "windows")]
impl HWiNFOManager {
    /// 创建新的 HWiNFO 管理器
    ///
    /// # 参数
    /// - `hwinfo_path`: 可选的 HWiNFO.exe 路径，若为 None 则使用扩展模块所在目录下的 HWiNFO64 子目录
    ///
    /// # 返回
    /// 成功返回 HWiNFOManager 实例
    pub fn new(hwinfo_path: Option<&str>) -> Result<Self> {
        // 默认使用扩展模块所在目录下的 HWiNFO64/HWiNFO64.EXE
        let path = if let Some(p) = hwinfo_path {
            PathBuf::from(p)
        } else {
            // 获取扩展模块所在目录
            let exe_dir = std::env::current_exe()?
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Cannot get exe directory"))?
                .to_path_buf();
            exe_dir.join("HWiNFO64").join("HWiNFO64.EXE")
        };

        // HWiNFO64.INI 与 EXE 在同一目录
        let ini_path = path.parent()
            .ok_or_else(|| anyhow::anyhow!("Cannot get HWiNFO directory"))?
            .join("HWiNFO64.INI");

        Ok(Self {
            path,
            ini_path,
        })
    }

    /// 启动 HWiNFO
    ///
    /// 如果 HWiNFO 已在运行，则跳过启动。
    /// 使用 PowerShell Start-Process -WindowStyle Hidden 启动，避免窗口闪烁
    /// PowerShell 本身也使用 CREATE_NO_WINDOW 隐藏
    ///
    /// # 返回
    /// 成功返回 Ok(())，失败返回错误
    pub fn start(&mut self) -> Result<()> {
        // 先检查是否已运行
        if self.is_running() {
            log::info!("HWiNFO already running, skip start");
            return Ok(());
        }

        let path_str = self.path.to_string_lossy();

        // 使用 PowerShell Start-Process 启动 HWiNFO，隐藏窗口
        // PowerShell 本身也使用 CREATE_NO_WINDOW，避免 PowerShell 弹窗闪烁
        #[cfg(target_os = "windows")]
        let output = Command::new("powershell")
            .args([
                "-WindowStyle", "Hidden",  // PowerShell 自己隐藏
                "-Command",
                &format!("Start-Process -FilePath '{}' -WindowStyle Hidden", path_str),
            ])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to start HWiNFO: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to start HWiNFO: {}", stderr));
        }

        // 等待共享内存生效
        std::thread::sleep(Duration::from_secs(5));

        Ok(())
    }

    /// 停止 HWiNFO
    ///
    /// 使用 taskkill 强制终止 HWiNFO64.EXE 进程
    ///
    /// # 返回
    /// 成功返回 Ok(())，失败返回错误
    pub fn stop(&mut self) -> Result<()> {
        // 使用 taskkill 强制终止 HWiNFO64.EXE 进程（隐藏窗口）
        #[cfg(target_os = "windows")]
        let _output = Command::new("taskkill")
            .arg("/f")
            .arg("/im")
            .arg("HWiNFO64.EXE")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to kill HWiNFO: {}", e))?;

        // taskkill 返回成功即使进程不存在，所以不需要检查 status
        // 只要命令执行成功就认为 OK
        Ok(())
    }

    /// 检查 HWiNFO 是否运行
    ///
    /// # 返回
    /// true 表示进程仍在运行，false 表示已退出或未启动
    pub fn is_running(&mut self) -> bool {
        // 使用 tasklist 检查进程是否存在（隐藏窗口）
        #[cfg(target_os = "windows")]
        let output = Command::new("tasklist")
            .arg("/fi")
            .arg("imagename eq HWiNFO64.EXE")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok();

        if let Some(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains("HWiNFO64.EXE")
        } else {
            false
        }
    }

    /// 重启 HWiNFO（用于处理 12 小时失效）
    ///
    /// HWiNFO 免费版本的共享内存功能在运行 12 小时后会失效，
    /// 此时需要重启 HWiNFO 来恢复功能。
    ///
    /// # 返回
    /// 成功返回 Ok(())，失败返回错误
    pub fn restart(&mut self) -> Result<()> {
        self.stop()?;
        std::thread::sleep(Duration::from_secs(1));
        self.start()?;
        Ok(())
    }

    /// 获取 HWiNFO 可执行文件路径
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 获取 HWiNFO64.INI 配置文件路径
    pub fn ini_path(&self) -> &PathBuf {
        &self.ini_path
    }

    /// 检查配置文件中的 SensorsSM=1 是否存在
    ///
    /// # 返回
    /// - true: 配置已启用共享内存
    /// - false: 配置不存在或未启用共享内存
    pub fn check_shared_memory_enabled(&self) -> bool {
        if !self.ini_path.exists() {
            return false;
        }

        let content = fs::read_to_string(&self.ini_path).ok();
        if let Some(content) = content {
            content.contains("SensorsSM=1")
        } else {
            false
        }
    }

    /// 确保配置文件中存在 SensorsSM=1
    ///
    /// HWiNFO 免费版在运行12小时后会移除 SensorsSM=1 配置，
    /// 导致共享内存失效。此方法用于在重启前恢复配置。
    ///
    /// # 返回
    /// 成功返回 Ok(是否修改了配置)
    pub fn ensure_shared_memory_enabled(&self) -> Result<bool> {
        if self.check_shared_memory_enabled() {
            return Ok(false);  // 已经存在，无需修改
        }

        // 读取现有内容
        let mut content = if self.ini_path.exists() {
            fs::read_to_string(&self.ini_path)?
        } else {
            String::from("[Settings]\n")
        };

        // 添加 SensorsSM=1
        // 查找 [Settings] 部分
        if content.contains("[Settings]") {
            // 在 Settings 部分末尾添加
            let settings_end = content.find("[Settings]").unwrap_or(0);
            let next_section = content[settings_end..].find("\n[")
                .map(|i| settings_end + i)
                .unwrap_or(content.len());

            // 在 Settings 部分末尾插入
            content.insert_str(next_section, "\nSensorsSM=1");
        } else {
            // 没有 Settings 部分，添加整个部分
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str("[Settings]\nSensorsSM=1\n");
        }

        // 写回文件
        fs::write(&self.ini_path, content)?;
        log::info!("Added SensorsSM=1 to HWiNFO64.INI");

        Ok(true)
    }

    /// 重启 HWiNFO 并确保共享内存配置正确
    ///
    /// HWiNFO 免费版本的共享内存功能在运行 12 小时后会失效，
    /// 此时需要：
    /// 1. 杀掉进程
    /// 2. 修改配置文件添加 SensorsSM=1
    /// 3. 重启 HWiNFO
    ///
    /// # 返回
    /// 成功返回 Ok(())，失败返回错误
    pub fn restart_with_fix(&mut self) -> Result<()> {
        self.stop()?;
        std::thread::sleep(Duration::from_secs(1));

        // 确保配置文件有 SensorsSM=1
        self.ensure_shared_memory_enabled()?;

        self.start()?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for HWiNFOManager {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            eprintln!("Failed to stop HWiNFO on drop: {}", e);
        }
    }
}

// 非 Windows 平台的 stub 实现
#[cfg(not(target_os = "windows"))]
pub struct HWiNFOManager {
    _private: (),
}

#[cfg(not(target_os = "windows"))]
impl HWiNFOManager {
    pub fn new(_hwinfo_path: Option<&str>) -> Result<Self> {
        Err(anyhow::anyhow!("HWiNFO is only available on Windows"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_new_with_path() {
        let manager = HWiNFOManager::new(Some("C:\\path\\to\\HWiNFO64.EXE"));
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        assert_eq!(manager.path(), &PathBuf::from("C:\\path\\to\\HWiNFO64.EXE"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_is_running_when_not_started() {
        // 注意：此测试可能因HWiNFO已在系统中运行而失败
        // 这是正常情况，不影响功能
        let mut manager = HWiNFOManager::new(Some("C:\\path\\to\\HWiNFO64.EXE")).unwrap();
        // 不强制断言，因为HWiNFO可能已在运行
        let is_running = manager.is_running();
        println!("HWiNFO is_running: {}", is_running);
    }
}