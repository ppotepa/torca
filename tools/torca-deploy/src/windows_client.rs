//! Narrow ownership boundary for the Windows Torca client process.
//!
//! Build and launch must make the same safety decision: they may affect only
//! `torca_app.exe` instances produced by *this* checkout, never a similarly
//! named executable from another workspace.

use std::collections::BTreeMap;
use std::time::Duration;

use thiserror::Error;

use crate::paths::RuntimePaths;
use crate::process::{CommandRunner, CommandSpec, ProcessError};

pub struct WorkspaceWindowsClient<'a> {
    paths: &'a RuntimePaths,
    runner: &'a dyn CommandRunner,
}

impl<'a> WorkspaceWindowsClient<'a> {
    pub fn new(paths: &'a RuntimePaths, runner: &'a dyn CommandRunner) -> Self {
        Self { paths, runner }
    }

    pub fn stop(&self) -> Result<(), WindowsClientError> {
        if !cfg!(windows) {
            return Ok(());
        }
        let root = self.runner_root();
        let script = format!(
            "$ErrorActionPreference = 'Stop'; \
             $root = [IO.Path]::GetFullPath('{root}').TrimEnd('\\'); \
             $clients = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue | \
               Where-Object {{ $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) }}); \
             $clients | ForEach-Object {{ try {{ [void]$_.CloseMainWindow() }} catch {{}} }}; \
             if ($clients.Count -gt 0) {{ Start-Sleep -Seconds 2 }}; \
             $clients = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue | \
               Where-Object {{ $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) }}); \
             $clients | ForEach-Object {{ Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }}; \
             $deadline = [DateTime]::UtcNow.AddSeconds(15); \
             do {{ \
               Start-Sleep -Milliseconds 250; \
               $clients = @(Get-Process -Name torca_app -ErrorAction SilentlyContinue | \
                 Where-Object {{ $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) }}); \
             }} while ($clients.Count -gt 0 -and [DateTime]::UtcNow -lt $deadline); \
             if ($clients.Count -gt 0) {{ throw ('Torca Windows client did not stop: PID(s) ' + ($clients.Id -join ', ')) }}; \
             'TORCA_CLIENT_STOPPED'"
        );
        let output = self.run(&script)?;
        if output.success && output.text.contains("TORCA_CLIENT_STOPPED") {
            Ok(())
        } else {
            Err(WindowsClientError::Command(output.text))
        }
    }

    pub fn is_running(&self) -> Result<bool, WindowsClientError> {
        if !cfg!(windows) {
            return Ok(false);
        }
        let root = self.runner_root();
        let output = self.run(&format!(
            "$root = [IO.Path]::GetFullPath('{root}').TrimEnd('\\'); \
             if(Get-Process -Name torca_app -ErrorAction SilentlyContinue | \
             Where-Object {{ $_.Path -and $_.Path.StartsWith($root,[StringComparison]::OrdinalIgnoreCase) }} | \
             Select-Object -First 1) {{ 'RUNNING' }}"
        ))?;
        Ok(output.success && output.text.contains("RUNNING"))
    }

    /// Brings the verified workspace window to foreground and reports whether
    /// a real visible window exists.
    pub fn activate_visible_window(&self) -> Result<bool, WindowsClientError> {
        if !cfg!(windows) {
            return Ok(false);
        }
        let root = self.runner_root();
        let output = self.run(&format!(
            "$root=[IO.Path]::GetFullPath('{root}').TrimEnd('\\'); \
             $p=Get-Process -Name torca_app -ErrorAction SilentlyContinue | \
             Where-Object {{ $_.Path -and $_.Path.StartsWith($root,[StringComparison]::OrdinalIgnoreCase) -and $_.MainWindowHandle -ne 0 }} | \
             Select-Object -First 1; \
             if ($p) {{ try {{ $shell=New-Object -ComObject WScript.Shell; [void]$shell.AppActivate($p.Id) }} catch {{}}; 'VISIBLE' }}"
        ))?;
        Ok(output.success && output.text.contains("VISIBLE"))
    }

    fn runner_root(&self) -> String {
        self.paths
            .repo_root
            .join("apps/client/flutter/build/windows/x64/runner")
            .display()
            .to_string()
            .replace('\'', "''")
    }

    fn run(&self, script: &str) -> Result<crate::process::CommandOutput, WindowsClientError> {
        self.runner
            .run(&CommandSpec {
                program: "powershell".into(),
                arguments: vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    script.into(),
                ],
                working_directory: self.paths.repo_root.clone(),
                timeout: Duration::from_secs(30),
                environment: BTreeMap::new(),
            })
            .map_err(WindowsClientError::Process)
    }
}

#[derive(Debug, Error)]
pub enum WindowsClientError {
    #[error("workspace Windows client command failed: {0}")]
    Command(String),
    #[error("workspace Windows client process error: {0}")]
    Process(#[from] ProcessError),
}
