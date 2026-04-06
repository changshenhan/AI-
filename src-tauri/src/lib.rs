mod commands;
mod db;
mod llm;
mod llm_stream;
mod metrics;
mod nlp_layout;
mod scheduler;
mod secure_settings;
mod summary_service;
mod tools;

use db::open_and_migrate;
use summary_service::catchup_summaries;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db = open_and_migrate(app.handle())?;
            app.manage(db.clone());
            catchup_summaries(app.handle(), &db);
            let handle = app.handle().clone();
            let db_bg = db.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    scheduler::scheduler_tick(&handle, &db_bg);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            llm::llm_save_settings,
            llm::llm_load_settings,
            llm::llm_clear_settings,
            llm::llm_test,
            llm::llm_chat,
            llm::llm_chat_stream,
            commands::time_now_iso,
            commands::calendar_list_day,
            commands::calendar_month_overview,
            commands::nlp_apply_plan,
            commands::complete_plan_item,
            commands::skip_plan_item,
            commands::list_summaries,
            commands::export_summaries_markdown,
            commands::system_timezone,
            commands::list_analytics_snapshot,
            commands::engine_run_tool,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
