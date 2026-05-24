pub mod builtin;
pub mod custom;
pub mod parser;
pub mod types;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use tracing::{debug, error, trace, warn};
use walkdir::{DirEntry, WalkDir};
use wiki_db::query;
use wiki_domain::error::ProjectError;

use crate::error::StorageResult;
use crate::format::JSON_EXT;
use crate::ingestor::issues::{FileIssues, IssueSink};
use crate::ingestor::recipes::parser::{RecipeParserRegistry, default_registry};
use crate::ingestor::recipes::types::{PreparedRecipeType, StubRecipe, StubRecipeType};
use crate::ingestor::{IngestContext, PreparationResult, SubIngestor, parse_json_path, JsonSource};

struct PreparedFile<T> {
    data: T,
    file: PathBuf,
}

pub struct RecipesSubIngestor {
    registry: &'static RecipeParserRegistry,
    recipe_types: Vec<PreparedFile<PreparedRecipeType>>,
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

fn read_recipe_type_file(
    namespace: &str,
    root: &Path,
    path: &Path,
    issues: &dyn IssueSink,
) -> Option<PreparedRecipeType> {
    let file_issues = FileIssues::new(issues, path.to_owned());
    let id = file_issues.loc_from_relative(namespace, root, path)?;

    let parsed: StubRecipeType = parse_json_path("recipe_type", path, &file_issues)?.value;
    Some(PreparedRecipeType {
        id: id.to_string(),
        background: parsed.background,
        input_slots: parsed.input_slots,
        output_slots: parsed.output_slots,
    })
}

fn read_recipe_file(
    namespace: &str,
    root: &Path,
    path: &Path,
    registry: &RecipeParserRegistry,
    issues: &dyn IssueSink,
) -> Option<StubRecipe> {
    let file_issues = FileIssues::new(issues, path.to_owned());
    let id = file_issues.loc_from_relative(namespace, root, path)?;

    let json: JsonSource = parse_json_path("recipe", path, &file_issues)?;

    match registry.parse_recipe(&id.to_string(), &json, &file_issues) {
        Ok(Some(r)) => Some(r),
        Ok(None) => None,
        Err(e) => {
            error!(recipe = %id, "Error parsing recipe: {e}");
            file_issues.ingestor_error(ProjectError::InvalidFormat, e.to_string());
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
        debug!(count = self.recipe_types.len(), "Adding recipe types");
        for rt in &self.recipe_types {
            trace!(id = %rt.data.id, "Registering recipe type");
            query::ingestor::add_recipe_type(conn, ctx.version_id, &rt.data.id).await?;
        }

        debug!(count = self.recipes.len(), "Adding recipes");
        for r in &self.recipes {
            let file = r.file.clone();
            let id = &r.data.id;
            let r_type = &r.data.r#type;
            let file_issues = FileIssues::new(&*ctx.issues, file);

            let recipe_type = query::ingestor::get_recipe_type_by_loc(conn, r_type).await?;
            let Some(rt) = recipe_type else {
                file_issues.ingestor_error(ProjectError::UnknownRecipeType, r_type.clone());
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
                    warn!(recipe = %id, ingredient = %name, "Invalid ingredient: {e}");
                    file_issues.ingestor_error(
                        ProjectError::InvalidIngredient,
                        format!("Invalid ingredient: '{name}'"),
                    );
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
