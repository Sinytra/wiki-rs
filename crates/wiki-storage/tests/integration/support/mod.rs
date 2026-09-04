use std::path::{Path, PathBuf};
use std::sync::Mutex;

use wiki_domain::error::{ProjectError, ProjectIssueLevel};
use wiki_storage::ingestor::JsonSource;
use wiki_storage::ingestor::issues::{FileIssues, IssueSink, ProjectIssue};
use wiki_storage::ingestor::recipes::parser::{RecipeParseError, default_registry};
use wiki_storage::ingestor::recipes::types::{
    StubRecipe, StubRecipeIngredient, VanillaIngredient, VanillaIngredientList,
};

#[derive(Default)]
pub struct CollectingIssueSink {
    issues: Mutex<Vec<ProjectIssue>>,
}

impl CollectingIssueSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn issues(&self) -> Vec<ProjectIssue> {
        self.issues.lock().unwrap().clone()
    }
}

impl IssueSink for CollectingIssueSink {
    fn add(&self, issue: ProjectIssue) {
        self.issues.lock().unwrap().push(issue);
    }

    fn has_errors(&self) -> bool {
        self.issues
            .lock()
            .unwrap()
            .iter()
            .any(|i| i.level == ProjectIssueLevel::Error)
    }
}

pub fn fixture_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

pub fn fixture(rel: &str) -> String {
    let path = fixture_path(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

pub fn item(id: &str) -> VanillaIngredient {
    VanillaIngredient::Item(id.to_owned())
}

pub fn tag(id: &str) -> VanillaIngredient {
    VanillaIngredient::Tag(id.to_owned())
}

pub fn ingredients(json: &str) -> Vec<VanillaIngredient> {
    match serde_json::from_str::<VanillaIngredientList>(json) {
        Ok(list) => list.0,
        Err(e) => panic!("expected ingredient to parse, got error: {e}\ninput: {json}"),
    }
}

pub fn ingredient_error(json: &str) -> String {
    match serde_json::from_str::<VanillaIngredientList>(json) {
        Ok(list) => panic!(
            "expected ingredient to fail, got: {:?}\ninput: {json}",
            list.0
        ),
        Err(e) => e.to_string(),
    }
}

pub fn stub(
    item_id: &str,
    slot: &str,
    count: i32,
    input: bool,
    is_tag: bool,
) -> StubRecipeIngredient {
    StubRecipeIngredient {
        item_id: item_id.to_owned(),
        slot: slot.to_owned(),
        count,
        input,
        is_tag,
    }
}

pub fn input(item_id: &str, slot: &str) -> StubRecipeIngredient {
    stub(item_id, slot, 1, true, false)
}

pub fn input_tag(tag_id: &str, slot: &str) -> StubRecipeIngredient {
    stub(tag_id, slot, 1, true, true)
}

pub fn output(item_id: &str, slot: &str, count: i32) -> StubRecipeIngredient {
    stub(item_id, slot, count, false, false)
}

pub struct ParsedRecipe {
    pub result: Result<Option<StubRecipe>, RecipeParseError>,
    pub issues: Vec<ProjectIssue>,
}

impl ParsedRecipe {
    pub fn recipe(self) -> StubRecipe {
        match self.result {
            Ok(Some(r)) => r,
            Ok(None) => panic!("recipe was skipped, issues: {:?}", self.issues),
            Err(e) => panic!("recipe failed to parse: {e}\nissues: {:?}", self.issues),
        }
    }

    pub fn error(self) -> RecipeParseError {
        match self.result {
            Ok(r) => panic!("expected recipe to fail, got: {r:?}"),
            Err(e) => e,
        }
    }

    pub fn skipped(self) -> Vec<ProjectIssue> {
        match self.result {
            Ok(None) => self.issues,
            other => panic!("expected recipe to be skipped, got: {other:?}"),
        }
    }

    pub fn subjects(&self) -> Vec<ProjectError> {
        self.issues.iter().map(|i| i.subject).collect()
    }
}

pub fn parse_recipe(id: &str, json: &str) -> ParsedRecipe {
    let value = serde_json::from_str(json).expect("recipe fixture must be valid JSON");
    let data = JsonSource {
        value,
        source: json.to_owned(),
    };
    let sink = CollectingIssueSink::new();
    let issues = FileIssues::new(&sink, format!("{id}.json"));
    let result = default_registry().parse_recipe(id, &data, &issues);
    ParsedRecipe {
        result,
        issues: sink.issues(),
    }
}

pub fn parse_fixture_recipe(rel: &str) -> ParsedRecipe {
    let stem = Path::new(rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("fixture path must have a file stem");
    parse_recipe(&format!("test:{stem}"), &fixture(rel))
}
