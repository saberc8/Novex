use sqlx::PgPool;

use crate::{
    domain::{
        auth::model::CurrentUser,
        rbac::model::{build_route_tree, PermissionContext, RouteItem},
    },
    infrastructure::persistence::rbac_repository::RbacRepository,
    shared::error::AppError,
};

#[derive(Debug, Clone)]
pub struct RbacService {
    menus: RbacRepository,
}

impl RbacService {
    pub fn new(db: PgPool) -> Self {
        Self {
            menus: RbacRepository::new(db),
        }
    }

    pub async fn route_tree(&self, current_user: &CurrentUser) -> Result<Vec<RouteItem>, AppError> {
        let permission_context = PermissionContext::from(current_user);
        let role_codes = permission_context.role_codes.clone();
        let menus = if permission_context.is_admin() {
            self.menus.all_enabled_route_menus().await?
        } else {
            self.menus
                .enabled_route_menus_by_user_id_for_tenant(current_user.id, current_user.tenant_id)
                .await?
        };

        Ok(build_route_tree(menus, role_codes))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_admin_route_tree_uses_current_user_active_tenant() {
        let source = include_str!("service.rs");

        assert!(
            source
                .matches(
                    "enabled_route_menus_by_user_id_for_tenant(current_user.id, current_user.tenant_id)"
                )
                .count()
                >= 2
        );
    }
}
