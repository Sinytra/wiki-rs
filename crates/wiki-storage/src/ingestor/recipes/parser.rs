use std::sync::LazyLock;

use serde::Deserialize;
use thiserror::Error;
use wiki_domain::content::ResourceLocation;

use crate::ingestor::issues::FileIssues;
use crate::ingestor::JsonSource;
use crate::ingestor::recipes::builtin::VanillaRecipeParser;
use crate::ingestor::recipes::custom::CustomRecipeParser;
use crate::ingestor::recipes::types::StubRecipe;

static REGISTRY: LazyLock<RecipeParserRegistry> = LazyLock::new(RecipeParserRegistry::default);

pub fn default_registry() -> &'static RecipeParserRegistry {
    &REGISTRY
}

#[derive(Debug, Error)]
pub enum RecipeParseError {
    #[error("recipe JSON missing required `type` field")]
    MissingType,
    #[error(transparent)]
    InvalidJson(#[from] serde_json::Error),
    #[error(transparent)]
    InvalidJsonPath(#[from] serde_path_to_error::Error<serde_json::Error>),
    #[error("unknown recipe type: {0}")]
    UnknownRecipeType(String),
}

pub trait RecipeParser: Send + Sync {
    fn handles(&self, loc: &ResourceLocation) -> bool;

    fn parse(
        &self,
        id: &str,
        recipe_type: &str,
        data: &JsonSource,
        issues: &FileIssues<'_>,
    ) -> Result<Option<StubRecipe>, RecipeParseError>;
}

#[derive(Deserialize)]
struct BaseRecipe {
    #[serde(rename = "type")]
    r#type: Option<String>,
}

pub struct RecipeParserRegistry {
    parsers: Vec<Box<dyn RecipeParser>>,
}

impl Default for RecipeParserRegistry {
    fn default() -> Self {
        Self {
            parsers: vec![
                Box::new(VanillaRecipeParser::new()),
                Box::new(CustomRecipeParser),
            ],
        }
    }
}

impl RecipeParserRegistry {
    pub fn find(&self, loc: &ResourceLocation) -> Option<&dyn RecipeParser> {
        self.parsers.iter().find(|p| p.handles(loc)).map(|b| &**b)
    }

    pub fn parse_recipe(
        &self,
        id: &str,
        data: &JsonSource,
        issues: &FileIssues<'_>,
    ) -> Result<Option<StubRecipe>, RecipeParseError> {
        let base: BaseRecipe =
            serde_json::from_value(data.value.clone()).map_err(RecipeParseError::InvalidJson)?;
        let Some(type_str) = base.r#type else {
            return Err(RecipeParseError::MissingType);
        };

        let Some(loc) = issues.parse_resloc(&type_str) else {
            return Ok(None);
        };

        let Some(parser) = self.find(&loc) else {
            return Err(RecipeParseError::UnknownRecipeType(type_str));
        };

        parser.parse(id, &type_str, data, issues)
    }
}
