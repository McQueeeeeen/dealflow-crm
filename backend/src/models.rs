use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Manager,
    SalesRep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "stage_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    Open,
    Won,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "audit_action", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    Company,
    Contact,
    Deal,
    Task,
    User,
    Stage,
}

impl EntityType {
    pub const ALL: [EntityType; 6] = [
        Self::Company,
        Self::Contact,
        Self::Deal,
        Self::Task,
        Self::User,
        Self::Stage,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Company => "company",
            Self::Contact => "contact",
            Self::Deal => "deal",
            Self::Task => "task",
            Self::User => "user",
            Self::Stage => "stage",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}
