use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::Role;

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }

    pub fn is_manager_or_admin(&self) -> bool {
        matches!(self.role, Role::Admin | Role::Manager)
    }

    pub fn require_admin(&self) -> AppResult<()> {
        allow(self.is_admin())
    }

    pub fn require_manager(&self) -> AppResult<()> {
        allow(self.is_manager_or_admin())
    }

    pub fn can_modify(&self, owner_id: Uuid) -> bool {
        self.is_manager_or_admin() || owner_id == self.user_id
    }

    pub fn require_can_modify(&self, owner_id: Uuid) -> AppResult<()> {
        allow(self.can_modify(owner_id))
    }

    pub fn resolve_owner(&self, requested: Option<Uuid>) -> AppResult<Uuid> {
        match requested {
            None => Ok(self.user_id),
            Some(owner) if owner == self.user_id || self.is_manager_or_admin() => Ok(owner),
            Some(_) => Err(AppError::Forbidden),
        }
    }
}

fn allow(permitted: bool) -> AppResult<()> {
    if permitted {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role) -> AuthUser {
        AuthUser {
            user_id: Uuid::now_v7(),
            org_id: Uuid::now_v7(),
            role,
        }
    }

    #[test]
    fn only_admins_pass_admin_checks() {
        assert!(user(Role::Admin).require_admin().is_ok());
        assert!(user(Role::Manager).require_admin().is_err());
        assert!(user(Role::SalesRep).require_admin().is_err());
    }

    #[test]
    fn managers_and_admins_pass_manager_checks() {
        assert!(user(Role::Admin).require_manager().is_ok());
        assert!(user(Role::Manager).require_manager().is_ok());
        assert!(user(Role::SalesRep).require_manager().is_err());
    }

    #[test]
    fn reps_modify_only_their_own_records() {
        let rep = user(Role::SalesRep);
        assert!(rep.can_modify(rep.user_id));
        assert!(!rep.can_modify(Uuid::now_v7()));
        assert!(user(Role::Manager).can_modify(Uuid::now_v7()));
    }

    #[test]
    fn reps_cannot_assign_records_to_others() {
        let rep = user(Role::SalesRep);
        assert_eq!(rep.resolve_owner(None).unwrap(), rep.user_id);
        assert_eq!(rep.resolve_owner(Some(rep.user_id)).unwrap(), rep.user_id);
        assert!(rep.resolve_owner(Some(Uuid::now_v7())).is_err());

        let manager = user(Role::Manager);
        let colleague = Uuid::now_v7();
        assert_eq!(manager.resolve_owner(Some(colleague)).unwrap(), colleague);
    }
}
