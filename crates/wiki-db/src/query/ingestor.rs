use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, QuerySelect, QueryTrait, Set};

use crate::entity::{
    item, project_item, project_item_page, project_tag, project_version, recipe,
    recipe_ingredient_item, recipe_ingredient_tag, recipe_type, recipe_workbench, tag, tag_item,
    tag_tag,
};
use crate::error::{DbError, DbResult};

pub async fn delete_existing_data<C: ConnectionTrait>(conn: &C, project_id: &str) -> DbResult<()> {
    let version_ids = project_version::Entity::find()
        .select_only()
        .column(project_version::Column::Id)
        .filter(project_version::Column::ProjectId.eq(project_id))
        .into_query();

    recipe::Entity::delete_many()
        .filter(recipe::Column::VersionId.in_subquery(version_ids.clone()))
        .exec(conn)
        .await?;

    recipe_type::Entity::delete_many()
        .filter(recipe_type::Column::VersionId.in_subquery(version_ids.clone()))
        .exec(conn)
        .await?;

    project_item::Entity::delete_many()
        .filter(project_item::Column::VersionId.in_subquery(version_ids.clone()))
        .exec(conn)
        .await?;

    project_tag::Entity::delete_many()
        .filter(project_tag::Column::VersionId.in_subquery(version_ids))
        .exec(conn)
        .await?;

    Ok(())
}

pub async fn find_or_create_item<C: ConnectionTrait>(conn: &C, loc: &str) -> DbResult<item::Model> {
    if let Some(existing) = item::Entity::find()
        .filter(item::Column::Loc.eq(loc))
        .one(conn)
        .await?
    {
        return Ok(existing);
    }
    let model = item::ActiveModel {
        loc: Set(loc.to_owned()),
        ..Default::default()
    };
    Ok(model.insert(conn).await?)
}

pub async fn find_or_create_tag<C: ConnectionTrait>(conn: &C, loc: &str) -> DbResult<tag::Model> {
    if let Some(existing) = tag::Entity::find()
        .filter(tag::Column::Loc.eq(loc))
        .one(conn)
        .await?
    {
        return Ok(existing);
    }
    let model = tag::ActiveModel {
        loc: Set(loc.to_owned()),
        ..Default::default()
    };
    Ok(model.insert(conn).await?)
}

pub async fn add_project_item<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    loc: &str,
) -> DbResult<project_item::Model> {
    let item = find_or_create_item(conn, loc).await?;

    if let Some(existing) = project_item::Entity::find()
        .filter(project_item::Column::ItemId.eq(item.id))
        .filter(project_item::Column::VersionId.eq(version_id))
        .one(conn)
        .await?
    {
        return Ok(existing);
    }

    let model = project_item::ActiveModel {
        item_id: Set(item.id),
        version_id: Set(version_id),
        ..Default::default()
    };
    Ok(model.insert(conn).await?)
}

pub async fn add_project_content_page<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    loc: &str,
    path: &str,
) -> DbResult<()> {
    let pi = add_project_item(conn, version_id, loc).await?;

    let model = project_item_page::ActiveModel {
        item_id: Set(pi.id),
        path: Set(path.to_owned()),
    };
    project_item_page::Entity::insert(model)
        .on_conflict_do_nothing_on([
            project_item_page::Column::ItemId,
            project_item_page::Column::Path,
        ])
        .exec(conn)
        .await?;
    Ok(())
}

pub async fn add_project_tag<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    loc: &str,
) -> DbResult<project_tag::Model> {
    let tag = find_or_create_tag(conn, loc).await?;

    if let Some(existing) = project_tag::Entity::find()
        .filter(project_tag::Column::TagId.eq(tag.id))
        .filter(project_tag::Column::VersionId.eq(version_id))
        .one(conn)
        .await?
    {
        return Ok(existing);
    }

    let model = project_tag::ActiveModel {
        tag_id: Set(tag.id),
        version_id: Set(version_id),
        ..Default::default()
    };
    Ok(model.insert(conn).await?)
}

pub async fn add_tag_item_entry<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    tag_loc: &str,
    item_loc: &str,
) -> DbResult<()> {
    let pt = add_project_tag(conn, version_id, tag_loc).await?;
    let pi = add_project_item(conn, version_id, item_loc).await?;

    let model = tag_item::ActiveModel {
        tag_id: Set(pt.id),
        item_id: Set(pi.id),
    };
    tag_item::Entity::insert(model)
        .on_conflict_do_nothing_on([tag_item::Column::TagId, tag_item::Column::ItemId])
        .exec(conn)
        .await?;
    Ok(())
}

pub async fn add_tag_tag_entry<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    parent_loc: &str,
    child_loc: &str,
) -> DbResult<()> {
    let parent = add_project_tag(conn, version_id, parent_loc).await?;
    let child = add_project_tag(conn, version_id, child_loc).await?;

    let model = tag_tag::ActiveModel {
        parent: Set(parent.id),
        child: Set(child.id),
    };
    tag_tag::Entity::insert(model)
        .on_conflict_do_nothing_on([tag_tag::Column::Parent, tag_tag::Column::Child])
        .exec(conn)
        .await?;
    Ok(())
}

pub async fn refresh_flat_tag_item_view<C: ConnectionTrait>(conn: &C) -> DbResult<()> {
    // language=postgresql
    conn.execute_unprepared("REFRESH MATERIALIZED VIEW tag_item_flat")
        .await?;
    Ok(())
}

pub async fn add_recipe_type<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    loc: &str,
) -> DbResult<recipe_type::Model> {
    let model = recipe_type::ActiveModel {
        version_id: Set(version_id),
        loc: Set(loc.to_owned()),
        ..Default::default()
    };
    Ok(model.insert(conn).await?)
}

pub async fn get_recipe_type_by_loc<C: ConnectionTrait>(
    conn: &C,
    loc: &str,
) -> DbResult<Option<recipe_type::Model>> {
    Ok(recipe_type::Entity::find()
        .filter(recipe_type::Column::Loc.eq(loc))
        .one(conn)
        .await?)
}

pub async fn add_recipe<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    loc: &str,
    type_id: i64,
) -> DbResult<recipe::Model> {
    let model = recipe::ActiveModel {
        version_id: Set(version_id),
        loc: Set(loc.to_owned()),
        type_id: Set(type_id),
        ..Default::default()
    };
    Ok(model.insert(conn).await?)
}

pub async fn delete_recipe<C: ConnectionTrait>(conn: &C, recipe_id: i64) -> DbResult<()> {
    recipe::Entity::delete_by_id(recipe_id).exec(conn).await?;
    Ok(())
}

pub async fn add_recipe_ingredient_item<C: ConnectionTrait>(
    conn: &C,
    recipe_id: i64,
    item_loc: &str,
    slot: &str,
    count: i32,
    input: bool,
) -> DbResult<()> {
    let item = item::Entity::find()
        .filter(item::Column::Loc.eq(item_loc))
        .one(conn)
        .await?
        .ok_or_else(|| DbError::NotFound)?;

    let model = recipe_ingredient_item::ActiveModel {
        recipe_id: Set(recipe_id),
        item_id: Set(item.id),
        slot: Set(slot.to_owned()),
        count: Set(count),
        input: Set(input),
    };
    recipe_ingredient_item::Entity::insert(model)
        .exec(conn)
        .await?;
    Ok(())
}

pub async fn add_recipe_ingredient_tag<C: ConnectionTrait>(
    conn: &C,
    recipe_id: i64,
    tag_loc: &str,
    slot: &str,
    count: i32,
    input: bool,
) -> DbResult<()> {
    let tag = tag::Entity::find()
        .filter(tag::Column::Loc.eq(tag_loc))
        .one(conn)
        .await?
        .ok_or_else(|| DbError::NotFound)?;

    let model = recipe_ingredient_tag::ActiveModel {
        recipe_id: Set(recipe_id),
        tag_id: Set(tag.id),
        slot: Set(slot.to_owned()),
        count: Set(count),
        input: Set(input),
    };
    recipe_ingredient_tag::Entity::insert(model)
        .exec(conn)
        .await?;
    Ok(())
}

pub async fn add_recipe_workbenches<C: ConnectionTrait>(
    conn: &C,
    version_id: i64,
    recipe_type_loc: &str,
    item_locs: &[String],
) -> DbResult<u64> {
    let Some(rt) = get_recipe_type_by_loc(conn, recipe_type_loc).await? else {
        return Ok(0);
    };

    let mut inserted = 0u64;
    for loc in item_locs {
        let pi = add_project_item(conn, version_id, loc).await?;
        let model = recipe_workbench::ActiveModel {
            type_id: Set(rt.id),
            item_id: Set(pi.id),
        };
        let res = recipe_workbench::Entity::insert(model)
            .on_conflict_do_nothing_on([
                recipe_workbench::Column::TypeId,
                recipe_workbench::Column::ItemId,
            ])
            .exec(conn)
            .await;
        if res.is_ok() {
            inserted += 1;
        }
    }

    Ok(inserted)
}
