pub mod builtin;
pub mod custom;
pub mod parser;
pub mod types;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use tracing::{error, info, trace, warn};
use walkdir::{DirEntry, WalkDir};
use wiki_db::query;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::ProjectError;

use crate::error::StorageResult;
use crate::format::JSON_EXT;
use crate::ingestor::issues::{FileIssues, IssueSink};
use crate::ingestor::recipes::parser::{RecipeParserRegistry, default_registry};
use crate::ingestor::recipes::types::{StubRecipe, StubRecipeType};
use crate::ingestor::{IngestContext, PreparationResult, SubIngestor, parse_json_path};

struct PreparedFile<T> {
    data: T,
    file: PathBuf,
}

pub struct RecipesSubIngestor {
    registry: &'static RecipeParserRegistry,
    recipe_types: Vec<PreparedFile<StubRecipeType>>,
    recipes: Vec<PreparedFile<StubRecipe>>,
}

impl Default for RecipesSubIngestor {
    fn default() -> Self {
        Self {
            registry: default_registry(),
            recipe_types: Vec::new(),
            recipes: Vec::new(),
        }
    }
}

fn loc_from_relative(namespace: &str, root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let s = rel.to_string_lossy();
    let stem = s.rfind('.').map(|i| &s[..i]).unwrap_or(&s);
    Some(format!("{namespace}:{stem}"))
}

fn read_recipe_type_file(
    namespace: &str,
    root: &Path,
    path: &Path,
    issues: &dyn IssueSink,
) -> Option<StubRecipeType> {
    let id = loc_from_relative(namespace, root, path)?;
    let file_issues = FileIssues::new(issues, path.to_owned());

    if !ResourceLocation::validate(&id) {
        file_issues.error(ProjectError::InvalidResloc, id.clone());
        return None;
    }

    let mut parsed: StubRecipeType = parse_json_path("recipe_type", path, &file_issues)?;
    parsed.id = id;
    Some(parsed)
}

fn read_recipe_file(
    namespace: &str,
    root: &Path,
    path: &Path,
    registry: &RecipeParserRegistry,
    issues: &dyn IssueSink,
) -> Option<StubRecipe> {
    let id = loc_from_relative(namespace, root, path)?;
    let file_issues = FileIssues::new(issues, path.to_owned());

    if !ResourceLocation::validate(&id) {
        file_issues.error(ProjectError::InvalidResloc, id.clone());
        return None;
    }

    let json: serde_json::Value = parse_json_path("recipe", path, &file_issues)?;

    match registry.parse_recipe(&id, &json, &file_issues) {
        Ok(Some(r)) => Some(r),
        Ok(None) => None,
        Err(e) => {
            error!(recipe = %id, "Error parsing recipe: {e}");
            file_issues.error(ProjectError::InvalidFormat, e.to_string());
            None
        }
    }
}

fn is_json_file(entry: &DirEntry) -> bool {
    entry.file_type().is_file()
        && entry.path().extension().and_then(|x| x.to_str()) == Some(JSON_EXT)
}

#[async_trait]
impl SubIngestor for RecipesSubIngestor {
    fn name(&self) -> &'static str {
        "Recipes"
    }

    async fn prepare(&mut self, ctx: &IngestContext<'_>) -> StorageResult<PreparationResult> {
        let mut result = PreparationResult::default();
        let data_root = ctx.format.data_root();
        let modid = ctx.modid;

        let recipes_root = data_root.join(modid).join("recipe"); // TODO Use format
        if recipes_root.exists() {
            for entry in WalkDir::new(&recipes_root)
                .into_iter()
                .filter_map(Result::ok)
                .filter(is_json_file)
            {
                let path = entry.path();

                match read_recipe_file(modid, &recipes_root, path, self.registry, &*ctx.issues) {
                    Some(r) => {
                        for ing in &r.ingredients {
                            if !ing.is_tag {
                                result.items.insert(ing.item_id.clone());
                            }
                        }

                        self.recipes.push(PreparedFile {
                            data: r,
                            file: path.to_owned(),
                        });
                    }
                    None => {
                        warn!(file = %path.display(), "Skipping recipe file");
                    }
                }
            }
        }

        let types_root = data_root.join(modid).join("recipe_type"); // TODO Use format
        if types_root.exists() {
            for entry in WalkDir::new(&types_root)
                .into_iter()
                .filter_map(Result::ok)
                .filter(is_json_file)
            {
                let path = entry.path();

                match read_recipe_type_file(modid, &types_root, path, &*ctx.issues) {
                    Some(rt) => {
                        self.recipe_types.push(PreparedFile {
                            data: rt,
                            file: path.to_owned(),
                        });
                    }
                    None => {
                        warn!(file = %path.display(), "Skipping recipe type file");
                    }
                }
            }
        }

        Ok(result)
    }

    async fn execute(
        &mut self,
        ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        info!(count = self.recipe_types.len(), "Adding recipe types");
        for rt in &self.recipe_types {
            trace!(id = %rt.data.id, "Registering recipe type");
            query::ingestor::add_recipe_type(conn, ctx.version_id, &rt.data.id).await?;
        }

        info!(count = self.recipes.len(), "Adding recipes");
        for r in &self.recipes {
            let file = r.file.clone();
            let id = &r.data.id;
            let r_type = &r.data.r#type;
            let file_issues = FileIssues::new(&*ctx.issues, file);

            let recipe_type = query::ingestor::get_recipe_type_by_loc(conn, r_type).await?;
            let Some(rt) = recipe_type else {
                file_issues.error(ProjectError::UnknownRecipeType, r_type.clone());
                continue;
            };

            let recipe_row = query::ingestor::add_recipe(conn, ctx.version_id, id, rt.id).await?;

            let mut failed = false;
            for ing in &r.data.ingredients {
                let res = if ing.is_tag {
                    query::ingestor::add_recipe_ingredient_tag(
                        conn,
                        recipe_row.id,
                        &ing.item_id,
                        &ing.slot,
                        ing.count,
                        ing.input,
                    )
                    .await
                } else {
                    query::ingestor::add_recipe_ingredient_item(
                        conn,
                        recipe_row.id,
                        &ing.item_id,
                        &ing.slot,
                        ing.count,
                        ing.input,
                    )
                    .await
                };
                if let Err(e) = res {
                    let name = if ing.is_tag {
                        format!("#{}", ing.item_id)
                    } else {
                        ing.item_id.clone()
                    };
                    warn!(recipe = %id, ingredient = %name, "Missing ingredient: {e}");
                    file_issues.error(ProjectError::InvalidIngredient, name);
                    failed = true;
                    break;
                }
            }

            if failed {
                query::ingestor::delete_recipe(conn, recipe_row.id).await?;
            }
        }

        Ok(())
    }
}
