#[cfg(target_os = "windows")]
use std::process::{Child, Command};
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
    process: Option<Child>,
    path: PathBuf,
}

#[cfg(target_os = "windows")]
impl HWiNFOManager {
    /// 创建新的 HWiNFO 管理器
    ///
    /// # 参数
    /// - `hwinfo_path`: 可选的 HWiNFO.exe 路径，若为 None 则使用扩展模块所在目录
    ///
    /// # 返回
    /// 成功返回 HWiNFOManager 实例
    pub fn new(hwinfo_path: Option<&str>) -> Result<Self> {
        // 默认使用扩展同目录下的 HWiNFO.exe
        let path = if let Some(p) = hwinfo_path {
            PathBuf::from(p)
        } else {
            // 获取扩展模块所在目录
            let exe_dir = std::env::current_exe()?
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Cannot get exe directory"))?
                .to_path_buf();
            exe_dir.join("HWiNFO.exe")
        };

        Ok(Self {
            process: None,
            path,
        })
    }

    /// 启动 HWiNFO（最小化托盘，启用共享内存）
    ///
    /// HWiNFO 启动参数需要预先配置好：
    /// - 共享内存需要在 HWiNFO 设置中启用
    /// - 这里使用 /minimize 参数让程序最小化启动到托盘
    ///
    /// # 返回
    /// 成功返回 Ok(())，失败返回错误
    pub fn start(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Ok(());  // 已经启动
        }

        // HWiNFO 启动参数需要预先配置好
        // 这里假设用户已经配置了共享内存和最小化托盘
        let child = Command::new(&self.path)
            .arg("/minimize")  // 最小化启动
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start HWiNFO: {}", e))?;

        self.process = Some(child);

        // 等待共享内存生效（大约需要几秒）
        std::thread::sleep(Duration::from_secs(3));

        Ok(())
    }

    /// 停止 HWiNFO
    ///
    /// # 返回
    /// 成功返回 Ok(())，失败返回错误
    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            process.kill()
                .map_err(|e| anyhow::anyhow!("Failed to kill HWiNFO: {}", e))?;
            process.wait().ok(); // 等待进程回收，忽略错误
        }
        Ok(())
    }

    /// 检查 HWiNFO 是否运行
    ///
    /// # 返回
    /// true 表示进程仍在运行，false 表示已退出或未启动
    pub fn is_running(&mut self) -> bool {
        if let Some(process) = &mut self.process {
            process.try_wait().map(|w| w.is_none()).unwrap_or(false)
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
        let manager = HWiNFOManager::new(Some("C:\\path\\to\\HWiNFO.exe"));
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        assert_eq!(manager.path(), &PathBuf::from("C:\\path\\to\\HWiNFO.exe"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_is_running_when_not_started() {
        let manager = HWiNFOManager::new(Some("C:\\path\\to\\HWiNFO.exe")).unwrap();
        assert!(!manager.is_running());
    }
}