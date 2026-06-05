use crate::entity::project_version;
use crate::error::{DbError, DbResult};
use crate::query::{PaginatedData, paginate};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, Condition, Order, QueryOrder, QuerySelect, Set};

#[tracing::instrument(name = "Creating project version", skip(db, model))]
pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: project_version::ActiveModel,
) -> DbResult<project_version::Model> {
    Ok(model.insert(db).await?)
}

#[tracing::instrument(name = "Getting default project version", skip(db))]
pub async fn get_default_version<C: ConnectionTrait>(
    db: &C,
    project_id: &str,
) -> DbResult<project_version::Model> {
    get_version(db, project_id, None).await
}

#[tracing::instrument(name = "Getting project version", skip(db, name))]
pub async fn get_version<C: ConnectionTrait>(
    db: &C,
    project_id: &str,
    name: Option<&str>,
) -> DbResult<project_version::Model> {
    project_version::Entity::find()
        .filter(project_version::Column::ProjectId.eq(project_id))
        .filter(project_version::Column::Name.eq(name))
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

#[tracing::instrument(name = "Getting or creating project version", skip(db, name))]
pub async fn get_or_create_version(
    db: &DatabaseConnection,
    project_id: &str,
    name: Option<&str>,
    branch: &str,
) -> DbResult<project_version::Model> {
    let existing = get_version(db, project_id, name).await;

    match existing {
        Ok(v) => Ok(v),
        Err(DbError::NotFound) => {
            let model = project_version::ActiveModel {
                project_id: Set(project_id.to_owned()),
                branch: Set(branch.to_owned()),
                name: Set(name.map(str::to_owned)),
                ..Default::default()
            };
            Ok(create(db, model).await?)
        }
        Err(e) => Err(e),
    }
}

#[tracing::instrument(name = "Upserting project version", skip(db, name))]
pub async fn upsert_version<C: ConnectionTrait>(
    db: &C,
    project_id: &str,
    name: Option<&str>,
    branch: &str,
) -> DbResult<project_version::Model> {
    match get_version(db, project_id, name).await {
        Ok(v) => {
            let supplied_name = name.map(str::to_owned);
            let needs_update = v.name != supplied_name || v.branch != branch;
            let model = if needs_update {
                let mut am: project_version::ActiveModel = v.into();
                am.name = Set(supplied_name);
                am.branch = Set(branch.to_owned());
                am.update(db).await?
            } else {
                v
            };
            Ok(model)
        }
        Err(DbError::NotFound) => {
            let model = project_version::ActiveModel {
                project_id: Set(project_id.to_owned()),
                branch: Set(branch.to_owned()),
                name: Set(name.map(str::to_owned)),
                ..Default::default()
            };
            Ok(create(db, model).await?)
        }
        Err(e) => Err(e),
    }
}

#[tracing::instrument(name = "Deleting all project versions", skip(db))]
pub async fn delete_all_for_project(db: &DatabaseConnection, project_id: &str) -> DbResult<()> {
    project_version::Entity::delete_many()
        .filter(project_version::Column::ProjectId.eq(project_id))
        .exec(db)
        .await?;
    Ok(())
}

#[tracing::instrument(name = "Getting named project versions", skip(db))]
pub async fn get_named_versions(
    db: &DatabaseConnection,
    project_id: &str,
) -> DbResult<Vec<project_version::Model>> {
    Ok(project_version::Entity::find()
        .filter(project_version::Column::ProjectId.eq(project_id))
        .filter(project_version::Column::Name.is_not_null())
        .all(db)
        .await?)
}

#[tracing::instrument(name = "Getting dev project versions", skip(db))]
pub async fn get_versions_dev(
    db: &DatabaseConnection,
    version_id: i64,
    search_query: &str,
    page: u64,
) -> DbResult<PaginatedData<project_version::Model>> {
    let project_id_subquery = sea_orm::QueryTrait::into_query(
        project_version::Entity::find_by_id(version_id)
            .select_only()
            .column(project_version::Column::ProjectId),
    );

    let query = project_version::Entity::find()
        .filter(project_version::Column::ProjectId.in_subquery(project_id_subquery))
        .filter(project_version::Column::Name.is_not_null())
        .filter(
            Condition::any()
                .add(project_version::Column::Name.contains(search_query))
                .add(project_version::Column::Branch.contains(search_query)),
        )
        .order_by(project_version::Column::Name, Order::Asc);

    paginate(db, query, page).await
}

#[tracing::instrument(name = "Deleting unused project versions", skip(db))]
pub async fn delete_unused_versions(
    db: &DatabaseConnection,
    project_id: &str,
    keep: &[String],
) -> DbResult<()> {
    let mut condition = Condition::all()
        .add(project_version::Column::ProjectId.eq(project_id))
        .add(project_version::Column::Name.is_not_null());

    if !keep.is_empty() {
        condition = condition.add(project_version::Column::Name.is_not_in(keep));
    }

    project_version::Entity::delete_many()
        .filter(condition)
        .exec(db)
        .await?;
    Ok(())
}
