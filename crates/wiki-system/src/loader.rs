use crate::error::{SystemError, SystemResult};

const JAR_CLASSIFIER: &str = "universal";

// Use Forge until 1.20.2 and NeoForge for newer versions
const NEOFORGE_MIN_GAME_VERSION: &[u64] = &[1, 20, 2];

static NEOFORGE_ARTIFACT: MavenArtifact = MavenArtifact {
    repository: "https://maven.neoforged.net/releases",
    group: "net/neoforged",
    name: "neoforge",
};

static FORGE_ARTIFACT: MavenArtifact = MavenArtifact {
    repository: "https://maven.minecraftforge.net/releases",
    group: "net/minecraftforge",
    name: "forge",
};

pub const ITEM_MODELS_DIR: &str = "assets/minecraft/models/item";
pub const ITEMS_DIR: &str = "assets/minecraft/items";

pub const LOADERS: &[&dyn ModLoader] = &[&NeoForge, &Forge];

pub fn loader_for(game_version: &GameVersion) -> Option<&'static dyn ModLoader> {
    if !game_version.is_numbered() {
        return None;
    }

    LOADERS
        .iter()
        .copied()
        .find(|loader| loader.supports(game_version))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersion {
    raw: String,
    components: Vec<u64>,
}

impl GameVersion {
    pub fn parse(raw: &str) -> Self {
        let components = raw
            .split('.')
            .map(|part| {
                let end = part
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(part.len());
                part[..end].parse::<u64>().ok()
            })
            .take_while(Option::is_some)
            .flatten()
            .collect();

        Self {
            raw: raw.to_owned(),
            components,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn components(&self) -> &[u64] {
        &self.components
    }

    pub fn at_least(&self, other: &[u64]) -> bool {
        self.components.as_slice() >= other
    }

    pub fn is_numbered(&self) -> bool {
        self.components.len() >= 2
    }
}

pub struct MavenArtifact {
    pub repository: &'static str,
    pub group: &'static str,
    pub name: &'static str,
}

impl MavenArtifact {
    pub fn metadata_url(&self) -> String {
        format!(
            "{}/{}/{}/maven-metadata.xml",
            self.repository, self.group, self.name
        )
    }

    pub fn jar_url(&self, version: &str, classifier: &str) -> String {
        format!(
            "{}/{}/{}/{version}/{}-{version}-{classifier}.jar",
            self.repository, self.group, self.name, self.name
        )
    }
}

pub trait ModLoader: Send + Sync {
    fn id(&self) -> &'static str;

    fn artifact(&self) -> &'static MavenArtifact;

    fn supports(&self, game_version: &GameVersion) -> bool;

    fn select_version(&self, versions: &[String], game_version: &GameVersion) -> Option<String>;

    fn metadata_url(&self) -> String {
        self.artifact().metadata_url()
    }

    fn jar_url(&self, loader_version: &str) -> String {
        self.artifact().jar_url(loader_version, JAR_CLASSIFIER)
    }

    fn items_listing_dir(&self) -> &'static str;
}

pub struct NeoForge;

impl NeoForge {
    pub fn select_neoforge_version(versions: &[String], game_version: &str) -> Option<String> {
        let prefixes = Self::get_version_prefixes(game_version);

        versions
            .iter()
            .filter_map(|version| {
                let build = prefixes.iter().find_map(|prefix| {
                    version
                        .strip_prefix(prefix.as_str())
                        .and_then(|rest| rest.strip_prefix('.'))
                        .and_then(Self::parse_version)
                })?;
                Some((build, version.clone()))
            })
            .max_by_key(|&((build, stable), _)| (stable, build))
            .map(|(_, version)| version)
    }

    fn get_version_prefixes(game_version: &str) -> Vec<String> {
        let primary = if game_version.matches('.').count() == 1 {
            format!("{game_version}.0")
        } else {
            game_version.to_owned()
        };
        let mut prefixes = vec![primary];

        if let Some(rest) = game_version.strip_prefix("1.")
            && !rest.is_empty()
        {
            let legacy = if rest.contains('.') {
                rest.to_owned()
            } else {
                format!("{rest}.0")
            };
            prefixes.push(legacy);
        }

        prefixes
    }

    fn parse_version(build: &str) -> Option<(u64, bool)> {
        let digits_end = build
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(build.len());
        let (digits, suffix) = build.split_at(digits_end);

        if !suffix.is_empty() && !suffix.starts_with('-') {
            return None;
        }

        Some((digits.parse().ok()?, suffix.is_empty()))
    }
}

impl ModLoader for NeoForge {
    fn id(&self) -> &'static str {
        "neoforge"
    }

    fn artifact(&self) -> &'static MavenArtifact {
        &NEOFORGE_ARTIFACT
    }

    fn supports(&self, game_version: &GameVersion) -> bool {
        game_version.at_least(NEOFORGE_MIN_GAME_VERSION)
    }

    fn select_version(&self, versions: &[String], game_version: &GameVersion) -> Option<String> {
        Self::select_neoforge_version(versions, game_version.as_str())
    }

    fn items_listing_dir(&self) -> &'static str {
        ITEMS_DIR
    }
}

pub struct Forge;

impl Forge {
    fn parse_version(version: &str, game_version: &str) -> Option<(Vec<u64>, bool)> {
        let rest = version
            .strip_prefix(game_version)
            .and_then(|rest| rest.strip_prefix('-'))?;
        let (build, branch) = rest.split_once('-').unwrap_or((rest, ""));
        let build: Vec<u64> = build
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect::<Option<_>>()?;

        Some((build, branch.is_empty()))
    }

    pub fn select_version(versions: &[String], game_version: &str) -> Option<String> {
        versions
            .iter()
            .filter_map(|version| {
                let build = Self::parse_version(version, game_version)?;
                Some((build, version.clone()))
            })
            .max_by(|((a_build, a_plain), _), ((b_build, b_plain), _)| {
                (a_build.as_slice(), a_plain).cmp(&(b_build.as_slice(), b_plain))
            })
            .map(|(_, version)| version)
    }
}

impl ModLoader for Forge {
    fn id(&self) -> &'static str {
        "forge"
    }

    fn artifact(&self) -> &'static MavenArtifact {
        &FORGE_ARTIFACT
    }

    fn supports(&self, game_version: &GameVersion) -> bool {
        !game_version.at_least(NEOFORGE_MIN_GAME_VERSION)
    }

    fn select_version(&self, versions: &[String], game_version: &GameVersion) -> Option<String> {
        Self::select_version(versions, game_version.as_str())
    }

    fn items_listing_dir(&self) -> &'static str {
        ITEM_MODELS_DIR
    }
}

pub fn parse_maven_versions(xml: &str) -> SystemResult<Vec<String>> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut in_versioning = false;
    let mut in_versions = false;
    let mut in_version = false;
    let mut versions = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                if name.as_ref() == b"versioning" {
                    in_versioning = true;
                } else if in_versioning && name.as_ref() == b"versions" {
                    in_versions = true;
                } else if in_versions && name.as_ref() == b"version" {
                    in_version = true;
                }
            }
            Ok(Event::Text(e)) if in_version => {
                let text = e
                    .xml10_content()
                    .map_err(|e| SystemError::Internal(format!("failed to decode XML text: {e}")))?
                    .into_owned();
                versions.push(text);
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                if name.as_ref() == b"version" {
                    in_version = false;
                } else if name.as_ref() == b"versions" {
                    in_versions = false;
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

    if versions.is_empty() {
        return Err(SystemError::Internal(
            "could not find any versions in maven metadata".into(),
        ));
    }

    Ok(versions)
}
