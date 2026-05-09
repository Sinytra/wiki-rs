use crate::curseforge::CurseForge;
use crate::error::ExternalResult;
use crate::modrinth::Modrinth;
use strum::EnumString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum ProjectType {
    Mod,
    ResourcePack,
    DataPack,
    Shader,
    ModPack,
    Plugin,
    #[strum(disabled)]
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PlatformProject {
    pub slug: String,
    pub name: String,
    pub source_url: String,
    pub project_type: ProjectType,
    pub platform: &'static str,
}

#[derive(Clone)]
pub struct Platforms {
    pub modrinth: Modrinth,
    pub curseforge: CurseForge,
}

impl Platforms {
    pub fn new(modrinth: Modrinth, curseforge: CurseForge) -> Self {
        Self {
            modrinth,
            curseforge,
        }
    }

    pub fn available_platforms(&self) -> Vec<&'static str> {
        vec![crate::modrinth::PLATFORM, crate::curseforge::PLATFORM]
    }

    pub async fn get_project(
        &self,
        platform: &str,
        slug: &str,
    ) -> ExternalResult<Option<PlatformProject>> {
        match platform {
            crate::modrinth::PLATFORM => self.modrinth.get_project(slug).await,
            crate::curseforge::PLATFORM => self.curseforge.get_project(slug).await,
            _ => Ok(None),
        }
    }
}
