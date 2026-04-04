use super::DB_FILENAME;

pub fn cmd_watch(debounce: u64) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let db_path = root.join(DB_FILENAME);
    if !db_path.exists() {
        anyhow::bail!("No index found. Run `ariadne index` first.");
    }
    let config = crate::config::repo::load(&root)?;
    crate::watch::watch_and_reindex(&root, &db_path, debounce, &config)
}
