#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod state;
use std::{path::PathBuf, sync::Arc};
use tauri::Manager;
fn main() {
    let executable = std::env::current_exe().expect("executable");
    let base = executable.parent().expect("directory");
    for relative in ["WebView2", "resources/WebView2"] {
        let runtime = base.join(relative);
        if runtime.join("msedgewebview2.exe").is_file() {
            std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", &runtime);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("icacls.exe")
                    .arg(&runtime)
                    .args([
                        "/grant",
                        "*S-1-15-2-2:(OI)(CI)(RX)",
                        "*S-1-15-2-1:(OI)(CI)(RX)",
                    ])
                    .creation_flags(0x08000000)
                    .output();
            }
            break;
        }
    }
    let portable = base.join("portable.flag").exists();
    let explicit = std::env::var_os("SQL_KILLER_DATA_DIR").map(PathBuf::from);
    let portable_path = if portable {
        Some(base.join("data"))
    } else {
        None
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let directory = explicit
                .or(portable_path)
                .unwrap_or(app.path().app_local_data_dir()?);
            let state = match state::AppState::new(directory) {
                Ok(state) => state,
                Err(error) => {
                    let message = format!(
                        "默认数据目录无法使用：{error}\n请选择一个可写文件夹存放 SQL-Killer 数据。"
                    );
                    rfd::MessageDialog::new()
                        .set_title("选择数据位置")
                        .set_description(&message)
                        .set_level(rfd::MessageLevel::Warning)
                        .show();
                    let selected = rfd::FileDialog::new()
                        .set_title("SQL-Killer 数据文件夹")
                        .pick_folder()
                        .ok_or_else(|| std::io::Error::other("没有选择数据目录"))?;
                    state::AppState::new(selected).map_err(std::io::Error::other)?
                }
            };
            app.manage(Arc::new(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::rpc])
        .run(tauri::generate_context!())
        .expect("SQL-Killer 启动失败：请检查数据目录写权限和 WebView2 运行时");
}
