// Choices offered by the log selector ("add panel" / "replace panel").

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::SelectionItem;
use crate::tui::log_manager::LogManager;
use crate::tui::state::AppState;

/// Logs of running apps not already open, grouped by project then app.
pub fn selector_candidates(
    state: &AppState,
    log_manager: &LogManager,
    already_open: &[PathBuf],
) -> Vec<SelectionItem> {
    // project -> app -> (path, label)
    let mut grouped: BTreeMap<String, BTreeMap<String, Vec<(PathBuf, String)>>> = BTreeMap::new();

    for project in &state.projects {
        for app in project.apps.iter().filter(|app| app.status.is_running()) {
            let Ok(logs) = log_manager.list_logs_for_app(&app.project, &app.name) else {
                continue;
            };
            for log in logs
                .into_iter()
                .filter(|log| !already_open.contains(&log.path))
            {
                let label = LogManager::format_log_label(&log.name);
                grouped
                    .entry(app.project.clone())
                    .or_default()
                    .entry(app.name.clone())
                    .or_default()
                    .push((log.path, label));
            }
        }
    }

    let mut candidates = Vec::new();
    for (project, apps) in grouped {
        candidates.push(SelectionItem::Header(project));
        for (app_name, logs) in apps {
            candidates.push(SelectionItem::SubHeader(app_name));
            candidates.extend(
                logs.into_iter()
                    .map(|(path, label)| SelectionItem::Option { label, path }),
            );
        }
    }
    candidates
}
