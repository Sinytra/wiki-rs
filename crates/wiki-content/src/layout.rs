use serde::Deserialize;
use wiki_domain::content::ResourceLocation;
use wiki_domain::metadata::ProjectSchemaVersion;

pub const WIKI_META_FILE: &str = "sinytra-wiki.json";
pub const FOLDER_META_FILE: &str = "_meta.json";
pub const DOCS_FILE_EXT: &str = "mdx";

const ASSET_EXT: &str = ".png";
const LANG_DIR: &str = "lang";
const RECIPES_DIR: &str = "recipe";
const RECIPE_TYPES_DIR: &str = "recipe_type";
const WORKBENCHES_FILE: &str = "workbenches.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Docs,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    schema: ProjectSchemaVersion,
}

fn join(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("/")
}

impl Layout {
    pub fn new(schema: ProjectSchemaVersion) -> Self {
        Self { schema }
    }

    pub fn schema(&self) -> ProjectSchemaVersion {
        self.schema
    }

    pub fn assets_dir(&self) -> &'static str {
        match self.schema {
            ProjectSchemaVersion::Legacy => ".assets",
            ProjectSchemaVersion::V1 => "assets",
        }
    }

    pub fn data_dir(&self) -> &'static str {
        match self.schema {
            ProjectSchemaVersion::Legacy => ".data",
            ProjectSchemaVersion::V1 => "data",
        }
    }

    pub fn translations_dir(&self) -> &'static str {
        match self.schema {
            ProjectSchemaVersion::Legacy => ".translated",
            ProjectSchemaVersion::V1 => "translated",
        }
    }

    pub fn section_dir(&self, section: Section) -> &'static str {
        match (self.schema, section) {
            (ProjectSchemaVersion::Legacy, Section::Docs) => "",
            (ProjectSchemaVersion::Legacy, Section::Content) => ".content",
            (ProjectSchemaVersion::V1, Section::Docs) => "docs",
            (ProjectSchemaVersion::V1, Section::Content) => "content",
        }
    }

    pub fn docs_index_slug(&self) -> &'static str {
        match self.schema {
            ProjectSchemaVersion::Legacy => "_homepage",
            ProjectSchemaVersion::V1 => "_index",
        }
    }

    pub fn section_path(&self, section: Section, rel: &str) -> String {
        join(&[self.section_dir(section), rel])
    }

    pub fn page_path(&self, section: Section, slug: &str) -> String {
        self.section_path(section, &format!("{slug}.{DOCS_FILE_EXT}"))
    }

    pub fn docs_index_page_path(&self) -> String {
        self.page_path(Section::Docs, self.docs_index_slug())
    }

    pub fn localized_path(&self, rel: &str, locale: &str) -> String {
        join(&[self.translations_dir(), locale, rel.trim_start_matches('/')])
    }

    pub fn item_properties_path(&self, modid: &str) -> String {
        match self.schema {
            ProjectSchemaVersion::Legacy => join(&[self.data_dir(), "properties.json"]),
            ProjectSchemaVersion::V1 => join(&[self.data_dir(), modid, "properties/item.json"]),
        }
    }

    pub fn language_file_path(&self, namespace: &str, locale: &str) -> String {
        join(&[
            self.assets_dir(),
            namespace,
            LANG_DIR,
            &format!("{locale}.json"),
        ])
    }

    pub fn data_recipes_path(&self, modid: &str) -> String {
        join(&[modid, RECIPES_DIR])
    }

    pub fn data_recipe_types_path(&self, modid: &str) -> String {
        join(&[modid, RECIPE_TYPES_DIR])
    }

    pub fn data_workbenches_path(&self, modid: &str) -> String {
        match self.schema {
            ProjectSchemaVersion::Legacy => WORKBENCHES_FILE.to_owned(),
            ProjectSchemaVersion::V1 => join(&[modid, WORKBENCHES_FILE]),
        }
    }

    pub fn item_asset_location(&self, location: &ResourceLocation) -> ResourceLocation {
        match self.schema {
            ProjectSchemaVersion::Legacy => location.clone(),
            ProjectSchemaVersion::V1 => location.with_path_prefix("item/"),
        }
    }

    pub fn item_asset_id(&self, id: &str) -> String {
        ResourceLocation::parse(id)
            .map(|loc| self.item_asset_location(&loc).to_string())
            .unwrap_or_else(|| id.to_owned())
    }

    pub fn asset_paths(&self, location: &ResourceLocation) -> [String; 2] {
        let legacy =
            ResourceLocation::new("item", format!("{}/{}", location.namespace, location.path));
        [self.asset_path(location), self.asset_path(&legacy)]
    }

    fn asset_path(&self, location: &ResourceLocation) -> String {
        let ext = if location.path.contains('.') {
            ""
        } else {
            ASSET_EXT
        };
        join(&[
            self.assets_dir(),
            &location.namespace,
            &format!("{}{ext}", location.path),
        ])
    }
}
