use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{SystemError, SystemResult};
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, Set, TransactionTrait};
use tracing::{debug, error, info, warn};
use wiki_db::query;
use wiki_domain::BUILTIN_PROJECT_ID;
use wiki_storage::format::{LegacyProjectFormat, ProjectFormat};
use wiki_storage::ingestor::Ingestor;
use wiki_storage::ingestor::issues::{IssueSink, LoggingIssueSink};
use wiki_storage::ingestor::tags::INGESTOR_MOD_TAGS;

const LAUNCHER_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

const NEOFORGE_MAVEN_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const NEOFORGE_URL_TEMPLATE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-universal.jar";

const EXTRACT_VANILLA_DIRS: &[&str] = &[
    "assets/minecraft/lang",
    "assets/minecraft/items",
    "data/minecraft/recipe",
    "data/minecraft/tags/item",
];

const EXTRACT_NEOFORGE_DIRS: &[&str] = &["data/c/recipe", "data/c/tags/item"];

#[async_trait]
pub trait GameDataSource: Send + Sync {
    async fn get_lang(&self, lang: &str) -> Option<HashMap<String, String>>;
}

pub struct FileGameData {
    lang_dir: PathBuf,
}

impl FileGameData {
    pub fn new(game_root: impl Into<PathBuf>) -> Self {
        let lang_dir = game_root.into().join("assets/minecraft/lang");
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
    http: reqwest::Client,
    db: DatabaseConnection,
}

impl GameDataService {
    pub fn new(
        game_root: impl Into<PathBuf>,
        http: reqwest::Client,
        db: DatabaseConnection,
    ) -> Self {
        Self {
            game_root: game_root.into(),
            http,
            db,
        }
    }

    pub fn game_root(&self) -> &Path {
        &self.game_root
    }

    // TODO transaction
    pub async fn import_game_data(&self, update_loader: bool) -> SystemResult<()> {
        debug!("checking game data status...");

        let version_manifest = self.resolve_latest_game_version_manifest().await?;
        let neoforge_version = self.get_latest_neoforge_version().await?;

        if let Some(existing) = self.get_existing_import(&version_manifest.version).await?
            && (!update_loader || existing.loader_version == neoforge_version)
        {
            debug!("game data up to date, skipping");
            return Ok(());
        }

        info!("setting up game data");
        self.download_game_files(&version_manifest.data, &neoforge_version)
            .await?;

        let version_id = self.get_or_create_version().await?;
        self.ingest_game_data(version_id).await?;
        self.register_items(version_id).await?;

        self.record_import(&version_manifest.version, &neoforge_version)
            .await?;

        info!("game data setup complete");
        Ok(())
    }

    async fn ingest_game_data(&self, version_id: i64) -> SystemResult<()> {
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
            .enabled_modules([INGESTOR_MOD_TAGS])
            .build()?;

        ingestor.run(&self.db).await?;

        if issues.has_errors() {
            return Err(SystemError::Internal(
                "errors encountered during game data ingestion".into(),
            ));
        }

        info!("game data ingestion successful");
        Ok(())
    }

    async fn resolve_latest_game_version_manifest(&self) -> SystemResult<VersionManifest> {
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

        let latest_release = manifest["latest"]["release"]
            .as_str()
            .ok_or_else(|| SystemError::Internal("missing latest.release in manifest".into()))?
            .to_owned();

        debug!(version = %latest_release, "found latest release");

        let versions = manifest["versions"]
            .as_array()
            .ok_or_else(|| SystemError::Internal("missing versions array".into()))?;

        for version in versions {
            if version["id"].as_str() == Some(latest_release.as_ref()) {
                let url = version["url"]
                    .as_str()
                    .ok_or_else(|| SystemError::Internal("missing version url".into()))?;

                debug!("fetching version manifest");
                let data: serde_json::Value = self
                    .http
                    .get(url)
                    .send()
                    .await
                    .map_err(|e| {
                        SystemError::Internal(format!("failed to fetch version manifest: {e}"))
                    })?
                    .json()
                    .await
                    .map_err(|e| {
                        SystemError::Internal(format!("invalid version manifest JSON: {e}"))
                    })?;

                return Ok(VersionManifest {
                    version: latest_release,
                    data,
                });
            }
        }

        Err(SystemError::Internal(format!(
            "version {latest_release} not found in launcher manifest"
        )))
    }

    async fn get_latest_neoforge_version(&self) -> SystemResult<String> {
        let body = self
            .http
            .get(NEOFORGE_MAVEN_METADATA)
            .send()
            .await
            .map_err(|e| {
                SystemError::Internal(format!("failed to fetch neoforge maven metadata: {e}"))
            })?
            .text()
            .await
            .map_err(|e| {
                SystemError::Internal(format!("failed to read neoforge metadata body: {e}"))
            })?;

        parse_maven_latest_version(&body)
    }

    async fn download_game_files(
        &self,
        version_manifest: &serde_json::Value,
        neoforge_version: &str,
    ) -> SystemResult<()> {
        let game_dir = &self.game_root;

        if game_dir.exists() {
            tokio::fs::remove_dir_all(game_dir)
                .await
                .map_err(|e| SystemError::Internal(format!("failed to clean game dir: {e}")))?;
        }

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
        let lang_dir = game_dir.join("assets/minecraft/lang");
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
        extract_zip(&client_dest, game_dir, EXTRACT_VANILLA_DIRS)?;
        tokio::fs::remove_file(&client_dest).await.ok();

        // Download NeoForge jar
        info!("downloading neoforge jar");
        let neoforge_url = NEOFORGE_URL_TEMPLATE.replace("{version}", neoforge_version);
        let neoforge_dest = game_dir.join("neoforge.jar");
        self.download_file(&neoforge_url, &neoforge_dest).await?;

        // Extract neoforge data
        info!("extracting neoforge jar");
        let combined_filter: Vec<&str> = EXTRACT_VANILLA_DIRS
            .iter()
            .chain(EXTRACT_NEOFORGE_DIRS.iter())
            .copied()
            .collect();
        extract_zip(&neoforge_dest, game_dir, &combined_filter)?;
        tokio::fs::remove_file(&neoforge_dest).await.ok();

        debug!("game data download successful");
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
                let hash = object["hash"]
                    .as_str()
                    .ok_or_else(|| SystemError::Internal("missing hash in asset object".into()))?;
                let prefix = &hash[..2];
                let resource_url = format!("{RESOURCES_URL}/{prefix}/{hash}");
                let download_path = lang_dir.join(file_name);

                self.download_file(&resource_url, &download_path).await?;
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

    async fn download_file(&self, url: &str, dest: &Path) -> SystemResult<()> {
        let bytes = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to download {url}: {e}")))?
            .bytes()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to read body from {url}: {e}")))?;

        tokio::fs::write(dest, &bytes).await.map_err(|e| {
            SystemError::Internal(format!("failed to write {}: {e}", dest.display()))
        })?;

        Ok(())
    }

    async fn register_items(&self, version_id: i64) -> SystemResult<()> {
        let items_root = self.game_root.join("assets/minecraft/items");
        if !items_root.exists() {
            debug!("no items directory found, skipping registration");
            return Ok(());
        }

        debug!("registering game items from asset files");
        let mut entries = tokio::fs::read_dir(&items_root)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to read items dir: {e}")))?;

        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| SystemError::Internal(format!("failed to begin transaction: {e}")))?;

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
                if let Err(e) =
                    query::ingestor::add_project_item(&tx, version_id, version_id, &item_id).await
                {
                    error!(item = %item_id, "failed to register game item: {e}");
                }
                count += 1;
            }
        }

        tx.commit().await.map_err(|e| {
            SystemError::Internal(format!("failed to commit item registration: {e}"))
        })?;

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

    async fn record_import(&self, game_version: &str, neoforge_version: &str) -> SystemResult<()> {
        use sea_orm::EntityTrait;
        use wiki_db::entity::data_import;

        let model = data_import::ActiveModel {
            game_version: Set(game_version.to_owned()),
            loader: Set("neoforge".to_owned()),
            loader_version: Set(neoforge_version.to_owned()),
            user_id: Set(None),
            created_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };

        data_import::Entity::insert(model)
            .exec(&self.db)
            .await
            .map_err(|e| {
                SystemError::Internal(format!("failed to insert data import record: {e}"))
            })?;

        Ok(())
    }

    async fn get_or_create_version(&self) -> SystemResult<i64> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        use wiki_db::entity::project_version;

        let existing = project_version::Entity::find()
            .filter(project_version::Column::ProjectId.eq(BUILTIN_PROJECT_ID))
            .one(&self.db)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to query version: {e}")))?;

        if let Some(v) = existing {
            return Ok(v.id);
        }

        let model = project_version::ActiveModel {
            project_id: Set(BUILTIN_PROJECT_ID.to_owned()),
            ..Default::default()
        };

        let result = project_version::Entity::insert(model)
            .exec(&self.db)
            .await
            .map_err(|e| SystemError::Internal(format!("failed to create version: {e}")))?;

        Ok(result.last_insert_id)
    }
}

struct VersionManifest {
    version: String,
    data: serde_json::Value,
}

fn parse_maven_latest_version(xml: &str) -> SystemResult<String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut in_versioning = false;
    let mut in_latest = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                if name.as_ref() == b"versioning" {
                    in_versioning = true;
                } else if in_versioning && name.as_ref() == b"latest" {
                    in_latest = true;
                }
            }
            Ok(Event::Text(e)) if in_latest => {
                let text = e
                    .xml10_content()
                    .map_err(|e| SystemError::Internal(format!("failed to decode XML text: {e}")))?
                    .into_owned();
                return Ok(text);
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                if name.as_ref() == b"latest" {
                    in_latest = false;
                } else if name.as_ref() == b"versioning" {
                    in_versioning = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(SystemError::Internal(format!("XML parse error: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }

    Err(SystemError::Internal(
        "could not find latest version in maven metadata".into(),
    ))
}

fn should_extract(path: &str, filter: &[&str]) -> bool {
    if path.ends_with('/') {
        return false;
    }
    if filter.is_empty() {
        return true;
    }
    filter.iter().any(|prefix| path.starts_with(prefix))
}

fn extract_zip(archive_path: &Path, dest_dir: &Path, filter: &[&str]) -> SystemResult<()> {
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
