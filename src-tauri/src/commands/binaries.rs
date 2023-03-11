#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  process::Command,
};

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
  config::LauncherConfig,
  util::file::{create_dir, overwrite_dir, read_last_lines_from_file},
};

use super::CommandError;

fn bin_ext(filename: &str) -> String {
  if cfg!(windows) {
    return format!("{}.exe", filename);
  }
  return filename.to_string();
}

struct CommonConfigData {
  install_path: std::path::PathBuf,
  active_version: String,
  active_version_folder: String,
}

fn common_prelude(
  config: &tokio::sync::MutexGuard<LauncherConfig>,
) -> Result<CommonConfigData, CommandError> {
  let install_path = match &config.installation_dir {
    None => {
      return Err(CommandError::BinaryExecution(format!(
        "No installation directory set, can't perform operation"
      )))
    }
    Some(path) => Path::new(path),
  };

  let active_version = config
    .active_version
    .as_ref()
    .ok_or(CommandError::BinaryExecution(format!(
      "No active version set, can't perform operation"
    )))?;

  let active_version_folder =
    config
      .active_version_folder
      .as_ref()
      .ok_or(CommandError::BinaryExecution(format!(
        "No active version folder set, can't perform operation"
      )))?;

  Ok(CommonConfigData {
    install_path: install_path.to_path_buf(),
    active_version: active_version.clone(),
    active_version_folder: active_version_folder.clone(),
  })
}

#[derive(Debug, Serialize, Deserialize)]
struct LauncherErrorCode {
  msg: String,
}

fn get_error_codes(
  config: &CommonConfigData,
  game_name: &String,
) -> HashMap<i32, LauncherErrorCode> {
  let json_file = config
    .install_path
    .join("active")
    .join(game_name)
    .join("data")
    .join("launcher")
    .join("error-code-metadata.json");
  if !json_file.exists() {
    warn!("couldn't locate error code file at {}", json_file.display());
    return HashMap::new();
  } else {
    let file_contents = match std::fs::read_to_string(&json_file) {
      Ok(content) => content,
      Err(_err) => {
        warn!("couldn't read error code file at {}", &json_file.display());
        return HashMap::new();
      }
    };
    let json: Value = match serde_json::from_str(&file_contents) {
      Ok(json) => json,
      Err(_err) => {
        warn!("couldn't parse error code file at {}", &json_file.display());
        return HashMap::new();
      }
    };

    if let Value::Object(map) = json {
      let mut result: HashMap<i32, LauncherErrorCode> = HashMap::new();
      for (key, value) in map {
        let Ok(error_code) = serde_json::from_value(value) else {
          continue;
        };
        let Ok(code) = key.parse::<i32>() else {
          continue;
        };
        result.insert(code, error_code);
      }
      return result;
    } else {
      warn!(
        "couldn't convert error code file at {}",
        &json_file.display()
      );
      return HashMap::new();
    }
  }
}

fn copy_data_dir(config_info: &CommonConfigData, game_name: &String) -> Result<(), CommandError> {
  let src_dir = config_info
    .install_path
    .join("versions")
    .join(&config_info.active_version_folder)
    .join(&config_info.active_version)
    .join("data");

  let dst_dir = config_info
    .install_path
    .join("active")
    .join(&game_name)
    .join("data");

  info!("Copying {} into {}", src_dir.display(), dst_dir.display());

  overwrite_dir(&src_dir, &dst_dir).map_err(|err| {
    CommandError::Installation(format!(
      "Unable to copy data directory: '{}'",
      err.to_string()
    ))
  })?;
  Ok(())
}

fn get_data_dir(
  config_info: &CommonConfigData,
  game_name: &String,
  copy_if_needed: bool,
) -> Result<PathBuf, CommandError> {
  let data_folder = config_info
    .install_path
    .join("active")
    .join(game_name)
    .join("data");
  if !data_folder.exists() {
    if copy_if_needed {
      copy_data_dir(&config_info, &game_name)?;
    } else {
      return Err(CommandError::BinaryExecution(format!(
        "Could not locate relevant data directory '{}', can't perform operation",
        data_folder.to_string_lossy()
      )));
    }
  }
  Ok(data_folder)
}

struct ExecutableLocation {
  executable_dir: PathBuf,
  executable_path: PathBuf,
}

fn get_exec_location(
  config_info: &CommonConfigData,
  executable_name: &str,
) -> Result<ExecutableLocation, CommandError> {
  let exec_dir = config_info
    .install_path
    .join("versions")
    .join(&config_info.active_version_folder)
    .join(&config_info.active_version);
  let exec_path = exec_dir.join(bin_ext(executable_name));
  if !exec_path.exists() {
    return Err(CommandError::BinaryExecution(format!(
      "Could not find the required binary '{}', can't perform operation",
      exec_path.to_string_lossy()
    )));
  }
  Ok(ExecutableLocation {
    executable_dir: exec_dir,
    executable_path: exec_path,
  })
}

fn create_log_file(
  app_handle: &tauri::AppHandle,
  name: &str,
  append: bool,
) -> Result<std::fs::File, CommandError> {
  let log_path = &match app_handle.path_resolver().app_log_dir() {
    None => {
      return Err(CommandError::Installation(format!(
        "Could not determine path to save installation logs"
      )))
    }
    Some(path) => path.clone(),
  };
  create_dir(&log_path)?;
  let mut file_options = std::fs::OpenOptions::new();
  file_options.create(true);
  if append {
    file_options.append(true);
  } else {
    file_options.write(true).truncate(true);
  }
  let file = file_options.open(log_path.join(name))?;
  Ok(file)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStepOutput {
  pub success: bool,
  pub msg: Option<String>,
}

#[tauri::command]
pub async fn update_data_directory(
  config: tauri::State<'_, tokio::sync::Mutex<LauncherConfig>>,
  game_name: String,
) -> Result<InstallStepOutput, CommandError> {
  let config_lock = config.lock().await;
  let config_info = common_prelude(&config_lock)?;

  copy_data_dir(&config_info, &game_name)?;

  Ok(InstallStepOutput {
    success: true,
    msg: None,
  })
}

#[tauri::command]
pub async fn get_end_of_logs(app_handle: tauri::AppHandle) -> Result<String, CommandError> {
  Ok(read_last_lines_from_file(
    &app_handle
      .path_resolver()
      .app_log_dir()
      .unwrap()
      .join("extractor.log"),
    250,
  )?)
}

#[tauri::command]
pub async fn extract_and_validate_iso(
  config: tauri::State<'_, tokio::sync::Mutex<LauncherConfig>>,
  app_handle: tauri::AppHandle,
  path_to_iso: String,
  game_name: String,
) -> Result<InstallStepOutput, CommandError> {
  let config_lock = config.lock().await;
  let config_info = common_prelude(&config_lock)?;

  let data_folder = get_data_dir(&config_info, &game_name, true)?;
  let exec_info = get_exec_location(&config_info, "extractor")?;

  let mut args = vec![
    path_to_iso.clone(),
    "--extract".to_string(),
    "--validate".to_string(),
    "--proj-path".to_string(),
    data_folder.to_string_lossy().into_owned(),
  ];
  if Path::new(&path_to_iso.clone()).is_dir() {
    args.push("--folder".to_string());
  }

  // This is the first install step, reset the file
  let log_file = create_log_file(&app_handle, "extractor.log", false)?;

  let mut command = Command::new(exec_info.executable_path);
  command
    .args(args)
    .current_dir(exec_info.executable_dir)
    .stdout(log_file.try_clone()?)
    .stderr(log_file.try_clone()?);
  #[cfg(windows)]
  {
    command.creation_flags(0x08000000);
  }
  let output = command.output()?;
  match output.status.code() {
    Some(code) => {
      if code == 0 {
        return Ok(InstallStepOutput {
          success: true,
          msg: None,
        });
      }
      let error_code_map = get_error_codes(&config_info, &game_name);
      let default_error = LauncherErrorCode {
        msg: format!("Unexpected error occured with code {}", code).to_owned(),
      };
      let message = error_code_map.get(&code).unwrap_or(&default_error);
      Ok(InstallStepOutput {
        success: false,
        msg: Some(message.msg.clone()),
      })
    }
    None => Ok(InstallStepOutput {
      success: false,
      msg: Some("Unexpected error occurred".to_owned()),
    }),
  }
}

#[tauri::command]
pub async fn run_decompiler(
  config: tauri::State<'_, tokio::sync::Mutex<LauncherConfig>>,
  app_handle: tauri::AppHandle,
  path_to_iso: String,
  game_name: String,
  truncate_logs: bool,
) -> Result<InstallStepOutput, CommandError> {
  let config_lock = config.lock().await;
  let config_info = common_prelude(&config_lock)?;

  let data_folder = get_data_dir(&config_info, &game_name, false)?;
  let exec_info = get_exec_location(&config_info, "extractor")?;

  let mut source_path = path_to_iso;
  if source_path.is_empty() {
    source_path = data_folder
      .join("iso_data")
      .join(&game_name)
      .to_string_lossy()
      .to_string();
  }

  let log_file = create_log_file(&app_handle, "extractor.log", !truncate_logs)?;
  let mut command = Command::new(exec_info.executable_path);
  command
    .args([
      source_path,
      "--decompile".to_string(),
      "--proj-path".to_string(),
      data_folder.to_string_lossy().into_owned(),
    ])
    .stdout(log_file.try_clone()?)
    .stderr(log_file)
    .current_dir(exec_info.executable_dir);
  #[cfg(windows)]
  {
    command.creation_flags(0x08000000);
  }
  let output = command.output()?;
  match output.status.code() {
    Some(code) => {
      if code == 0 {
        return Ok(InstallStepOutput {
          success: true,
          msg: None,
        });
      }
      let error_code_map = get_error_codes(&config_info, &game_name);
      let default_error = LauncherErrorCode {
        msg: format!("Unexpected error occured with code {}", code).to_owned(),
      };
      let message = error_code_map.get(&code).unwrap_or(&default_error);
      Ok(InstallStepOutput {
        success: false,
        msg: Some(message.msg.clone()),
      })
    }
    None => Ok(InstallStepOutput {
      success: false,
      msg: Some("Unexpected error occurred".to_owned()),
    }),
  }
}

#[tauri::command]
pub async fn run_compiler(
  config: tauri::State<'_, tokio::sync::Mutex<LauncherConfig>>,
  app_handle: tauri::AppHandle,
  path_to_iso: String,
  game_name: String,
  truncate_logs: bool,
) -> Result<InstallStepOutput, CommandError> {
  let config_lock = config.lock().await;
  let config_info = common_prelude(&config_lock)?;

  let data_folder = get_data_dir(&config_info, &game_name, false)?;
  let exec_info = get_exec_location(&config_info, "extractor")?;

  let mut source_path = path_to_iso;
  if source_path.is_empty() {
    source_path = data_folder
      .join("iso_data")
      .join(&game_name)
      .to_string_lossy()
      .to_string();
  }

  let log_file = create_log_file(&app_handle, "extractor.log", !truncate_logs)?;
  let mut command = Command::new(exec_info.executable_path);
  command
    .args([
      source_path,
      "--compile".to_string(),
      "--proj-path".to_string(),
      data_folder.to_string_lossy().into_owned(),
    ])
    .stdout(log_file.try_clone().unwrap())
    .stderr(log_file)
    .current_dir(exec_info.executable_dir);
  #[cfg(windows)]
  {
    command.creation_flags(0x08000000);
  }
  let output = command.output()?;
  match output.status.code() {
    Some(code) => {
      if code == 0 {
        return Ok(InstallStepOutput {
          success: true,
          msg: None,
        });
      }
      let error_code_map = get_error_codes(&config_info, &game_name);
      let default_error = LauncherErrorCode {
        msg: format!("Unexpected error occured with code {}", code).to_owned(),
      };
      let message = error_code_map.get(&code).unwrap_or(&default_error);
      Ok(InstallStepOutput {
        success: false,
        msg: Some(message.msg.clone()),
      })
    }
    None => Ok(InstallStepOutput {
      success: false,
      msg: Some("Unexpected error occurred".to_owned()),
    }),
  }
}

#[tauri::command]
pub async fn open_repl(
  config: tauri::State<'_, tokio::sync::Mutex<LauncherConfig>>,
  game_name: String,
) -> Result<(), CommandError> {
  // TODO - explore a linux option though this is very annoying because without doing a ton of research
  // we seem to have to handle various terminals.  Which honestly we should probably do on windows too
  //
  // So maybe we can make a menu where the user will specify what terminal to use / what launch-options to use
  let config_lock = config.lock().await;
  let config_info = common_prelude(&config_lock)?;

  let data_folder = get_data_dir(&config_info, &game_name, false)?;
  let exec_info = get_exec_location(&config_info, "goalc")?;
  let mut command = Command::new("cmd");
  command
    .args([
      "/K",
      "start",
      &bin_ext("goalc"),
      "--proj-path",
      &data_folder.to_string_lossy().into_owned(),
    ])
    .current_dir(exec_info.executable_dir);
  #[cfg(windows)]
  {
    command.creation_flags(0x08000000);
  }
  command.spawn()?;
  Ok(())
}

#[tauri::command]
pub async fn launch_game(
  config: tauri::State<'_, tokio::sync::Mutex<LauncherConfig>>,
  app_handle: tauri::AppHandle,
  game_name: String,
  in_debug: bool,
) -> Result<(), CommandError> {
  let config_lock = config.lock().await;
  let config_info = common_prelude(&config_lock)?;

  let data_folder = get_data_dir(&config_info, &game_name, false)?;
  let exec_info = get_exec_location(&config_info, "gk")?;

  let mut args = vec!["-boot".to_string(), "-fakeiso".to_string()];
  // NOTE - order unfortunately matters for gk args
  if in_debug {
    args.push("-debug".to_string());
  }
  args.push("-proj-path".to_string());
  args.push(data_folder.to_string_lossy().into_owned());
  let log_file = create_log_file(&app_handle, "game.log", false)?;
  let mut command = Command::new(exec_info.executable_path);
  command
    .args(args)
    .stdout(log_file.try_clone().unwrap())
    .stderr(log_file)
    .current_dir(exec_info.executable_dir);
  #[cfg(windows)]
  {
    command.creation_flags(0x08000000);
  }
  command.spawn()?;
  Ok(())
}