use serde::Serialize;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::{Arguments, Encode, FromRow, PgPool, Postgres, Type};

use crate::error::AppResult;

pub const DEFAULT_PER_PAGE: u32 = 25;
pub const MAX_PER_PAGE: u32 = 100;

#[derive(Debug, Clone, Copy)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
}

impl Pagination {
    pub fn new(page: Option<u32>, per_page: Option<u32>) -> Self {
        Self {
            page: page.unwrap_or(1).max(1),
            per_page: per_page.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE),
        }
    }

    pub fn limit(&self) -> i64 {
        i64::from(self.per_page)
    }

    pub fn offset(&self) -> i64 {
        i64::from(self.page - 1) * i64::from(self.per_page)
    }

    pub fn wrap<T>(self, data: Vec<T>, total: i64) -> Page<T> {
        Page {
            data,
            page: self.page,
            per_page: self.per_page,
            total,
        }
    }
}

#[derive(Default)]
pub struct Params {
    args: PgArguments,
    len: usize,
}

impl Params {
    pub fn arguments(self) -> PgArguments {
        self.args
    }

    pub fn placeholder(&self) -> String {
        format!("${}", self.len + 1)
    }

    pub fn bind<'q, T>(mut self, value: T) -> AppResult<Self>
    where
        T: 'q + Encode<'q, Postgres> + Type<Postgres>,
    {
        self.args.add(value).map_err(|err| anyhow::anyhow!(err))?;
        self.len += 1;
        Ok(self)
    }
}

impl Pagination {
    pub async fn fetch<T>(
        self,
        pool: &PgPool,
        sql: &str,
        order: &str,
        params: Params,
    ) -> AppResult<Page<T>>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    {
        let total: i64 = sqlx::query_scalar_with(
            &format!("SELECT count(*) FROM ({sql}) AS filtered"),
            params.args.clone(),
        )
        .fetch_one(pool)
        .await?;
        let Params { args, len } = params.bind(self.limit())?.bind(self.offset())?;
        let rows = sqlx::query_as_with(
            &format!("{sql} ORDER BY {order} LIMIT ${} OFFSET ${len}", len - 1),
            args,
        )
        .fetch_all(pool)
        .await?;
        Ok(self.wrap(rows, total))
    }
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_bounds() {
        let p = Pagination::new(None, None);
        assert_eq!((p.page, p.per_page), (1, DEFAULT_PER_PAGE));

        let p = Pagination::new(Some(0), Some(10_000));
        assert_eq!((p.page, p.per_page), (1, MAX_PER_PAGE));
    }

    #[test]
    fn offset_skips_previous_pages() {
        assert_eq!(Pagination::new(Some(3), Some(20)).offset(), 40);
    }
}
