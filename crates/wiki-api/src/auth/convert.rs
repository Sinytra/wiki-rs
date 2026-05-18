use crate::auth::User;
use wiki_domain::response::UserProfile;

impl From<&User> for UserProfile {
    fn from(user: &User) -> Self {
        Self {
            username: user.id.clone(),
            name: user.name.clone(),
            role: user.role.clone(),
            modrinth_id: user.modrinth_id.clone(),
            avatar_url: user.avatar_url.clone(),
            created_at: user.created_at,
        }
    }
}