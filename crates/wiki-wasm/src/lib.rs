use std::fmt::Display;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wiki_content::content::{ContentIssue, content_tree, scan_content_pages};
use wiki_content::fs::{ContentFs, DirEntry};
use wiki_content::infobox::{default_infobox, merge_infobox};
use wiki_content::layout::{Layout, Section};
use wiki_content::links::parse_links;
use wiki_content::markdown::{collect_links, first_h1, parse_frontmatter, parse_mdast};
use wiki_content::metadata::parse_project_metadata;
use wiki_content::resolve::{LinkData, ProjectView, link_data_from_pages};
use wiki_content::tree::{TreeIssue, build_tree};
use wiki_domain::content::ResourceLocation;
use wiki_domain::metadata::ProjectSchemaVersion;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::tree::ContentFileTree;

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &str = r#"
import type {
  ContentFileTreeEntry,
  FileTreeEntry,
  Frontmatter,
  Infobox,
  ProjectMetadata,
  ProjectSchemaVersion,
  ResolvedLink,
  ResourceLocation
} from '@sinytra/wiki-api-types';

export type Section = 'docs' | 'content';

export interface TreeIssue {
  kind: 'folder_metadata' | 'frontmatter';
  path: string;
  message: string;
}

export interface DocsTree {
  tree: FileTreeEntry[];
  issues: TreeIssue[];
}

export interface ContentIssue {
  kind: 'invalid_frontmatter' | 'missing_ids' | 'invalid_id' | 'no_page_title';
  path: string;
  message: string;
}

export interface ContentTree {
  tree: ContentFileTreeEntry[];
  issues: TreeIssue[];
  page_issues: ContentIssue[];
}

export interface ParsedPage {
  frontmatter: Frontmatter;
  links: string[];
  heading: string | null;
}

"#;

#[derive(Serialize)]
struct ParsedPage {
    frontmatter: Frontmatter,
    links: Vec<String>,
    heading: Option<String>,
}

fn js_error(e: impl Display) -> JsError {
    JsError::new(&e.to_string())
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(js_error)
}

fn layout_from_js(schema: JsValue) -> Result<Layout, JsError> {
    let schema: ProjectSchemaVersion = serde_wasm_bindgen::from_value(schema).map_err(js_error)?;
    Ok(Layout::new(schema))
}

fn section_from_js(section: JsValue) -> Result<Section, JsError> {
    serde_wasm_bindgen::from_value(section).map_err(js_error)
}

#[derive(Deserialize)]
struct JsResourceLocation {
    namespace: String,
    path: String,
}

fn location_from_js(location: JsValue) -> Result<ResourceLocation, JsError> {
    let location: JsResourceLocation =
        serde_wasm_bindgen::from_value(location).map_err(js_error)?;
    Ok(ResourceLocation::new(location.namespace, location.path))
}

#[wasm_bindgen(js_name = parsePage, unchecked_return_type = "ParsedPage")]
pub fn parse_page(text: &str) -> Result<JsValue, JsError> {
    let tree = parse_mdast(text).map_err(js_error)?;
    to_js(&ParsedPage {
        frontmatter: parse_frontmatter(&tree).map_err(js_error)?,
        links: collect_links(&tree),
        heading: first_h1(&tree),
    })
}

#[wasm_bindgen(js_name = parseProjectMetadata, unchecked_return_type = "ProjectMetadata")]
pub fn parse_project_metadata_js(text: &str) -> Result<JsValue, JsError> {
    to_js(&parse_project_metadata(text).map_err(js_error)?)
}

#[wasm_bindgen(js_name = buildInfobox, unchecked_return_type = "Infobox")]
pub fn build_infobox(
    #[wasm_bindgen(unchecked_param_type = "Frontmatter")] frontmatter: JsValue,
    names: Vec<String>,
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
) -> Result<JsValue, JsError> {
    let frontmatter: Frontmatter = serde_wasm_bindgen::from_value(frontmatter).map_err(js_error)?;
    let layout = layout_from_js(schema)?;
    let default = default_infobox(&frontmatter, &names, |id| layout.item_asset_id(id));
    to_js(&merge_infobox(default, frontmatter.infobox))
}

#[wasm_bindgen(js_name = itemAssetLocation, unchecked_return_type = "ResourceLocation")]
pub fn item_asset_location(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    #[wasm_bindgen(unchecked_param_type = "ResourceLocation")] location: JsValue,
) -> Result<JsValue, JsError> {
    let layout = layout_from_js(schema)?;
    to_js(&layout.item_asset_location(&location_from_js(location)?))
}

#[wasm_bindgen(js_name = assetPaths)]
pub fn asset_paths(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    #[wasm_bindgen(unchecked_param_type = "ResourceLocation")] location: JsValue,
) -> Result<Vec<String>, JsError> {
    let layout = layout_from_js(schema)?;
    Ok(layout.asset_paths(&location_from_js(location)?).to_vec())
}

#[wasm_bindgen(js_name = pagePath)]
pub fn page_path(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Section")] section: JsValue,
    slug: &str,
) -> Result<String, JsError> {
    Ok(layout_from_js(schema)?.page_path(section_from_js(section)?, slug))
}

#[wasm_bindgen(js_name = docsIndexSlug)]
pub fn docs_index_slug(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
) -> Result<String, JsError> {
    Ok(layout_from_js(schema)?.docs_index_slug().to_owned())
}

#[wasm_bindgen(js_name = translationsDir)]
pub fn translations_dir(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
) -> Result<String, JsError> {
    Ok(layout_from_js(schema)?.translations_dir().to_owned())
}

#[wasm_bindgen(js_name = localizedPath)]
pub fn localized_path(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    rel: &str,
    locale: &str,
) -> Result<String, JsError> {
    Ok(layout_from_js(schema)?.localized_path(rel, locale))
}

#[wasm_bindgen(js_name = itemPropertiesPath)]
pub fn item_properties_path(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    modid: &str,
) -> Result<String, JsError> {
    Ok(layout_from_js(schema)?.item_properties_path(modid))
}

#[wasm_bindgen(js_name = languageFilePath)]
pub fn language_file_path(
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    namespace: &str,
    locale: &str,
) -> Result<String, JsError> {
    Ok(layout_from_js(schema)?.language_file_path(namespace, locale))
}

#[wasm_bindgen(module = "fs")]
extern "C" {
    #[wasm_bindgen(js_name = readFileSync, catch)]
    fn read_file_sync(path: &str, encoding: &str) -> Result<String, JsValue>;

    #[wasm_bindgen(js_name = readdirSync, catch)]
    fn readdir_sync(path: &str, options: &JsValue) -> Result<Vec<JsValue>, JsValue>;

    #[wasm_bindgen(js_name = existsSync)]
    fn exists_sync(path: &str) -> bool;

    type Dirent;

    #[wasm_bindgen(method, getter)]
    fn name(this: &Dirent) -> String;

    #[wasm_bindgen(method, js_name = isDirectory)]
    fn is_directory(this: &Dirent) -> bool;

    #[wasm_bindgen(method, js_name = isFile)]
    fn is_file(this: &Dirent) -> bool;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReaddirOptions {
    with_file_types: bool,
}

struct NodeFs {
    root: String,
}

impl NodeFs {
    fn path(&self, rel: &str) -> String {
        if rel.is_empty() {
            self.root.to_owned()
        } else {
            format!("{}/{rel}", self.root.trim_end_matches(['/', '\\']))
        }
    }
}

impl ContentFs for NodeFs {
    fn read_to_string(&self, path: &str) -> Option<String> {
        read_file_sync(&self.path(path), "utf8").ok()
    }

    fn read_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
        let options = to_js(&ReaddirOptions {
            with_file_types: true,
        })
        .ok()?;
        let entries = readdir_sync(&self.path(path), &options).ok()?;
        Some(
            entries
                .into_iter()
                .map(JsCast::unchecked_into::<Dirent>)
                .filter(|e| e.is_directory() || e.is_file())
                .map(|e| DirEntry {
                    name: e.name(),
                    is_dir: e.is_directory(),
                })
                .collect(),
        )
    }

    fn exists(&self, path: &str) -> bool {
        exists_sync(&self.path(path))
    }
}

#[wasm_bindgen(js_name = buildDocsTree, unchecked_return_type = "DocsTree")]
pub fn build_docs_tree(
    root: &str,
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Section")] section: JsValue,
    locale: Option<String>,
) -> Result<JsValue, JsError> {
    let layout = layout_from_js(schema)?;
    let section = section_from_js(section)?;
    to_js(&build_tree(
        &NodeFs { root: root.into() },
        layout,
        section,
        locale.as_deref(),
    ))
}

#[derive(Serialize)]
struct ContentTreeResult {
    tree: ContentFileTree,
    issues: Vec<TreeIssue>,
    page_issues: Vec<ContentIssue>,
}

#[wasm_bindgen(js_name = buildContentTree, unchecked_return_type = "ContentTree")]
pub fn build_content_tree(
    root: &str,
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    modid: Option<String>,
    locale: Option<String>,
) -> Result<JsValue, JsError> {
    let layout = layout_from_js(schema)?;
    let fs = NodeFs { root: root.into() };

    let (refs, page_issues) = match modid {
        Some(modid) => {
            let scan = scan_content_pages(&fs, layout, &modid);
            let refs = scan.pages.into_iter().map(|p| (p.slug, p.r#ref)).collect();
            (refs, scan.issues)
        }
        None => Default::default(),
    };

    let docs = build_tree(&fs, layout, Section::Content, locale.as_deref());
    to_js(&ContentTreeResult {
        tree: content_tree(&fs, layout, locale.as_deref(), docs.tree, &refs),
        issues: docs.issues,
        page_issues,
    })
}

fn scanned_link_data(fs: &NodeFs, layout: Layout, modid: Option<&str>) -> LinkData {
    modid
        .map(|modid| link_data_from_pages(&scan_content_pages(fs, layout, modid).pages))
        .unwrap_or_default()
}

#[wasm_bindgen(js_name = resolveLinks, unchecked_return_type = "Record<string, ResolvedLink>")]
pub fn resolve_links(
    root: &str,
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    modid: Option<String>,
    locale: Option<String>,
    lang: &str,
    links: Vec<String>,
) -> Result<JsValue, JsError> {
    let layout = layout_from_js(schema)?;
    let fs = NodeFs { root: root.into() };
    let data = scanned_link_data(&fs, layout, modid.as_deref());
    let view = ProjectView {
        fs: &fs,
        layout,
        locale: locale.as_deref(),
        lang,
    };
    to_js(&view.resolve_links(&parse_links(&links), modid.as_deref(), &data))
}

#[wasm_bindgen(js_name = itemNames, unchecked_return_type = "(string | null)[]")]
pub fn item_names(
    root: &str,
    #[wasm_bindgen(unchecked_param_type = "ProjectSchemaVersion")] schema: JsValue,
    modid: Option<String>,
    locale: Option<String>,
    lang: &str,
    ids: Vec<String>,
) -> Result<JsValue, JsError> {
    let layout = layout_from_js(schema)?;
    let fs = NodeFs { root: root.into() };
    let data = scanned_link_data(&fs, layout, modid.as_deref());
    let view = ProjectView {
        fs: &fs,
        layout,
        locale: locale.as_deref(),
        lang,
    };
    let names: Vec<Option<String>> = ids
        .iter()
        .map(|id| {
            let page = data.item_pages.get(id).map(|p| p.slug.as_str());
            view.item_name(id, page)
        })
        .collect();
    to_js(&names)
}
