#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::time::Duration;
use anyhow::Result;

/// HWiNFO 进程管理器
///
/// 负责 HWiNFO 可执行文件的启动、停止和生命周期管理。
/// 注意：HWiNFO 仅存在于 Windows 平台。
#[cfg(target_os = "windows")]
pub struct HWiNFOManager {
    path: PathBuf,
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

        Ok(Self {
            path,
        })
    }

    /// 启动 HWiNFO
    ///
    /// 如果 HWiNFO 已在运行，则跳过启动。
    /// 使用 PowerShell Start-Process 启动，隐藏窗口避免置顶
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
        let output = Command::new("powershell")
            .arg("-Command")
            .arg(format!("Start-Process -FilePath '{}' -WindowStyle Hidden", path_str))
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
        // 使用 taskkill 强制终止 HWiNFO64.EXE
        let _output = Command::new("taskkill")
            .arg("/f")
            .arg("/im")
            .arg("HWiNFO64.EXE")
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
        // 使用 tasklist 检查进程是否存在
        let output = Command::new("tasklist")
            .arg("/fi")
            .arg("imagename eq HWiNFO64.EXE")
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
        let mut manager = HWiNFOManager::new(Some("C:\\path\\to\\HWiNFO64.EXE")).unwrap();
        assert!(!manager.is_running());
    }
}