// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::HashMap, path::PathBuf};
use std::fs;
use std::process::Command;
use serde::Serializer;
use serde_json::Result as SerdeResult;
use tauri::{Manager, PhysicalSize};
mod database;
mod mods;
mod files;

#[derive(serde::Serialize)]
#[serde(rename_all(serialize = "PascalCase"))]
enum ErrorCode {
    IncorrectPath,
    FailedToCreateModsFolder,
    FailedToSaveModsList
}

#[derive(serde::Serialize)]
struct InitialInfo {
    enabled_mods : HashMap<String, bool>,
    mods : HashMap<String, mods::ModInfo>,
    database: Vec<database::ModDatabaseInfo>,
    #[serde(serialize_with = "serialize_mod_info_option")]
    winch_mod_info : Option<mods::ModInfo>
}

fn serialize_mod_info_option<S>(maybe_mod_info : &Option<mods::ModInfo>, serializer : S) -> Result<S::Ok, S::Error> where S : Serializer {
    match maybe_mod_info {
        Some(mod_info) => serializer.serialize_some(mod_info),
        None => serializer.serialize_none()
    }
}

#[derive(serde::Serialize)]
struct InitialInfoError {
    error_code : ErrorCode,
    message : String
}

#[tauri::command]
fn load_dredge_path() -> Result<String, String> {
    // Check the metadata file for the path to dredge
    let file: String = format!("{}/data.txt", files::get_local_dir()?);
    let dredge_path = match fs::read_to_string(&file) {
        Ok(v) => v,
        Err(_) => match dredge_path_changed("".to_string()) 
        {
            Ok(_) => "".to_string(),
            Err(err) => return Err(err.to_string())
        }
    };
    Ok(dredge_path)
}

#[tauri::command]
fn load(dredge_path : String) -> Result<InitialInfo, InitialInfoError> {
    // Validate that the folder path is correct
    if !fs::metadata(format!("{}/DREDGE.exe", dredge_path)).is_ok() {
        return Err(InitialInfoError { error_code : ErrorCode::IncorrectPath, message : format!("Couldn't find DREDGE.exe at [{}]", dredge_path) });
    }

    // Search for installed mods
    let mods_dir_path = format!("{}/Mods", dredge_path);
    if !fs::metadata(&mods_dir_path).is_ok() {
        match fs::create_dir(&mods_dir_path) {
            Ok(_) => (),
            Err(e) => return Err(InitialInfoError {
                error_code: ErrorCode::FailedToCreateModsFolder, 
                message: format!("Couldn't create mods folder at [{}] - {}", mods_dir_path, e) 
            })
        }
    }

    // If it fails to find/read the file this will just be an empty dictionary and we'll write a new file
    let enabled_mods_path = files::get_enabled_mods_path(&dredge_path);
    let mut enabled_mods = check_enabled_mods(enabled_mods_path.clone());

    // Check all installed mods to see if any are missing
    let mut mods: HashMap<String, mods::ModInfo> = HashMap::new();
    let mut update_enabled_mods_list_flag = false;

    let mut check_enabled_mod_meta_json = |file_path : String| {
        let mod_info_res = mods::load_mod_info(file_path);
    
        match mod_info_res {
            Ok(mod_info) => {
                let key = mod_info.mod_guid.clone();
                mods.insert(key.clone(), mod_info);
                // Check that the mod is included in the file
                if !enabled_mods.contains_key(&key) {
                    enabled_mods.insert(key, true);
                    update_enabled_mods_list_flag = true;
                }
            },
            Err(e) => println!("Failed to load mod {}", e)
        }
    };

    // Check Winch installed
    check_enabled_mod_meta_json(format!("{}/mod_meta.json", dredge_path));

    for entry in walkdir::WalkDir::new(&mods_dir_path) {
        let entry = entry.unwrap();
        let file_path : String = entry.path().display().to_string();

        if file_path.contains("mod_meta.json") {
            println!("Found mod: {}", entry.path().display());
            check_enabled_mod_meta_json(file_path)
        }
    }
    if update_enabled_mods_list_flag {
        match write_enabled_mods(enabled_mods.clone(), enabled_mods_path.clone()) {
            Ok(_) => (),
            Err(e) => 
            return Err(InitialInfoError { 
                error_code: ErrorCode::FailedToSaveModsList,
                message : format!("Couldn't save the mod list at [{}] - [{}]", enabled_mods_path, e).to_string() 
            })
        }
    }

    let database: Vec<database::ModDatabaseInfo> = database::access_database();

    // Get Winch mod info
    let winch_mod_meta_path = format!("{}/mod_meta.json", dredge_path);
    let winch_mod_info = match mods::load_mod_info(winch_mod_meta_path) {
        Ok(x) => Some(x),
        Err(_) => None
    };

    Ok(InitialInfo {enabled_mods, mods, database, winch_mod_info})
}

fn check_enabled_mods(enabled_mods_path : String) -> HashMap<String, bool> {
        // Load enabled/disabled mods    
        let enabled_mods_json_string = match fs::read_to_string(&enabled_mods_path) {
            Ok(v) => v,
            Err(e) => {
                println!("Couldn't read mod list json {}", e);
                return HashMap::new()
            }
        };
    
        let enabled_mods = match serde_json::from_str(&enabled_mods_json_string) as SerdeResult<HashMap<String, bool>> {
            Ok(v) => v,
            Err(_) => {
                println!("Couldn't parse mod list json");
                return HashMap::new()
            }
        };

        return enabled_mods;
}

#[tauri::command]
fn dredge_path_changed(path: String) -> Result<(), String> {
    println!("DREDGE folder path changed to: {}", path);

    let folder: String = files::get_local_dir()?;
    let file: String = format!("{}/data.txt", folder);
    if !fs::metadata(&folder).is_ok() {
        match fs::create_dir_all(&folder) {
            Ok(_) => (),
            Err(e) => return Err(format!("Failed to create DredgeModManager appdata directory - {}", e))
        };
    }

    match fs::write(&file, path) {
        Ok(_) => (),
        Err(e) => return Err(format!("Failed to write to manager meta data file at [{}] - {}", file, e))
    }

    println!("DREDGE folder path saved to: {}", file);

    Ok(())
}

#[tauri::command]
fn toggle_enabled_mod(mod_guid : String, enabled : bool, dredge_path : String) -> Result<(), String> {
    let enabled_mods_path = files::get_enabled_mods_path(&dredge_path);
    let file_contents = match fs::read_to_string(&enabled_mods_path) {
        Ok(x) => x,
        Err(e) => return Err(format!("Could not load mods json - {}", e))
    };

    let mut json = match serde_json::from_str(&file_contents) as SerdeResult<HashMap<String, bool>> {
        Ok(v) => v,
        Err(e) => return Err(format!("Could not parse mods json - {}", e))
    };

    json.insert(mod_guid, enabled);

    match write_enabled_mods(json, enabled_mods_path) {
        Ok(()) => (),
        Err(e) => return Err(format!("Could not update mods json - {}", e))
    };

    Ok(())
}

fn write_enabled_mods(json : HashMap<String, bool>, enabled_mods_path : String) -> Result<(), Box<dyn std::error::Error>> {
    let json_string = serde_json::to_string_pretty(&json)?;

    fs::write(&enabled_mods_path, json_string)?;

    Ok(())
}

#[tauri::command]
fn start_game(dredge_path : String) -> Result<(), String> {
    let exe = format!("{}/DREDGE.exe", dredge_path);
    match Command::new(exe).spawn() {
        Ok(_) => return Ok(()),
        Err(_) => return Err("Failed to start DREDGE.exe. Is the game directory correct?".to_string())
    }
}

#[tauri::command]
fn uninstall_mod(mod_meta_path : String) -> () {
    mods::uninstall_mod(mod_meta_path);
}

#[tauri::command]
fn install_mod(repo : String, download : String, dredge_folder : String) -> Result<(), String> {
    let unique_id: String = match mods::install_mod(repo, download, dredge_folder.to_string()) {
        Ok(s) => s,
        Err(error) => return Err(format!("Failed to install mod {}", error.to_string()))
    };

    // #12 newly installed mods should be enabled by default
   match toggle_enabled_mod(unique_id.to_string(), true, dredge_folder.to_string()) {
        Ok(_) => Ok(()),
        Err(err) => return Err(format!("Failed to enable mod {} after installing {}", unique_id.to_string(), err))
   }
}

#[tauri::command]
fn open_dir(path : String) -> Result<(), String> {
    let mut path_buf: PathBuf = PathBuf::from(path);

    if !path_buf.is_dir() {
        path_buf.pop();
    }

    let dir: String = path_buf.display().to_string();

    match open::that(dir) {
        Ok(_) => return Ok(()),
        Err(error) => return Err(format!("Couldn't open directory {}", error))
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            dredge_path_changed,
            load_dredge_path,
            load,
            toggle_enabled_mod,
            start_game,
            uninstall_mod,
            install_mod,
            open_dir
            ])
        .setup(|app| {
                let main_window: tauri::Window = app.get_window("main").unwrap();
                main_window.set_min_size(Some(tauri::Size::Physical(PhysicalSize {width: 640, height: 480}))).unwrap();
                Ok(())
            })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
