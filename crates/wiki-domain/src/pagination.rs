use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PaginatedData<T> {
    pub total: u64,
    pub pages: u64,
    pub size: u64,
    pub data: Vec<T>,
}

impl<T> PaginatedData<T> {
    pub fn empty() -> Self {
        Self {
            total: 0,
            pages: 0,
            size: 0,
            data: Vec::new(),
        }
    }

    pub fn new(data: Vec<T>, total: u64, page_size: u64) -> Self {
        let pages = if page_size > 0 {
            (total + page_size - 1) / page_size
        } else {
            0
        };
        Self {
            total,
            pages,
            size: page_size,
            data,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TableQueryParams {
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_page")]
    pub page: u64,
}

fn default_page() -> u64 {
    1
}

impl Default for TableQueryParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            page: 1,
        }
    }
}
