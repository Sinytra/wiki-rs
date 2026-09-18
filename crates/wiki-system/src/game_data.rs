use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{SystemError, SystemResult};
use crate::loader::{GameVersion, ITEMS_DIR, ITEM_MODELS_DIR, ModLoader, loader_for, parse_maven_versions};
use crate::util::{clean_dir_filtered, merge_json_bytes, merge_json_file};
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction, Set, TransactionTrait};
use tracing::{debug, error, info, warn};
use wiki_db::query;
use wiki_domain::BUILTIN_PROJECT_ID;
use wiki_domain::content::ResourceLocation;
use wiki_domain::util::LogErr;
use wiki_storage::format::{LegacyProjectFormat, ProjectFormat};
use wiki_storage::ingestor::Ingestor;
use wiki_storage::ingestor::issues::{IssueSink, LoggingIssueSink};
use wiki_storage::ingestor::metadata::INGESTOR_MOD_METADATA;
use wiki_storage::ingestor::tags::INGESTOR_MOD_TAGS;

const LAUNCHER_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

const EXTRACT_ASSET_DIRS: &[&str] = &[
    "assets/minecraft/lang",
    ITEMS_DIR,
    ITEM_MODELS_DIR
];

const LANG_DIR: &str = "assets/minecraft/lang";
const KEEP_DIRS: &[&str] = &[LANG_DIR];

#[async_trait]
pub trait GameDataSource: Send + Sync {
    async fn get_lang(&self, lang: &str) -> Option<HashMap<String, String>>;
}

pub struct FileGameData {
    lang_dir: PathBuf,
}

impl FileGameData {
    pub fn new(game_root: impl Into<PathBuf>) -> Self {
        let lang_dir = game_root.into().join(LANG_DIR);
        Self { lang_dir }
    }

    fn lang_path(&self, lang: &str) -> PathBuf {
        self.lang_dir.join(format!("{lang}.json"))
    }
}

#[async_trait]
impl GameDataSource for FileGameData {
    async fn get_lang(&self, lang: &str) -> Option<HashMap<String, String>> {
        let path = self.lang_path(lang);
        read_lang_file(&path).await
    }
}

async fn read_lang_file(path: &Path) -> Option<HashMap<String, String>> {
    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            warn!("failed to read lang file {}: {e}", path.display());
            return None;
        }
    };

    match serde_json::from_slice(&bytes) {
        Ok(map) => Some(map),
        Err(e) => {
            warn!("failed to parse lang file {}: {e}", path.display());
            None
        }
    }
}

pub struct GameDataService {
    game_root: PathBuf,
    builtin_data_dir: PathBuf,
    http: reqwest::Client,
    db: DatabaseConnection,
}

impl GameDataService {
    pub fn new(
        game_root: impl Into<PathBuf>,
        builtin_data_dir: impl Into<PathBuf>,
        http: reqwest::Client,
        db: DatabaseConnection,
    ) -> Self {
        Self {
            game_root: game_root.into(),
            builtin_data_dir: builtin_data_dir.into(),
            http,
            db,
        }
    }

    pub fn game_root(&self) -> &Path {
        &self.game_root
    }

    pub async fn import_game_data(
        &self,
        game_version: Option<String>,
        update_loader: bool,
    ) -> SystemResult<()> {
        debug!("checking game data status");

        let version_manifest = self
            .resolve_game_version_manifest(game_version.as_deref())
            .await?;
        let used_game_version = GameVersion::parse(&version_manifest.version);
        let loader = loader_for(&used_game_version).ok_or_else(|| {
            SystemError::Internal(format!(
                "no mod loader supports game version {}",
                used_game_version.as_str()
            ))
        })?;
        let loader_version = self
            .resolve_loader_version(loader, &used_game_version)
            .await?;

        if let Some(existing) = self.get_existing_import(used_game_version.as_str()).await?
            && (!update_loader
                || (existing.loader == loader.id() && existing.loader_version == loader_version))
        {
            debug!("game data up to date, skipping");
            return Ok(());
        }

        info!(loader = loader.id(), version = %loader_version, "setting up game data");
        self.download_game_files(&version_manifest.data, loader, &loader_version)
            .await?;

        self.copy_builtin_data().await?;

        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to begin transaction: {e}")))?;

        match self
            .import_game_data_inner(&tx, used_game_version.as_str(), loader, &loader_version)
            .await
        {
            Ok(()) => tx.commit().await.map_err(|e| {
                SystemError::Internal(format!("failed to commit game data import: {e}"))
            })?,
            Err(e) => {
                if let Err(rb) = tx.rollback().await {
                    error!("failed to roll back game data import: {rb}");
                }
                return Err(e);
            }
        }

        self.clean_game_dir().await.log_err("cleaning game dir");

        info!("game data setup complete");
        Ok(())
    }

    async fn import_game_data_inner(
        &self,
        tx: &DatabaseTransaction,
        game_version: &str,
        loader: &dyn ModLoader,
        loader_version: &str,
    ) -> SystemResult<()> {
        let version_id = self.get_or_create_version(tx).await?;
        self.ingest_game_data(tx, version_id).await?;
        self.register_items(tx, loader, version_id).await?;
        self.record_import(tx, game_version, loader, loader_version)
            .await?;
        Ok(())
    }

    async fn ingest_game_data(
        &self,
        tx: &DatabaseTransaction,
        version_id: i64,
    ) -> SystemResult<()> {
        info!("ingesting game data");

        let format: Arc<dyn ProjectFormat> = Arc::new(
            LegacyProjectFormat::new(self.game_root.clone())
                .with_data_root(self.game_root.join("data")),
        );
        let issues = Arc::new(LoggingIssueSink::new());

        let ingestor = Ingestor::builder()
            .project_id(BUILTIN_PROJECT_ID)
            .modid(BUILTIN_PROJECT_ID)
            .version_id(version_id)
            // For builtin ingestion the project version *is* the builtin version.
            .builtin_version_id(version_id)
            .format(format)
            .issues(Arc::clone(&issues) as Arc<dyn IssueSink>)
            .enabled_modules([INGESTOR_MOD_TAGS, INGESTOR_MOD_METADATA])
            .build()?;

        ingestor.run_in_tx(tx).await?;

        if issues.has_errors() {
            return Err(SystemError::Internal(
                "errors encountered during game data ingestion".into(),
            ));
        }

        info!("game data ingestion successful");
        Ok(())
    }

    async fn resolve_game_version_manifest(
        &self,
        game_version: Option<&str>,
    ) -> SystemResult<VersionManifest> {
        debug!("fetching launcher manifest");
        let manifest: serde_json::Value = self
            .http
            .get(LAUNCHER_MANIFEST_URL)
            .send()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to fetch launcher manifest: {e}")))?
            .json()
            .await
            .map_err(|e| SystemError::Internal(format!("invalid launcher manifest JSON: {e}")))?;

        let target = match game_version {
            Some(version) => version.to_owned(),
            None => {
                let latest_release = manifest["latest"]["release"]
                    .as_str()
                    .ok_or_else(|| {
                        SystemError::Internal("missing latest.release in manifest".into())
                    })?
                    .to_owned();

                debug!(version = %latest_release, "found latest release");
                latest_release
            }
        };

        let versions = manifest["versions"]
            .as_array()
            .ok_or_else(|| SystemError::Internal("missing versions array".into()))?;

        let entry = versions
            .iter()
            .find(|version| version["id"].as_str() == Some(target.as_str()))
            .ok_or_else(|| {
                SystemError::Internal(format!("version {target} not found in launcher manifest"))
            })?;

        let url = entry["url"]
            .as_str()
            .ok_or_else(|| SystemError::Internal("missing version url".into()))?;

        debug!(version = %target, "fetching version manifest");
        let data: serde_json::Value = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to fetch version manifest: {e}")))?
            .json()
            .await
            .map_err(|e| SystemError::Internal(format!("invalid version manifest JSON: {e}")))?;

        Ok(VersionManifest {
            version: target,
            data,
        })
    }

    async fn resolve_loader_version(
        &self,
        loader: &dyn ModLoader,
        game_version: &GameVersion,
    ) -> SystemResult<String> {
        let id = loader.id();
        let body = self
            .http
            .get(loader.metadata_url())
            .send()
            .await
            .map_err(|e| {
                SystemError::Internal(format!("failed to fetch {id} maven metadata: {e}"))
            })?
            .text()
            .await
            .map_err(|e| {
                SystemError::Internal(format!("failed to read {id} metadata body: {e}"))
            })?;

        let versions = parse_maven_versions(&body)?;
        let selected = loader
            .select_version(&versions, game_version)
            .ok_or_else(|| {
                SystemError::Internal(format!(
                    "no {id} release found for game version {}",
                    game_version.as_str()
                ))
            })?;

        debug!(loader = id, version = %selected, game_version = game_version.as_str(), "resolved loader version");
        Ok(selected)
    }

    async fn download_game_files(
        &self,
        version_manifest: &serde_json::Value,
        loader: &dyn ModLoader,
        loader_version: &str,
    ) -> SystemResult<()> {
        let game_dir = &self.game_root;

        self.clean_game_dir().await.log_err("cleaning game dir");

        info!("downloading game files");
        tokio::fs::create_dir_all(game_dir)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to create game dir: {e}")))?;

        // Download asset index
        debug!("fetching asset index");
        let asset_index_url = version_manifest["assetIndex"]["url"]
            .as_str()
            .ok_or_else(|| SystemError::Internal("missing assetIndex.url".into()))?;

        let asset_index: serde_json::Value = self
            .http
            .get(asset_index_url)
            .send()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to fetch asset index: {e}")))?
            .json()
            .await
            .map_err(|e| SystemError::Internal(format!("invalid asset index JSON: {e}")))?;

        // Download additional language files
        debug!("downloading additional language files");
        let lang_dir = game_dir.join(LANG_DIR);
        tokio::fs::create_dir_all(&lang_dir)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to create lang dir: {e}")))?;
        self.download_language_files(&asset_index, &lang_dir)
            .await?;

        // Download client jar
        info!("downloading client");
        let client_url = version_manifest["downloads"]["client"]["url"]
            .as_str()
            .ok_or_else(|| SystemError::Internal("missing downloads.client.url".into()))?;
        let client_dest = game_dir.join("client.jar");
        self.download_file(client_url, &client_dest).await?;

        // Extract client data
        info!("extracting client data");
        let extract_dirs = extract_dirs();
        extract_zip(&client_dest, game_dir, &extract_dirs, KEEP_DIRS)?;
        tokio::fs::remove_file(&client_dest).await.ok();

        // Download loader jar
        info!(loader = loader.id(), "downloading loader jar");
        let loader_url = loader.jar_url(loader_version);
        let loader_dest = game_dir.join(format!("{}.jar", loader.id()));
        self.download_file(&loader_url, &loader_dest).await?;

        // Extract loader data
        info!(loader = loader.id(), "extracting loader jar");
        extract_zip(&loader_dest, game_dir, &extract_dirs, KEEP_DIRS)?;
        tokio::fs::remove_file(&loader_dest).await.ok();

        debug!("game data download successful");
        Ok(())
    }

    async fn copy_builtin_data(&self) -> SystemResult<()> {
        if !self.builtin_data_dir.exists() {
            warn!(
                path = %self.builtin_data_dir.display(),
                "builtin data dir not found, skipping"
            );
            return Ok(());
        }

        debug!(
            from = %self.builtin_data_dir.display(),
            to = %self.game_root.display(),
            "copying builtin data into game root"
        );
        let src = self.builtin_data_dir.clone();
        let dst = self.game_root.clone();
        tokio::task::spawn_blocking(move || copy_dir_contents(&src, &dst))
            .await
            .map_err(|e| SystemError::Internal(format!("copy_builtin_data join error: {e}")))?
            .map_err(|e| SystemError::Internal(format!("failed to copy builtin data: {e}")))?;

        Ok(())
    }

    async fn download_language_files(
        &self,
        asset_index: &serde_json::Value,
        lang_dir: &Path,
    ) -> SystemResult<()> {
        const LANG_FILE_PREFIX: &str = "minecraft/lang/";

        let objects = asset_index["objects"]
            .as_object()
            .ok_or_else(|| SystemError::Internal("missing objects in asset index".into()))?;

        let mut count = 0u32;
        let start = std::time::Instant::now();

        for (key, object) in objects {
            if let Some(file_name) = key.strip_prefix(LANG_FILE_PREFIX) {
                if !file_name.contains('_') {
                    continue;
                }

                let hash = object["hash"]
                    .as_str()
                    .ok_or_else(|| SystemError::Internal("missing hash in asset object".into()))?;
                let prefix = &hash[..2];
                let resource_url = format!("{RESOURCES_URL}/{prefix}/{hash}");
                let download_path = lang_dir.join(file_name);

                let bytes = self.fetch_bytes(&resource_url).await?;
                merge_json_file(&download_path, &bytes).await?;
                count += 1;
            }
        }

        let elapsed = start.elapsed();
        info!(
            count,
            elapsed_ms = elapsed.as_millis(),
            "downloaded language files"
        );
        Ok(())
    }

    async fn fetch_bytes(&self, url: &str) -> SystemResult<Vec<u8>> {
        self.http
            .get(url)
            .send()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to download {url}: {e}")))?
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| SystemError::Internal(format!("failed to read body from {url}: {e}")))
    }

    async fn download_file(&self, url: &str, dest: &Path) -> SystemResult<()> {
        let bytes = self.fetch_bytes(url).await?;

        tokio::fs::write(dest, &bytes).await.map_err(|e| {
            SystemError::Internal(format!("failed to write {}: {e}", dest.display()))
        })?;

        Ok(())
    }

    async fn register_items(&self, tx: &DatabaseTransaction, loader: &dyn ModLoader, version_id: i64) -> SystemResult<()> {
        let items_root = self.game_root.join(loader.items_listing_dir());
        if !items_root.exists() {
            debug!("no items directory found, skipping registration");
            return Ok(());
        }

        debug!("registering game items from asset files");
        let mut entries = tokio::fs::read_dir(&items_root)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to read items dir: {e}")))?;

        let mut count = 0u32;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to read dir entry: {e}")))?
        {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if let Some(base) = name.strip_suffix(".json") {
                let item_id = format!("minecraft:{base}");
                query::ingestor::add_project_item(tx, version_id, version_id, &item_id)
                    .await
                    .map_err(|e| {
                        SystemError::Internal(format!(
                            "failed to register game item {item_id}: {e}"
                        ))
                    })?;
                count += 1;
            }
        }

        debug!(count, "registered game items");
        Ok(())
    }

    async fn get_existing_import(
        &self,
        game_version: &str,
    ) -> SystemResult<Option<wiki_db::entity::data_import::Model>> {
        match query::data_import::get_data_import(&self.db, game_version).await {
            Ok(model) => Ok(Some(model)),
            Err(wiki_db::error::DbError::NotFound) => Ok(None),
            Err(e) => Err(SystemError::Internal(format!(
                "failed to query data import: {e}"
            ))),
        }
    }

    async fn record_import(
        &self,
        tx: &DatabaseTransaction,
        game_version: &str,
        loader: &dyn ModLoader,
        loader_version: &str,
    ) -> SystemResult<()> {
        use sea_orm::EntityTrait;
        use wiki_db::entity::data_import;

        let model = data_import::ActiveModel {
            game_version: Set(game_version.to_owned()),
            loader: Set(loader.id().to_owned()),
            loader_version: Set(loader_version.to_owned()),
            user_id: Set(None),
            created_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };

        data_import::Entity::insert(model)
            .exec(tx)
            .await
            .map_err(|e| {
                SystemError::Internal(format!("failed to insert data import record: {e}"))
            })?;

        Ok(())
    }

    async fn get_or_create_version(&self, tx: &DatabaseTransaction) -> SystemResult<i64> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        use wiki_db::entity::project_version;

        let existing = project_version::Entity::find()
            .filter(project_version::Column::ProjectId.eq(BUILTIN_PROJECT_ID))
            .one(tx)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to query version: {e}")))?;

        if let Some(v) = existing {
            return Ok(v.id);
        }

        let model = project_version::ActiveModel {
            project_id: Set(BUILTIN_PROJECT_ID.to_owned()),
            branch: Set("".to_owned()),
            ..Default::default()
        };

        let result = project_version::Entity::insert(model)
            .exec(tx)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to create version: {e}")))?;

        Ok(result.last_insert_id)
    }

    async fn clean_game_dir(&self) -> SystemResult<()> {
        let game_dir = self.game_root.clone();
        if game_dir.exists() {
            tokio::task::spawn_blocking(move || clean_dir_filtered(&game_dir, KEEP_DIRS))
                .await
                .map_err(|e| SystemError::Internal(format!("clean game dir join error: {e}")))?
                .map_err(|e| SystemError::Internal(format!("failed to clean game dir: {e}")))?;
        }
        Ok(())
    }
}

struct VersionManifest {
    version: String,
    data: serde_json::Value,
}

fn extract_data_dirs(namespace: &str) -> [String; 2] {
    [
        format!("data/{namespace}/recipe"),
        format!("data/{namespace}/tags/item"),
    ]
}

fn extract_dirs() -> Vec<String> {
    let namespaced = ResourceLocation::BUILTIN_NAMESPACES
        .iter()
        .flat_map(|namespace| extract_data_dirs(namespace));

    EXTRACT_ASSET_DIRS
        .iter()
        .map(|dir| (*dir).to_owned())
        .chain(namespaced)
        .collect()
}

fn should_extract(path: &str, filter: &[String]) -> bool {
    if path.ends_with('/') {
        return false;
    }
    if filter.is_empty() {
        return true;
    }
    filter.iter().any(|prefix| path.starts_with(prefix))
}

fn copy_dir_contents(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_contents(&entry.path(), &target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn extract_zip(
    archive_path: &Path,
    dest_dir: &Path,
    filter: &[String],
    merge_dirs: &[&str],
) -> SystemResult<()> {
    let file = std::fs::File::open(archive_path).map_err(|e| {
        SystemError::Internal(format!(
            "cannot open zip file {}: {e}",
            archive_path.display()
        ))
    })?;

    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        SystemError::Internal(format!(
            "cannot read zip archive {}: {e}",
            archive_path.display()
        ))
    })?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| SystemError::Internal(format!("failed to read zip entry {i}: {e}")))?;

        let Some(name) = entry.enclosed_name().map(|p| p.to_owned()) else {
            continue;
        };

        let name_str = name.to_string_lossy();
        if !should_extract(&name_str, filter) {
            continue;
        }

        let out_path = dest_dir.join(&name);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| {
                SystemError::Internal(format!("failed to create dir {}: {e}", out_path.display()))
            })?;

            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SystemError::Internal(format!("failed to create dir {}: {e}", parent.display()))
            })?;
        }

        if merge_dirs.iter().any(|prefix| name_str.starts_with(prefix))
            && let Ok(existing) = std::fs::read(&out_path)
        {
            let mut incoming = Vec::new();
            std::io::copy(&mut entry, &mut incoming)
                .map_err(|e| SystemError::Internal(format!("failed to extract {name_str}: {e}")))?;

            let merged = merge_json_bytes(&out_path, Some(&existing), &incoming);
            std::fs::write(&out_path, &merged).map_err(|e| {
                SystemError::Internal(format!("failed to write {}: {e}", out_path.display()))
            })?;

            continue;
        }

        let mut out_file = std::fs::File::create(&out_path).map_err(|e| {
            SystemError::Internal(format!(
                "failed to create output file {}: {e}",
                out_path.display()
            ))
        })?;

        std::io::copy(&mut entry, &mut out_file)
            .map_err(|e| SystemError::Internal(format!("failed to extract {}: {e}", name_str)))?;
    }

    Ok(())
}
