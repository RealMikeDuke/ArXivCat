pub mod commands;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(CancelState::new())
        .invoke_handler(tauri::generate_handler![
            extract_paper,
            get_paper_list,
            open_workspace,
            load_paper,
            save_note,
            strip_comments,
            scan_pdfs,
            download_all,
            start_chat,
            cancel_chat,
            build_description,
            get_token_status,
            set_token,
            validate_token,
            get_chat_sessions,
            save_chat_session_data,
            rename_chat_session_data,
            delete_chat_session_data,
            get_last_workspace,
            open_paper_folder,
            open_paper_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
