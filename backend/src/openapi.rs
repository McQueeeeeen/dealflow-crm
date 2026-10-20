use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::api;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "DealFlow CRM API",
        description = "Multi-tenant sales CRM. Every request is scoped to the caller's organization; \
                       IDs from another organization behave as if they do not exist.",
    ),
    paths(
        api::health,
        api::auth::register,
        api::auth::login,
        api::auth::refresh_session,
        api::auth::logout,
        api::auth::me,
        api::users::list_users,
        api::users::create_user,
        api::users::update_user,
        api::stages::list_stages,
        api::stages::create_stage,
        api::stages::update_stage,
        api::stages::reorder_stages,
        api::stages::delete_stage,
        api::companies::list_companies,
        api::companies::get_company,
        api::companies::create_company,
        api::companies::update_company,
        api::companies::delete_company,
        api::contacts::list_contacts,
        api::contacts::get_contact,
        api::contacts::create_contact,
        api::contacts::update_contact,
        api::contacts::delete_contact,
        api::deals::list_deals,
        api::deals::board,
        api::deals::get_deal,
        api::deals::create_deal,
        api::deals::update_deal,
        api::deals::move_deal,
        api::deals::delete_deal,
        api::deals::stage_history,
        api::tasks::list_tasks,
        api::tasks::get_task,
        api::tasks::create_task,
        api::tasks::update_task,
        api::tasks::complete_task,
        api::tasks::reopen_task,
        api::tasks::delete_task,
        api::notes::list_notes,
        api::notes::create_note,
        api::notes::delete_note,
        api::audit_log::list_audit,
        api::analytics::summary,
        api::analytics::funnel,
        api::analytics::forecast,
        api::analytics::leaderboard,
    ),
    modifiers(&BearerAuth),
)]
pub struct ApiDoc;

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }
}
